// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A6`: automatic relay qualification, NIP-11 and probes
//! (`docs/geohash-relays-requirements-and-alternatives.md`,
//! §5 A6, requirements of §3).
//!
//! Not a rendezvous: a **filter** of the candidates, before they are ranked,
//! for A0, A1 or A2.
//!
//! * [`Nip11`]: what a relay declares; [`Verdict::of`] checks it against the
//!   requirements (R5, R6, R7 refuse; R8, R11, R13 warn).
//! * [`Probe`]: what a test showed (write a kind 20000 event, read it back,
//!   through Tor): it **prevails** over the declaration.
//! * [`Prober`]: the probe machine ([`crate::sansio`]): fetches the NIP-11
//!   documents, publishes the test events the host signed, reads them back.
//! * [`Qualification`]: the verdicts, and the filter of a list. A filter
//!   only this peer applies breaks the rendezvous (peers no longer see the
//!   same candidates): it is only accepted by a strategy that writes wide
//!   ([`crate::a0::SharedStrategy`]), or when the verdicts are shared
//!   (published by monitors, A2).

use std::collections::{BTreeMap, HashSet, VecDeque};

use serde_json::Value;

use crate::directory::{RelayDirectory, normalize_url};
use crate::error::{Error, Result};
use crate::params::{
    A6_CONCURRENCY, A6_MAX_POW_BITS, A6_MIN_EPHEMERAL_RETENTION_SECS, A6_MIN_MESSAGE_BYTES,
    A6_MIN_SUBSCRIPTIONS, A6_PROBE_CELL, A6_PROBE_FIRST_READ_SECS, A6_PROBE_TIMEOUT_SECS,
    KIND_CHANNEL, KIND_PRESENCE,
};
use crate::sansio::{Action, EventMeta, FilterSpec, Input, Machine};

/// A NIP-11 retention rule: the kinds it covers (ranges; `None`: every
/// kind), and how long they are kept in seconds (`None`: forever).
pub type RetentionRule = (Option<Vec<(u32, u32)>>, Option<i64>);

/// The parts of a NIP-11 document the requirements read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nip11 {
    pub auth_required: bool,
    pub payment_required: bool,
    pub restricted_writes: bool,
    pub min_pow_difficulty: u32,
    pub max_subscriptions: Option<u64>,
    pub max_message_length: Option<u64>,
    pub retention: Vec<RetentionRule>,
}

impl Nip11 {
    /// Reads a NIP-11 document; unknown or ill-typed fields are ignored.
    pub fn parse(json: &str) -> Option<Nip11> {
        let v: Value = serde_json::from_str(json).ok()?;
        let o = v.as_object()?;
        let lim = o.get("limitation").and_then(Value::as_object);
        let flag = |k: &str| {
            lim.and_then(|l| l.get(k))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        let num = |k: &str| lim.and_then(|l| l.get(k)).and_then(Value::as_u64);
        let mut retention = Vec::new();
        for r in o
            .get("retention")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let kinds = r.get("kinds").and_then(Value::as_array).map(|ks| {
                ks.iter()
                    .filter_map(|k| match k {
                        Value::Number(n) => n.as_u64().map(|n| (n as u32, n as u32)),
                        Value::Array(a) => {
                            Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32))
                        }
                        _ => None,
                    })
                    .collect()
            });
            let time = match r.get("time") {
                Some(Value::Number(n)) => n.as_i64(),
                _ => None,
            };
            retention.push((kinds, time));
        }
        Some(Nip11 {
            auth_required: flag("auth_required"),
            payment_required: flag("payment_required"),
            restricted_writes: flag("restricted_writes"),
            min_pow_difficulty: num("min_pow_difficulty").unwrap_or(0) as u32,
            max_subscriptions: num("max_subscriptions"),
            max_message_length: num("max_message_length"),
            retention,
        })
    }

    /// Declared retention of `kind` in seconds: `None` when nothing is
    /// declared, `Some(None)` for forever. The first rule that names the
    /// kind wins, then a rule without kinds.
    pub fn retention_of(&self, kind: u32) -> Option<Option<i64>> {
        let named = self.retention.iter().find(|(ks, _)| {
            ks.as_ref()
                .is_some_and(|ks| ks.iter().any(|(a, b)| (*a..=*b).contains(&kind)))
        });
        named
            .or_else(|| self.retention.iter().find(|(ks, _)| ks.is_none()))
            .map(|(_, t)| *t)
    }
}

