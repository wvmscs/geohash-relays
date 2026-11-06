// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The async front end (feature `async`): no runtime imposed, the host's
//! [`RelayComm`] waits and talks.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use super::Strategy;
use super::engine::{Check, Engine};
use super::files::HEALTH;
use super::types::{Authority, Cell, CellRelays, CommError, Extra, RelayUrl, Seen, StoredFile};
use crate::error::{Error, Result};
use crate::params::RUN_TICK_SECS;
use crate::params::comm::{MAX_LIST_BYTES, MAX_NIP11_BYTES};

/// Everything geohash-relays needs from the host. The host only carries
/// bytes: what to fetch, when, what to keep and what a failure means is
/// decided here.
pub trait RelayComm: Send + Sync + 'static {
    type Link: Link;
    type File: StoredFile;

    /// A WebSocket to `relay`, isolated per `cell` (one Tor circuit per
    /// cell when the host uses Tor).
    fn connect_relay(
        &self,
        relay: &RelayUrl,
        cell: &Cell,
    ) -> impl Future<Output = std::result::Result<Self::Link, CommError>> + Send;

    /// An HTTPS GET of `url` with this `Accept` header; the body.
    fn fetch(
        &self,
        url: &str,
        accept: &str,
    ) -> impl Future<Output = std::result::Result<Vec<u8>, CommError>> + Send;

    /// The persistent file called `name`, empty when it does not exist yet.
    fn file(&self, name: &str) -> std::result::Result<Self::File, CommError>;

    /// Unix time, in seconds.
    fn now(&self) -> i64;

    fn sleep(&self, seconds: u64) -> impl Future<Output = ()> + Send;

    /// Where the log lines go (relay addresses and counts, never contents
    /// or keys). Default: nowhere.
    fn log(&self, _line: &str) {}
}

/// A link to a relay: text frames in and out. Dropping it closes it.
pub trait Link: Send {
    fn send(
        &mut self,
        frame: &str,
    ) -> impl Future<Output = std::result::Result<(), CommError>> + Send;

    /// The next frame; `None` when the relay closed the link.
    fn recv(
        &mut self,
    ) -> impl Future<Output = std::result::Result<Option<String>, CommError>> + Send;
}

struct Inner<C> {
    comm: C,
    engine: Mutex<Engine>,
}

/// The relays of the cells, kept up to date. Cloning it shares it.
pub struct Relays<C: RelayComm> {
    inner: Arc<Inner<C>>,
}

