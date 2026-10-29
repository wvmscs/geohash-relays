// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A2`: decentralised directory from monitors (NIP-66)
//! (`bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`,
//! §5 A2).
//!
//! Independent monitors publish, for each relay, a kind 30166 report (`d`:
//! the relay URL; `R`: requirements `auth`, `writes`, `pow`, `payment`,
//! `!` for false; `k`: kinds accepted or, with `!`, refused; `n`: network;
//! `g`: geohash; content: the NIP-11 document, optional), and announce
//! themselves with kind 10166 (`frequency`, `c` checks). Format checked
//! against `nostr-protocol/nips` `66.md` on 2026-10-29.
//!
//! A relay is a candidate when *m* of the trusted monitors report it
//! recently ([`A2_MIN_MONITORS`], [`A2_REPORT_MAX_AGE_SECS`]), and the
//! majority of them do not find it failing the requirements of §3 (A6
//! checks). The result is a list for A1 (or A0), and a **shared**
//! qualification (every peer that trusts the same monitors gets the same
//! one) for any list. As NIP-66 asks, a result that would remove more than
//! half of a list is not used ([`Directory::guard`]).
//!
//! [`Watcher`] is the machine that subscribes to the monitors' reports.

use std::collections::{BTreeMap, BTreeSet};

use crate::a6::{Finding, Nip11, Qualification, Verdict};
use crate::directory::{Platform, RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::{
    A2_HISTORY_LIMIT, A2_MAX_REMOVED_SHARE, A2_MIN_MONITORS, A2_REPORT_MAX_AGE_SECS, KIND_CHANNEL,
    KIND_MONITOR_ANNOUNCEMENT, KIND_PRESENCE, KIND_RELAY_DISCOVERY,
};
use crate::sansio::{Action, EventMeta, FilterSpec, Input, Machine};

/// A monitor's report on a relay (kind 30166).
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub monitor: String,
    pub relay: String,
    pub created_at: i64,
    /// `R` tags: requirement -> required.
    pub requirements: BTreeMap<String, bool>,
    /// `k` tags: kinds accepted, refused.
    pub accepted_kinds: BTreeSet<u32>,
    pub refused_kinds: BTreeSet<u32>,
    /// `n`: `clearnet`, `tor`…
    pub network: Option<String>,
    /// `g`: where the relay is.
    pub geohash: Option<String>,
    /// The NIP-11 document in the content, if any.
    pub nip11: Option<Nip11>,
}

impl Report {
    /// Reads a kind 30166 event (its signature already checked).
    pub fn parse(e: &EventMeta, content: &str) -> Option<Report> {
        if e.kind != KIND_RELAY_DISCOVERY {
            return None;
        }
        let relay = normalize_url(e.tag("d")?)?;
        let mut r = Report {
            monitor: e.pubkey.clone(),
            relay,
            created_at: e.created_at,
            requirements: BTreeMap::new(),
            accepted_kinds: BTreeSet::new(),
            refused_kinds: BTreeSet::new(),
            network: e.tag("n").map(str::to_owned),
            geohash: e
                .tag_values("g")
                .filter(|g| crate::geohash::bbox(g).is_ok())
                .max_by_key(|g| g.len())
                .map(str::to_owned),
            nip11: Nip11::parse(content),
        };
        for v in e.tag_values("R") {
            match v.strip_prefix('!') {
                Some(name) => r.requirements.insert(name.to_owned(), false),
                None => r.requirements.insert(v.to_owned(), true),
            };
        }
        for v in e.tag_values("k") {
            match v.strip_prefix('!') {
                Some(k) => k.parse().ok().map(|k| r.refused_kinds.insert(k)),
                None => v.parse().ok().map(|k| r.accepted_kinds.insert(k)),
            };
        }
        Some(r)
    }

