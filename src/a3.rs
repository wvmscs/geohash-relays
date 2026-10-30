// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A3`: relays announced in presence events, self-organisation
//! (`docs/geohash-relays-requirements-and-alternatives.md`,
//! §5 A3; decision G9 of `PLAN_ET1.md`).
//!
//! Each presence (kind 20001) also names the relays its author writes to:
//! `["relay", url]`, at most [`A3_MAX_TAGS`] (not `r`: the iOS mesh bridge
//! uses it). A reader adds a relay to its set when [`A3_MIN_KEYS`] distinct
//! keys of the cell announce it, in presences with a proof of work of
//! [`A3_MIN_POW_BITS`] (fake keys cost something: Sybil, Annex A.4) and
//! recent ones ([`A3_ANNOUNCE_TTL_SECS`]). The cell converges on a shared
//! set even when the initial lists differ.
//!
//! Upward compatible: the apps read only the `g`, `n` and `t` tags of a
//! presence (iOS `NostrInboundPipeline.subscribeNostrEvent`; Android
//! `GeohashMessageHandler.kt`, commit `20501fdfe0`): the extra tags are
//! ignored. A first shared relay is still needed to start: the base
//! strategy (A1, or `AS`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::directory::normalize_url;
use crate::error::Result;
use crate::params::{
    A3_ANNOUNCE_TTL_SECS, A3_MAX_TAGS, A3_MIN_KEYS, A3_MIN_POW_BITS, A3_TAG, KIND_PRESENCE,
};
use crate::sansio::EventMeta;
use crate::strategy::{GeoRelay, Origin, Role, Selected, Strategy};

/// What this strategy asks of a candidate relay: A3 takes the relays the cell's peers announce.
pub const NEEDS: crate::strategy::Needs = crate::strategy::Needs { place: false };

/// The tags a presence adds to announce `relays` (normalised, distinct, at
/// most [`A3_MAX_TAGS`]).
pub fn announce_tags(relays: &[String]) -> Vec<Vec<String>> {
    let mut seen = BTreeSet::new();
    relays
        .iter()
        .filter_map(|r| normalize_url(r))
        .filter(|r| seen.insert(r.clone()))
        .take(A3_MAX_TAGS)
        .map(|r| vec![A3_TAG.to_owned(), r])
        .collect()
}

/// The announcements seen, per cell and key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresenceBook {
    /// cell -> key -> (relays, when).
    cells: BTreeMap<String, BTreeMap<String, (Vec<String>, i64)>>,
}

impl PresenceBook {
    /// Notes a verified presence of `cell`. Returns whether the book
    /// changed. Left aside: another kind or cell, too little proof of work,
    /// no announcement.
    pub fn record(&mut self, cell: &str, e: &EventMeta) -> bool {
        if e.kind != KIND_PRESENCE || e.tag("g") != Some(cell) || e.pow_bits() < A3_MIN_POW_BITS {
            return false;
        }
        let relays: Vec<String> = e
            .tag_values(A3_TAG)
            .filter_map(normalize_url)
            .take(A3_MAX_TAGS)
            .collect();
        if relays.is_empty() {
            return false;
        }
        let keys = self.cells.entry(cell.to_owned()).or_default();
        match keys.get(&e.pubkey) {
            Some((r, at))
                if *at >= e.created_at
                    || *r == relays && e.created_at - at < A3_ANNOUNCE_TTL_SECS / 2 =>
            {
                false
            }
            _ => {
                keys.insert(e.pubkey.clone(), (relays, e.created_at));
                true
            }
        }
    }

    /// Relays of `cell` announced by enough distinct recent keys, the most
    /// announced first: (relay, keys).
    pub fn announced(&self, cell: &str, now: i64) -> Vec<(String, usize)> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for (relays, at) in self.cells.get(cell).into_iter().flat_map(|k| k.values()) {
            if now - at > A3_ANNOUNCE_TTL_SECS {
                continue;
            }
            for r in relays {
                *count.entry(r.as_str()).or_default() += 1;
            }
        }
        let mut out: Vec<(String, usize)> = count
            .into_iter()
            .filter(|(_, n)| *n >= A3_MIN_KEYS)
            .map(|(r, n)| (r.to_owned(), n))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// Forgets the announcements older than the TTL.
    pub fn prune(&mut self, now: i64) {
        for keys in self.cells.values_mut() {
            keys.retain(|_, (_, at)| now - *at <= A3_ANNOUNCE_TTL_SECS);
        }
        self.cells.retain(|_, k| !k.is_empty());
    }
}

/// `A3`: a base strategy plus the relays the cell announces.
#[derive(Clone)]
pub struct Announced {
    base: Arc<dyn Strategy>,
    book: PresenceBook,
    now: i64,
}

impl Announced {
    pub fn new(base: Arc<dyn Strategy>, book: PresenceBook, now: i64) -> Self {
        Announced { base, book, now }
    }

    fn add(&self, geohash: &str, mut out: Vec<Selected>) -> Vec<Selected> {
        for (url, _) in self.book.announced(geohash, self.now) {
            if !out
                .iter()
                .any(|s| normalize_url(&s.relay.url).as_deref() == Some(url.as_str()))
            {
                let rank = out.len() + 1;
                out.push(Selected::new(
                    GeoRelay::unplaced(url),
                    rank,
                    Role::ReadWrite,
                    Origin::Announced,
                ));
            }
        }
        out
    }
}

impl Strategy for Announced {
    fn name(&self) -> &str {
        "A3"
    }

    fn describe(&self) -> String {
        format!(
            "A3 (announced by {} keys, PoW {}) over {}",
            A3_MIN_KEYS,
            A3_MIN_POW_BITS,
            self.base.describe()
        )
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        Ok(self.add(geohash, self.base.read(geohash)?))
    }

    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        Ok(self.add(geohash, self.base.write(geohash)?))
    }
}