/// What a probe showed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Probe {
    /// The WebSocket upgrade was redirected (R2).
    pub redirected: bool,
    /// The TLS certificate was refused (R1).
    pub bad_certificate: bool,
    /// `OK true` for the test event (kind 20000).
    pub accepted: Option<bool>,
    /// The relay sent the test event to a live subscription (R9).
    pub live: Option<bool>,
    /// Read back after these delays (seconds): found or not.
    pub read_back: Vec<(i64, bool)>,
}

/// A requirement a relay does not meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// R1: no valid public certificate.
    Certificate,
    /// R2: redirection.
    Redirect,
    /// R5: NIP-42 required.
    AuthRequired,
    /// R6: payment required.
    Payment,
    /// R6: writes restricted (allow list, web of trust).
    RestrictedWrites,
    /// R7: proof of work above 8 bits.
    Pow(u32),
    /// R9: the test event was refused, or not sent back.
    Kind20000,
    /// R8: too few subscriptions per connection (warning).
    Subscriptions(u64),
    /// R11: ephemeral kinds kept less than 1 h (seconds; warning).
    Retention(i64),
    /// R13: messages smaller than 64 KiB (warning).
    MessageLength(u64),
}

impl Finding {
    /// The requirement, as the document numbers it.
    pub fn requirement(&self) -> &'static str {
        match self {
            Finding::Certificate => "R1",
            Finding::Redirect => "R2",
            Finding::AuthRequired => "R5",
            Finding::Payment | Finding::RestrictedWrites => "R6",
            Finding::Pow(_) => "R7",
            Finding::Subscriptions(_) => "R8",
            Finding::Kind20000 => "R9",
            Finding::Retention(_) => "R11",
            Finding::MessageLength(_) => "R13",
        }
    }
}

/// Whether a relay can serve location channels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Verdict {
    /// Requirements not met that make the relay unusable (MUST).
    pub refusals: Vec<Finding>,
    /// Requirements not met that only weaken it (SHOULD).
    pub warnings: Vec<Finding>,
    /// A probe was made (its results prevail).
    pub probed: bool,
}

impl Verdict {
    pub fn usable(&self) -> bool {
        self.refusals.is_empty()
    }

