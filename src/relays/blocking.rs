// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The blocking front end (feature `blocking`), for devices without an
//! async runtime: nothing runs in the background. Call
//! [`refresh`](Relays::refresh) at start-up (and now and then), then
//! [`get`](Relays::get), which only does I/O when no list is kept.

use std::sync::{Arc, Mutex, MutexGuard};

use super::Strategy;
use super::engine::{Check, Engine};
use super::files::HEALTH;
pub use super::types::StoredFile;
use super::types::{Authority, Cell, CellRelays, CommError, Extra, RelayUrl, Seen};
use crate::error::{Error, Result};
use crate::params::comm::{MAX_LIST_BYTES, MAX_NIP11_BYTES};

/// The async [`RelayComm`](super::RelayComm), blocking.
pub trait RelayComm: Send + Sync + 'static {
    type Link: Link;
    type File: StoredFile;

    /// A WebSocket to `relay`, isolated per `cell`.
    fn connect_relay(
        &self,
        relay: &RelayUrl,
        cell: &Cell,
    ) -> std::result::Result<Self::Link, CommError>;

    /// An HTTPS GET of `url` with this `Accept` header; the body.
    fn fetch(&self, url: &str, accept: &str) -> std::result::Result<Vec<u8>, CommError>;

    /// The persistent file called `name`, empty when it does not exist yet.
    fn file(&self, name: &str) -> std::result::Result<Self::File, CommError>;

    /// Unix time, in seconds.
    fn now(&self) -> i64;

    /// Where the log lines go. Default: nowhere.
    fn log(&self, _line: &str) {}
}

/// A link to a relay: text frames in and out. Dropping it closes it.
pub trait Link {
    fn send(&mut self, frame: &str) -> std::result::Result<(), CommError>;
    /// The next frame; `None` when the relay closed the link.
    fn recv(&mut self) -> std::result::Result<Option<String>, CommError>;
}

struct Inner<C> {
    comm: C,
    engine: Mutex<Engine>,
}

/// The relays of the cells. Cloning it shares it.
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

    /// See [`super::Relays::migrating_to`].
    pub fn migrating_to(self, target: Strategy) -> Relays<C> {
        self.engine().cfg.target = Some(target);
        self
    }

    /// See [`super::Relays::also`].
    pub fn also(self, others: &[Strategy]) -> Relays<C> {
        self.engine().cfg.others.extend_from_slice(others);
        self
    }

    /// See [`super::Relays::home`].
    pub fn home(self, region: Cell, min_relays: usize) -> Relays<C> {
        self.engine().cfg.home = Some((region, min_relays));
        self
    }

    /// See [`super::Relays::authorities`].
    pub fn authorities(self, authorities: Vec<Authority>) -> Relays<C> {
        self.engine().cfg.authorities = authorities;
        self
    }

    /// See [`super::Relays::extra_relays`].
    pub fn extra_relays(self, relays: Vec<RelayUrl>, mode: Extra) -> Relays<C> {
        {
            let mut e = self.engine();
            e.cfg.extra = relays;
            e.cfg.extra_mode = mode;
        }
        self
    }

    /// See [`super::Relays::qualify`].
    pub fn qualify(self, on: bool) -> Relays<C> {
        self.engine().cfg.qualify = on;
        self
    }

    /// The due work: lists older than a day, relays not checked for 6 h.
    /// `force`: all of it.
    pub fn refresh(&self, force: bool) -> Result<()> {
        self.restore();
        self.fetch_lists(force);
        self.check_relays(force);
        if self.engine().has_list() {
            Ok(())
        } else {
            Err(Error::NoRelayList)
        }
    }

    /// See [`super::Relays::get`].
    pub fn get(&self, cell: &Cell) -> Result<CellRelays> {
        self.ready()?;
        let now = self.inner.comm.now();
        self.engine().get(None, cell, now)
    }

    /// See [`super::Relays::get_with`].
    pub fn get_with(&self, strategy: &Strategy, cell: &Cell) -> Result<CellRelays> {
        self.ready()?;
        let now = self.inner.comm.now();
        self.engine().get(Some(strategy), cell, now)
    }

    /// See [`super::Relays::report`].
    pub fn report(&self, relay: &RelayUrl, seen: Seen) {
        let now = self.inner.comm.now();
        let bytes = {
            let mut e = self.engine();
            e.report(relay, seen, now);
            e.health_bytes()
        };
        self.store(HEALTH, &bytes);
    }

    fn ready(&self) -> Result<()> {
        self.restore();
        if !self.engine().has_list() {
            self.fetch_lists(false);
        }
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

    fn fetch_lists(&self, force: bool) {
        let comm = &self.inner.comm;
        let due = self.engine().due_lists(comm.now(), force);
        for authority in due {
            let body = comm.fetch(&authority.url, "text/csv");
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

    fn check_relays(&self, force: bool) {
        let comm = &self.inner.comm;
        let due: Vec<Check> = self.engine().due_checks(comm.now(), force);
        if due.is_empty() {
            return;
        }
        for check in due {
            let nip11 = comm
                .fetch(&check.relay.nip11_url(), "application/nostr+json")
                .ok()
                .filter(|b| b.len() <= MAX_NIP11_BYTES);
            let reachable = comm.connect_relay(&check.relay, &check.cell).is_ok();
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