    /// What this report finds failing (A6 findings).
    pub fn verdict(&self) -> Verdict {
        let mut v = Verdict::of(self.nip11.as_ref(), None);
        let req = |k: &str| self.requirements.get(k).copied().unwrap_or(false);
        let mut refuse = |f: Finding| {
            if !v.refusals.contains(&f) {
                v.refusals.push(f);
            }
        };
        if req("auth") {
            refuse(Finding::AuthRequired);
        }
        if req("payment") {
            refuse(Finding::Payment);
        }
        if req("writes") {
            refuse(Finding::RestrictedWrites);
        }
        // `pow` without a level: refused only when the NIP-11 document says
        // more than 8 bits (already in the verdict, R7).
        if [KIND_CHANNEL, KIND_PRESENCE]
            .iter()
            .any(|k| self.refused_kinds.contains(k))
        {
            refuse(Finding::Kind20000);
        }
        v
    }
}

/// A monitor's announcement (kind 10166).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub monitor: String,
    pub frequency_secs: Option<i64>,
    pub checks: BTreeSet<String>,
}

impl Announcement {
    pub fn parse(e: &EventMeta) -> Option<Announcement> {
        (e.kind == KIND_MONITOR_ANNOUNCEMENT).then(|| Announcement {
            monitor: e.pubkey.clone(),
            frequency_secs: e.tag("frequency").and_then(|f| f.parse().ok()),
            checks: e.tag_values("c").map(str::to_owned).collect(),
        })
    }
}

/// A relay the monitors agree on.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub relay: String,
    /// Distinct trusted monitors that report it.
    pub monitors: usize,
    /// Where they put it (most reported geohash), if anywhere.
    pub geohash: Option<String>,
}

/// The directory of the trusted monitors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Directory {
    pub candidates: Vec<Candidate>,
    /// The shared verdicts: every relay reported by enough monitors.
    pub qualification: Qualification,
}

impl Directory {
    /// From `reports`, those of `trusted` monitors only, at time `now`, with
    /// at least `min_monitors` (*m*) of them per relay.
    pub fn build(
        reports: &[Report],
        trusted: &BTreeSet<String>,
        now: i64,
        min_monitors: usize,
    ) -> Directory {
        // The latest report of each (relay, monitor).
        let mut latest: BTreeMap<(&str, &str), &Report> = BTreeMap::new();
        for r in reports {
            if !trusted.contains(&r.monitor) || now - r.created_at > A2_REPORT_MAX_AGE_SECS {
                continue;
            }
            let key = (r.relay.as_str(), r.monitor.as_str());
            if latest
                .get(&key)
                .is_none_or(|old| old.created_at < r.created_at)
            {
                latest.insert(key, r);
            }
        }
        let mut by_relay: BTreeMap<&str, Vec<&Report>> = BTreeMap::new();
        for ((relay, _), r) in latest {
            by_relay.entry(relay).or_default().push(r);
        }
        let mut dir = Directory {
            qualification: Qualification {
                shared: true,
                ..Qualification::default()
            },
            ..Directory::default()
        };
        for (relay, rs) in by_relay {
            if rs.len() < min_monitors.max(1) {
                continue;
            }
            // A finding counts when most of the monitors make it.
            let verdicts: Vec<Verdict> = rs.iter().map(|r| r.verdict()).collect();
            let mut agreed = Verdict {
                probed: true,
                ..Verdict::default()
            };
            for v in &verdicts {
                for f in &v.refusals {
                    let n = verdicts.iter().filter(|w| w.refusals.contains(f)).count();
                    if 2 * n > verdicts.len() && !agreed.refusals.contains(f) {
                        agreed.refusals.push(f.clone());
                    }
                }
            }
            let mut places: BTreeMap<&str, usize> = BTreeMap::new();
            for r in &rs {
                if let Some(g) = &r.geohash {
                    *places.entry(g.as_str()).or_default() += 1;
                }
            }
            let geohash = places
                .iter()
                .max_by_key(|(_, n)| **n)
                .map(|(g, _)| (*g).to_owned());
            if agreed.usable() {
                dir.candidates.push(Candidate {
                    relay: relay.to_owned(),
                    monitors: rs.len(),
                    geohash,
                });
            }
            dir.qualification.verdicts.insert(relay.to_owned(), agreed);
        }
        dir
    }

