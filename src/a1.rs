// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A1`: rendezvous hashing (HRW), salted, instead of distance
//! (`docs/geohash-relays-requirements-and-alternatives.md`,
//! §5 A1; decision G8 of `PLAN_ET1.md`).
//!
//! Each candidate gets a score for the cell; each peer keeps the *k* best:
//!
//! ```text
//! score(relay, cell) = SHA-256(salt ‖ 0x00 ‖ prefix(cell, p) ‖ 0x00 ‖ url)
//! ```
//!
//! read as a big-endian number (the first 8 bytes), the highest first, ties
//! by URL. `url` is normalised ([`normalize_url`]); `prefix(cell, p)` is
//! the first *p* characters (the whole cell when shorter). The zero bytes
//! keep the three fields apart, so that no two inputs hash the same text.
//!
//! * **Uniform**: a big city does not overload 5 servers.
//! * **Stable**: a relay that enters or leaves the list only moves the
//!   cells that chose it; two lists that differ a little give almost the
//!   same choice (the iOS/Android case).
//! * **Prefix**: a cell and its neighbours usually share their relays
//!   (location notes read 9 cells).
//! * **Salt**: one per epoch (a UTC day), unknown in advance (for example
//!   the hash of a Bitcoin block at a height fixed for the epoch): host
//!   names prepared to win a cell are worth nothing. Around an epoch change,
//!   writes go to both epochs' relays ([`A1_OVERLAP_SECS`]).
//!
//! Without a salt the choice can be ground offline: tests only.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::directory::{RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::{A1_EPOCH_SECS, A1_K, A1_OVERLAP_SECS, A1_PREFIX};
use crate::strategy::{GeoRelay, Origin, Role, Selected, Strategy};

/// What this strategy asks of a candidate relay: A1 scores the URL only (the place is shown, not used).
pub const NEEDS: crate::strategy::Needs = crate::strategy::Needs { place: false };

/// The score of `url` for `cell`: the first 8 bytes of the hash, big-endian.
pub fn score(salt: &[u8], cell: &str, prefix: usize, url: &str) -> u64 {
    let p = &cell[..cell.len().min(prefix)];
    let url = normalize_url(url).unwrap_or_else(|| url.to_owned());
    let mut h = Sha256::new();
    h.update(salt);
    h.update([0u8]);
    h.update(p.as_bytes());
    h.update([0u8]);
    h.update(url.as_bytes());
    let d = h.finalize();
    u64::from_be_bytes(d[..8].try_into().expect("8 bytes"))
}

/// The epoch of a time (Unix seconds).
pub fn epoch_of(now: i64) -> i64 {
    now.div_euclid(A1_EPOCH_SECS)
}

/// The salts the host knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Salts {
    /// No salt: the choice can be ground offline (tests only).
    None,
    /// One salt, whatever the epoch.
    Fixed(Vec<u8>),
    /// One salt per epoch (an epoch without one is not served).
    ByEpoch(BTreeMap<i64, Vec<u8>>),
}

impl Salts {
    fn get(&self, epoch: i64) -> Option<&[u8]> {
        match self {
            Salts::None => Some(&[]),
            Salts::Fixed(s) => Some(s),
            Salts::ByEpoch(m) => m.get(&epoch).map(Vec::as_slice),
        }
    }
}

/// `A1`: salted HRW on a candidate list.
#[derive(Debug, Clone)]
pub struct HrwStrategy {
    candidates: Vec<GeoRelay>,
    k: usize,
    prefix: usize,
    salts: Salts,
    now: i64,
}

impl HrwStrategy {
    /// On the relays of `list` (one per server), k = 5, p = 4, no salt, at
    /// time 0.
    pub fn new(list: &RelayDirectory) -> Self {
        let candidates = list
            .clone()
            .one_per_server()
            .relays()
            .iter()
            .filter_map(|r| {
                Some(GeoRelay {
                    url: normalize_url(&r.url)?,
                    lat: r.lat,
                    lon: r.lon,
                    km: f64::INFINITY,
                })
            })
            .collect();
        HrwStrategy {
            candidates,
            k: A1_K,
            prefix: A1_PREFIX,
            salts: Salts::None,
            now: 0,
        }
    }

    /// The salts, and the time the strategy is for.
    pub fn with_salts(mut self, salts: Salts, now: i64) -> Self {
        self.salts = salts;
        self.now = now;
        self
    }

    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k.max(1);
        self
    }

    pub fn with_prefix(mut self, prefix: usize) -> Self {
        self.prefix = prefix.max(1);
        self
    }

    /// The *k* best for `cell` with `salt`.
    fn best(&self, cell: &str, salt: &[u8]) -> Vec<GeoRelay> {
        let mut scored: Vec<(u64, &GeoRelay)> = self
            .candidates
            .iter()
            .map(|r| (score(salt, cell, self.prefix, &r.url), r))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.url.cmp(&b.1.url)));
        scored
            .into_iter()
            .take(self.k)
            .map(|(_, r)| r.clone())
            .collect()
    }

    /// The other epoch to write to, near a change.
    fn neighbour_epoch(&self) -> Option<i64> {
        let e = epoch_of(self.now);
        let into = self.now - e * A1_EPOCH_SECS;
        if into < A1_OVERLAP_SECS {
            Some(e - 1)
        } else if A1_EPOCH_SECS - into <= A1_OVERLAP_SECS {
            Some(e + 1)
        } else {
            None
        }
    }
}

impl Strategy for HrwStrategy {
    fn name(&self) -> &str {
        "A1"
    }

    fn describe(&self) -> String {
        let salt = match &self.salts {
            Salts::None => "no salt",
            Salts::Fixed(_) => "fixed salt",
            Salts::ByEpoch(_) => "salt per day",
        };
        format!("A1 (HRW, k = {}, p = {}, {salt})", self.k, self.prefix)
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        crate::geohash::bbox(geohash)?;
        let Some(salt) = self.salts.get(epoch_of(self.now)) else {
            return Ok(Vec::new());
        };
        Ok(self
            .best(geohash, salt)
            .into_iter()
            .enumerate()
            .map(|(i, r)| Selected::new(r, i + 1, Role::ReadWrite, Origin::List))
            .collect())
    }

    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        let mut out = self.read(geohash)?;
        if let Some(other) = self.neighbour_epoch().and_then(|e| self.salts.get(e))
            && !matches!(self.salts, Salts::None | Salts::Fixed(_))
        {
            for r in self.best(geohash, other) {
                if !out.iter().any(|s| s.relay.url == r.url) {
                    let rank = out.len() + 1;
                    out.push(Selected::new(r, rank, Role::WriteOnly, Origin::List));
                }
            }
        }
        Ok(out)
    }
}