    /// The verdict from a declaration and a probe, either may be missing.
    pub fn of(nip11: Option<&Nip11>, probe: Option<&Probe>) -> Verdict {
        let mut v = Verdict {
            probed: probe.is_some(),
            ..Verdict::default()
        };
        // What the probe saw of the access: it writes and delivers live, or
        // not (storage is R11, a SHOULD).
        let works = probe.and_then(|p| {
            p.accepted.map(|ok| {
                ok && p
                    .live
                    .unwrap_or_else(|| p.read_back.first().is_some_and(|(_, found)| *found))
            })
        });
        if let Some(n) = nip11 {
            if works != Some(true) {
                if n.auth_required {
                    v.refusals.push(Finding::AuthRequired);
                }
                if n.payment_required {
                    v.refusals.push(Finding::Payment);
                }
                if n.restricted_writes {
                    v.refusals.push(Finding::RestrictedWrites);
                }
                if n.min_pow_difficulty > A6_MAX_POW_BITS {
                    v.refusals.push(Finding::Pow(n.min_pow_difficulty));
                }
            }
            if let Some(m) = n.max_subscriptions.filter(|m| *m < A6_MIN_SUBSCRIPTIONS) {
                v.warnings.push(Finding::Subscriptions(m));
            }
            if let Some(m) = n.max_message_length.filter(|m| *m < A6_MIN_MESSAGE_BYTES) {
                v.warnings.push(Finding::MessageLength(m));
            }
            let declared = [KIND_CHANNEL, KIND_PRESENCE]
                .iter()
                .filter_map(|k| n.retention_of(*k).flatten())
                .min();
            if let Some(t) = declared.filter(|t| *t < A6_MIN_EPHEMERAL_RETENTION_SECS) {
                v.warnings.push(Finding::Retention(t));
            }
        }
        if let Some(p) = probe {
            if p.bad_certificate {
                v.refusals.push(Finding::Certificate);
            }
            if p.redirected {
                v.refusals.push(Finding::Redirect);
            }
            if works == Some(false) {
                v.refusals.push(Finding::Kind20000);
            }
            // Measured retention: the first delay of an hour or more where
            // the event was gone.
            let lost = p
                .read_back
                .iter()
                .filter(|(d, found)| !found && *d <= A6_MIN_EPHEMERAL_RETENTION_SECS)
                .map(|(d, _)| *d)
                .min();
            if let Some(d) = lost.filter(|_| works == Some(true)) {
                v.warnings.retain(|w| !matches!(w, Finding::Retention(_)));
                v.warnings.push(Finding::Retention(d));
            }
        }
        v
    }
}

/// Verdicts by relay (normalised URL).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Qualification {
    pub verdicts: BTreeMap<String, Verdict>,
    /// Published for every peer (monitors, A2), not only this one's tests.
    pub shared: bool,
}

impl Qualification {
    /// Whether the relay may be a candidate: not refused (an unknown relay
    /// is kept).
    pub fn allows(&self, url: &str) -> bool {
        let url = normalize_url(url).unwrap_or_else(|| url.to_owned());
        self.verdicts.get(&url).is_none_or(Verdict::usable)
    }

    /// The list without the refused relays. Only a shared qualification can
    /// filter a list that a strategy reads narrow (`AS`, A1);
    /// [`SharedStrategy::qualified`](crate::a0::SharedStrategy::qualified)
    /// takes a local one.
    pub fn filter(&self, list: RelayDirectory) -> Result<RelayDirectory> {
        if !self.shared {
            return Err(Error::LocalQualification);
        }
        Ok(self.filter_any(list))
    }

    pub(crate) fn filter_any(&self, list: RelayDirectory) -> RelayDirectory {
        list.retain(|r| self.allows(&r.url))
    }

    /// Relays refused, with the requirements they fail.
    pub fn refused(&self) -> Vec<(&str, Vec<&'static str>)> {
        self.verdicts
            .iter()
            .filter(|(_, v)| !v.usable())
            .map(|(u, v)| {
                (
                    u.as_str(),
                    v.refusals.iter().map(Finding::requirement).collect(),
                )
            })
            .collect()
    }
}

/// The probe machine. For each relay: its NIP-11 document, and, when the
/// host gave a signed test event for it, a live subscription then the
/// publication (R9: sent to live subscribers), then read-backs after
/// `delays` (the first one [`A6_PROBE_FIRST_READ_SECS`] after the `OK`;
/// R11: storage). Test events are kind 20000 in [`A6_PROBE_CELL`], one per
/// relay.
pub struct Prober {
    relays: Vec<String>,
    tests: BTreeMap<String, EventMeta>,
    delays: Vec<i64>,
    nip11: BTreeMap<String, Option<Nip11>>,
    probes: BTreeMap<String, Probe>,
    /// When the `OK` came, per relay.
    accepted_at: BTreeMap<String, i64>,
    /// Read-backs in progress: relay -> (delay, found so far).
    reading: BTreeMap<String, (i64, bool)>,
    /// Relays that sent the test event on the live subscription.
    live: HashSet<String>,
    /// Waiting for their turn ([`A6_CONCURRENCY`] at a time): NIP-11
    /// fetches, probes; and the probes running, since when.
    nip11_queue: VecDeque<String>,
    nip11_in_flight: usize,
    probe_queue: VecDeque<String>,
    probing: BTreeMap<String, i64>,
    started: i64,
    now: i64,
}