    /// The candidates as a list (iOS rule), at the centre of their geohash;
    /// without one, at (0, 0) (fine for A1, which ignores distance).
    pub fn to_list(&self) -> RelayDirectory {
        let mut csv = String::from("Relay URL,Latitude,Longitude\n");
        for c in &self.candidates {
            let (lat, lon) = c
                .geohash
                .as_deref()
                .and_then(|g| crate::geohash::center(g).ok())
                .unwrap_or((0.0, 0.0));
            csv.push_str(&format!("{},{lat},{lon}\n", c.relay));
        }
        RelayDirectory::parse(&csv, Platform::Ios)
    }

    /// NIP-66 risk mitigation: `Ok(list filtered)` unless the monitors would
    /// remove more than half of it, then `Ok(list)` unchanged and `false`.
    pub fn guard(&self, list: RelayDirectory) -> Result<(RelayDirectory, bool)> {
        let before = list.len();
        let filtered = self.qualification.filter(list.clone())?;
        let removed = before.saturating_sub(filtered.len());
        if before > 0 && removed as f64 / before as f64 > A2_MAX_REMOVED_SHARE {
            return Ok((list, false));
        }
        Ok((filtered, true))
    }
}

/// The machine that reads the monitors: subscribes to their reports and
/// announcements on `relays`, keeps them.
pub struct Watcher {
    relays: Vec<String>,
    trusted: BTreeSet<String>,
    reports: BTreeMap<(String, String), Report>,
    announcements: BTreeMap<String, Announcement>,
    eose: BTreeSet<String>,
    now: i64,
}

impl Watcher {
    pub fn new(relays: Vec<String>, trusted: BTreeSet<String>) -> Self {
        Watcher {
            relays,
            trusted,
            reports: BTreeMap::new(),
            announcements: BTreeMap::new(),
            eose: BTreeSet::new(),
            now: 0,
        }
    }

    /// Every monitor relay sent its stored reports, or failed.
    pub fn caught_up(&self) -> bool {
        self.relays.iter().all(|r| {
            self.eose
                .contains(&normalize_url(r).unwrap_or_else(|| r.clone()))
        })
    }

    pub fn reports(&self) -> Vec<Report> {
        self.reports.values().cloned().collect()
    }

    pub fn announcements(&self) -> impl Iterator<Item = &Announcement> {
        self.announcements.values()
    }

    /// The directory of what came so far.
    pub fn directory(&self, min_monitors: usize) -> Directory {
        Directory::build(&self.reports(), &self.trusted, self.now, min_monitors)
    }

    /// The default threshold.
    pub fn default_directory(&self) -> Directory {
        self.directory(A2_MIN_MONITORS)
    }
}

impl Machine for Watcher {
    fn start(&mut self, now: i64) -> Vec<Action> {
        self.now = now;
        self.relays
            .iter()
            .map(|r| Action::Subscribe {
                relay: r.clone(),
                filter: FilterSpec {
                    kinds: vec![KIND_RELAY_DISCOVERY, KIND_MONITOR_ANNOUNCEMENT],
                    authors: self.trusted.iter().cloned().collect(),
                    since: Some(now - A2_REPORT_MAX_AGE_SECS),
                    limit: Some(A2_HISTORY_LIMIT),
                    ..FilterSpec::default()
                },
            })
            .collect()
    }

    fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Event { event, .. } if self.trusted.contains(&event.pubkey) => {
                if let Some(a) = Announcement::parse(&event) {
                    self.announcements.insert(a.monitor.clone(), a);
                } else {
                    let content = serde_json::from_str::<serde_json::Value>(&event.raw)
                        .ok()
                        .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_owned))
                        .unwrap_or_default();
                    if let Some(r) = Report::parse(&event, &content) {
                        let key = (r.relay.clone(), r.monitor.clone());
                        if self
                            .reports
                            .get(&key)
                            .is_none_or(|old| old.created_at < r.created_at)
                        {
                            self.reports.insert(key, r);
                        }
                    }
                }
            }
            // A relay that sent its stored reports, or cannot be reached, is
            // done.
            Input::Eose { relay } | Input::Failed { relay, .. } => {
                self.eose.insert(normalize_url(&relay).unwrap_or(relay));
            }
            Input::Tick { now } => self.now = now,
            _ => {}
        }
        Vec::new()
    }
}