impl<C: RelayComm> Clone for Relays<C> {
    fn clone(&self) -> Self {
        Relays {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<C: RelayComm> Relays<C> {
    pub fn new(comm: C, strategy: Strategy) -> Relays<C> {
        Relays {
            inner: Arc::new(Inner {
                comm,
                engine: Mutex::new(Engine::new(strategy)),
            }),
        }
    }

    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.inner.engine.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ------------------------------------------------------------ building

    /// During a migration: `get` gives the relays of both strategies.
    pub fn migrating_to(self, target: Strategy) -> Relays<C> {
        self.engine().cfg.target = Some(target);
        self
    }

    /// Demonstrations: more strategies, asked through
    /// [`get_with`](Self::get_with).
    pub fn also(self, others: &[Strategy]) -> Relays<C> {
        self.engine().cfg.others.extend_from_slice(others);
        self
    }

    /// Keep only the relays the cells of `region` need, at least
    /// `min_relays` (constrained devices). Ignored while a strategy needs
    /// the whole list (A1).
    pub fn home(self, region: Cell, min_relays: usize) -> Relays<C> {
        self.engine().cfg.home = Some((region, min_relays));
        self
    }

    /// Replace the default sources of lists ([`Authority::defaults`]).
    pub fn authorities(self, authorities: Vec<Authority>) -> Relays<C> {
        self.engine().cfg.authorities = authorities;
        self
    }

    /// Private or special relays: beside the strategy's (`Append`) or
    /// instead (`Replace`).
    pub fn extra_relays(self, relays: Vec<RelayUrl>, mode: Extra) -> Relays<C> {
        {
            let mut e = self.engine();
            e.cfg.extra = relays;
            e.cfg.extra_mode = mode;
        }
        self
    }

    /// Check the relays in use and set aside those that are dead or refuse
    /// (default: on). Only A0 replaces them; the other strategies keep
    /// what every peer chooses.
    pub fn qualify(self, on: bool) -> Relays<C> {
        self.engine().cfg.qualify = on;
        self
    }

    // ------------------------------------------------------------ running

    /// Refreshes the lists on their schedule and checks the relays in use.
    /// Never returns: spawn it on your runtime.
    pub async fn run(self) {
        loop {
            if let Err(e) = self.work(false).await {
                self.inner.comm.log(&format!("geohash-relays: {e}"));
            }
            self.inner.comm.sleep(RUN_TICK_SECS).await;
        }
    }

    /// Download every list again and check every relay in use, now.
    pub async fn refresh(&self) -> Result<()> {
        self.work(true).await
    }

    /// The relays of `cell` for the strategy in use, and for the migration
    /// target if any. The first call waits for a list when none is kept.
    pub async fn get(&self, cell: &Cell) -> Result<CellRelays> {
        self.ready().await?;
        let now = self.inner.comm.now();
        self.engine().get(None, cell, now)
    }

    /// The same for one of the strategies given to [`also`](Self::also)
    /// (or the one in use, or the target).
    pub async fn get_with(&self, strategy: &Strategy, cell: &Cell) -> Result<CellRelays> {
        self.ready().await?;
        let now = self.inner.comm.now();
        self.engine().get(Some(strategy), cell, now)
    }

    /// What the client's own connections saw of a relay.
    pub fn report(&self, relay: &RelayUrl, seen: Seen) {
        let now = self.inner.comm.now();
        let bytes = {
            let mut e = self.engine();
            e.report(relay, seen, now);
            e.health_bytes()
        };
        self.store(HEALTH, &bytes);
    }

    // ------------------------------------------------------------ work

    /// Kept files read, and a list at hand.
    async fn ready(&self) -> Result<()> {
        self.restore();
        if !self.engine().has_list() {
            self.fetch_lists(false).await;
        }
        if self.engine().has_list() {
            Ok(())
        } else {
            Err(Error::NoRelayList)
        }
    }

    async fn work(&self, force: bool) -> Result<()> {
        self.restore();
        self.fetch_lists(force).await;
        self.check_relays(force).await;
        if self.engine().has_list() {
            Ok(())
        } else {
            Err(Error::NoRelayList)
        }
    }

    fn restore(&self) {
        if self.engine().restored() {
            return;
        }
        let names = self.engine().files_to_restore();
        for name in names {
            let bytes = match self.inner.comm.file(&name).and_then(|mut f| f.read()) {
                Ok(b) => b,
                Err(e) => {
                    self.inner.comm.log(&format!("geohash-relays: {name}: {e}"));
                    continue;
                }
            };
            if let Err(e) = self.engine().restore(&name, &bytes) {
                self.inner.comm.log(&format!("geohash-relays: {e}"));
            }
        }
        self.engine().mark_restored();
    }

    async fn fetch_lists(&self, force: bool) {
        let comm = &self.inner.comm;
        let due = self.engine().due_lists(comm.now(), force);
        for authority in due {
            let body = comm.fetch(&authority.url, "text/csv").await;
            let now = comm.now();
            let kept = match body {
                Ok(b) if b.len() <= MAX_LIST_BYTES => {
                    self.engine().ingest_list(&authority, &b, now)
                }
                Ok(_) => Err(Error::Unreadable(format!(
                    "list {} (too large)",
                    authority.url
                ))),
                Err(e) => Err(Error::Unreadable(format!("list {}: {e}", authority.url))),
            };
            match kept {
                Ok(bytes) => {
                    self.store(&Engine::list_file(&authority.url), &bytes);
                    comm.log(&format!("geohash-relays: list {} fetched", authority.url));
                }
                Err(e) => {
                    self.engine().list_failed(&authority, now);
                    comm.log(&format!("geohash-relays: {e}"));
                }
            }
        }
    }

    async fn check_relays(&self, force: bool) {
        let comm = &self.inner.comm;
        let due: Vec<Check> = self.engine().due_checks(comm.now(), force);
        if due.is_empty() {
            return;
        }
        for check in due {
            let nip11 = comm
                .fetch(&check.relay.nip11_url(), "application/nostr+json")
                .await
                .ok()
                .filter(|b| b.len() <= MAX_NIP11_BYTES);
            let reachable = comm.connect_relay(&check.relay, &check.cell).await.is_ok();
            let now = comm.now();
            self.engine()
                .checked(&check.relay, nip11.as_deref(), reachable, now);
        }
        let bytes = self.engine().health_bytes();
        self.store(HEALTH, &bytes);
    }

    fn store(&self, name: &str, bytes: &[u8]) {
        let written = self.inner.comm.file(name).and_then(|mut f| f.write(bytes));
        if let Err(e) = written {
            self.inner.comm.log(&format!("geohash-relays: {name}: {e}"));
        }
    }
}