impl Prober {
    /// `tests`: the signed test event for each relay to probe by writing
    /// (none: NIP-11 only). `delays`: read-backs after the first one.
    pub fn new(relays: Vec<String>, tests: BTreeMap<String, EventMeta>, delays: &[i64]) -> Self {
        let mut d = vec![A6_PROBE_FIRST_READ_SECS];
        d.extend(
            delays
                .iter()
                .copied()
                .filter(|x| *x > A6_PROBE_FIRST_READ_SECS),
        );
        Prober {
            relays,
            tests,
            delays: d,
            nip11: BTreeMap::new(),
            probes: BTreeMap::new(),
            accepted_at: BTreeMap::new(),
            reading: BTreeMap::new(),
            live: HashSet::new(),
            nip11_queue: VecDeque::new(),
            nip11_in_flight: 0,
            probe_queue: VecDeque::new(),
            probing: BTreeMap::new(),
            started: 0,
            now: 0,
        }
    }

    /// The probe filter: the test event of a relay, in the probe cell.
    pub fn filter(test: &EventMeta, since: i64) -> FilterSpec {
        FilterSpec {
            kinds: vec![KIND_CHANNEL],
            tags: vec![('g', vec![A6_PROBE_CELL.to_owned()])],
            authors: vec![test.pubkey.clone()],
            since: Some(since),
            limit: Some(10),
        }
    }

    /// Done: every NIP-11 answer is in, and every probe has finished or
    /// timed out.
    pub fn done(&self) -> bool {
        self.nip11.len() == self.relays.len()
            && self.probe_queue.is_empty()
            && self.probing.is_empty()
    }

    /// A running probe is over: refused, read back after every delay, or
    /// silent for too long.
    fn finished(&self, relay: &str, since: i64) -> bool {
        let p = self.probes.get(relay);
        p.is_some_and(|p| p.accepted == Some(false) || p.read_back.len() == self.delays.len())
            || (!self.accepted_at.contains_key(relay) && self.now - since > A6_PROBE_TIMEOUT_SECS)
    }

    /// A probe that just finished makes room for the next.
    fn settle(&mut self, relay: &str) -> Vec<Action> {
        match self.probing.get(relay) {
            Some(since) if self.finished(relay, *since) => {
                self.probing.remove(relay);
                self.next_probes()
            }
            _ => Vec::new(),
        }
    }

    fn next_fetches(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        while self.nip11_in_flight < A6_CONCURRENCY
            && let Some(relay) = self.nip11_queue.pop_front()
        {
            self.nip11_in_flight += 1;
            out.push(Action::FetchNip11 { relay });
        }
        out
    }

    fn next_probes(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        while self.probing.len() < A6_CONCURRENCY
            && let Some(relay) = self.probe_queue.pop_front()
        {
            let e = &self.tests[&relay];
            out.push(Action::Subscribe {
                relay: relay.clone(),
                filter: Prober::filter(e, e.created_at - 1),
            });
            out.push(Action::Publish {
                relay: relay.clone(),
                id: e.id.clone(),
                raw: e.raw.clone(),
            });
            self.probing.insert(relay, self.now);
        }
        out
    }

    /// The verdicts so far (local: this peer's own tests).
    pub fn qualification(&self) -> Qualification {
        let verdicts = self
            .relays
            .iter()
            .map(|r| {
                let n = self.nip11.get(r).and_then(Option::as_ref);
                let p = self.probes.get(r).filter(|p| p.accepted.is_some());
                (r.clone(), Verdict::of(n, p))
            })
            .collect();
        Qualification {
            verdicts,
            shared: false,
        }
    }

    /// A relay's NIP-11 document, if it had one.
    pub fn nip11_of(&self, relay: &str) -> Option<&Nip11> {
        self.nip11.get(relay).and_then(Option::as_ref)
    }

    fn norm(relay: &str) -> String {
        normalize_url(relay).unwrap_or_else(|| relay.to_owned())
    }
}

impl Machine for Prober {
    fn start(&mut self, now: i64) -> Vec<Action> {
        self.started = now;
        self.now = now;
        self.relays = self.relays.iter().map(|r| Self::norm(r)).collect();
        self.tests = std::mem::take(&mut self.tests)
            .into_iter()
            .map(|(r, e)| (Self::norm(&r), e))
            .collect();
        self.nip11_queue = self.relays.iter().cloned().collect();
        self.probe_queue = self.tests.keys().cloned().collect();
        let mut out = self.next_fetches();
        out.extend(self.next_probes());
        out
    }

    fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Nip11 { relay, json } => {
                let relay = Self::norm(&relay);
                self.nip11
                    .insert(relay, json.as_deref().and_then(Nip11::parse));
                self.nip11_in_flight = self.nip11_in_flight.saturating_sub(1);
                self.next_fetches()
            }
            Input::Ok {
                relay,
                id,
                accepted,
                ..
            } => {
                let relay = Self::norm(&relay);
                if self.tests.get(&relay).is_some_and(|e| e.id == id) {
                    self.probes.entry(relay.clone()).or_default().accepted = Some(accepted);
                    if accepted {
                        self.accepted_at.insert(relay.clone(), self.now);
                    }
                }
                self.settle(&relay)
            }
            Input::Failed { relay, health } => {
                let relay = Self::norm(&relay);
                let probe = self.probes.entry(relay).or_default();
                match health {
                    crate::strategy::Health::Redirect => probe.redirected = true,
                    crate::strategy::Health::BadCertificate => probe.bad_certificate = true,
                    _ => {}
                }
                Vec::new()
            }
            Input::Event { relay, event } => {
                let relay = Self::norm(&relay);
                if self.tests.get(&relay).is_some_and(|t| t.id == event.id) {
                    match self.reading.get_mut(&relay) {
                        Some((_, found)) => *found = true,
                        None => {
                            self.live.insert(relay);
                        }
                    }
                }
                Vec::new()
            }
            Input::Eose { relay } => {
                let relay = Self::norm(&relay);
                let mut out = Vec::new();
                if let Some((delay, found)) = self.reading.remove(&relay) {
                    self.probes
                        .entry(relay.clone())
                        .or_default()
                        .read_back
                        .push((delay, found));
                    out.push(Action::Unsubscribe {
                        relay: relay.clone(),
                    });
                }
                out.extend(self.settle(&relay));
                out
            }
            Input::Tick { now } => {
                self.now = now;
                let over: Vec<String> = self
                    .probing
                    .iter()
                    .filter(|(r, since)| self.finished(r, **since))
                    .map(|(r, _)| r.clone())
                    .collect();
                for r in over {
                    self.probing.remove(&r);
                }
                let mut out = self.next_probes();
                let seen: HashSet<&String> = self.reading.keys().collect();
                let mut start = Vec::new();
                for (relay, at) in &self.accepted_at {
                    let done = self.probes.get(relay).map_or(0, |p| p.read_back.len());
                    if seen.contains(relay) || done >= self.delays.len() {
                        continue;
                    }
                    if now - at >= self.delays[done] {
                        start.push((relay.clone(), self.delays[done]));
                    }
                }
                for (relay, delay) in start {
                    let probe = self.probes.entry(relay.clone()).or_default();
                    if probe.live.is_none() {
                        probe.live = Some(self.live.contains(&relay));
                    }
                    let test = &self.tests[&relay];
                    out.push(Action::Subscribe {
                        relay: relay.clone(),
                        filter: Prober::filter(test, test.created_at - 1),
                    });
                    self.reading.insert(relay, (delay, false));
                }
                out
            }
        }
    }
}
