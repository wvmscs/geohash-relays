// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Sonar's relay choice for a location channel, as its code does it (not as
//! its comments say): [hedwig-corp/bitchat-to-sonar](https://github.com/hedwig-corp/bitchat-to-sonar)
//! (Unlicense), a bitchat-compatible client with a Rust core.
//!
//! Sources, commit `fdf091c` (2026-11-01; the relay code is the same in
//! release `v0.1-alpha.15.5`): `core/sonar-core/src/relay_directory.rs`
//! (`closest_relays_for_geohash`), `core/sonar-core/src/client.rs`
//! (`ensure_geohash_relays`, `subscribe_geohash`, `send_geohash`),
//! `apps/sonar/composeApp/src/commonMain/kotlin/chat/bitchat/sonar/RelayDiagnostics.kt`.
//!
//! - **List:** `core/sonar-core/src/relays_gps.csv`, compiled into the core
//!   (`include_str!`): a copy, made by hand on 2026-06-15 (`85629a8`), of the
//!   repository's `relays/online_relays_gps.csv`, which was then georelays'
//!   raw list of 2026-02-15. A weekly job (`fetch_georelays.yml`, Sundays
//!   06:00 UTC) keeps that file up to date; the core has not been copied
//!   from it again.
//! - **Reading:** every line as is, `wss://` put in front: no lowercasing,
//!   no `:443` removed, no duplicate removed, no host refused (Android's
//!   reading, [`Platform::Android`]).
//! - **Choice:** the [`SONAR_GEO_RELAY_COUNT`] (12) nearest to the cell's
//!   centre, sorted by distance only with a stable sort: ties keep the
//!   list's order (Android's rule, not iOS's).
//! - **Use:** they are added to the client's one relay pool, and a channel
//!   event is sent to, and subscribed on, every relay of the pool: the
//!   default relays ([`SONAR_DEFAULT_RELAYS`] when the app has nothing else)
//!   and the relays of every cell opened before, for the whole session
//!   (nothing removes them). The pool is not told to anyone: no event, no
//!   mesh message carries it. [`SonarStrategy::with_pool`] adds the default
//!   relays; [`with_history`](SonarStrategy::with_history) and
//!   [`with_levels`](SonarStrategy::with_levels) simulate the cells opened
//!   before.
//!
//! This is the core, used by the Android and desktop apps. Sonar's iOS app
//! still chooses with the bitchat iOS code it started from
//! (`ios/bitchat/Nostr/GeoRelayDirectory.swift`): 5 nearest, from georelays'
//! raw list fetched once a day; it is not modelled here.

use crate::directory::{Platform, RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::{CHANNEL_LEVELS, SONAR_DEFAULT_RELAYS, SONAR_GEO_RELAY_COUNT};
use crate::select::closest;
use crate::strategy::{GeoRelay, Origin, Role, Selected, Strategy};

/// Sonar: the 12 nearest relays of its compiled-in list, and its pool.
#[derive(Debug, Clone)]
pub struct SonarStrategy {
    list: RelayDirectory,
    pool: Vec<String>,
    history: Vec<String>,
    levels: bool,
}

impl SonarStrategy {
    /// `csv`: the list compiled into the core. The cell's 12 relays only.
    pub fn new(csv: &str) -> Self {
        SonarStrategy {
            list: RelayDirectory::parse(csv, Platform::Android),
            pool: Vec::new(),
            history: Vec::new(),
            levels: false,
        }
    }

    /// With the default relays of the pool, which carry the channel's
    /// events too.
    pub fn with_pool(mut self) -> Self {
        self.pool = SONAR_DEFAULT_RELAYS
            .iter()
            .map(|u| (*u).to_owned())
            .collect();
        self
    }

    /// Simulation: the user opened these cells before, in the same session;
    /// their 12 relays stay in the pool.
    pub fn with_history(mut self, cells: &[&str]) -> Self {
        self.history = cells.iter().map(|c| (*c).to_owned()).collect();
        self
    }

    /// Simulation: before a cell, the user opened the coarser channels of the
    /// same place ([`CHANNEL_LEVELS`] shorter than the cell), as the channel
    /// picker offers them.
    pub fn with_levels(mut self) -> Self {
        self.levels = true;
        self
    }

    pub fn list(&self) -> &RelayDirectory {
        &self.list
    }
}

impl Strategy for SonarStrategy {
    fn name(&self) -> &str {
        "AS"
    }

    fn describe(&self) -> String {
        let history = !self.history.is_empty() || self.levels;
        format!(
            "AS (Sonar, {SONAR_GEO_RELAY_COUNT} nearest{}{})",
            if self.pool.is_empty() { "" } else { " + pool" },
            if history {
                " + cells opened before"
            } else {
                ""
            }
        )
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        let mut out: Vec<Selected> = closest(&self.list, geohash, SONAR_GEO_RELAY_COUNT)?
            .into_iter()
            .enumerate()
            .map(|(i, r)| Selected::new(r, i + 1, Role::ReadWrite, Origin::List))
            .collect();
        let mut before: Vec<&str> = self.history.iter().map(String::as_str).collect();
        if self.levels {
            before.extend(
                CHANNEL_LEVELS
                    .iter()
                    .filter(|&&n| n < geohash.len())
                    .map(|&n| &geohash[..n]),
            );
        }
        let mut pooled: Vec<GeoRelay> = self.pool.iter().map(GeoRelay::unplaced).collect();
        for cell in before {
            pooled.extend(closest(&self.list, cell, SONAR_GEO_RELAY_COUNT)?);
        }
        // The pool, shared by every cell the user follows.
        for relay in pooled {
            let known = out
                .iter()
                .any(|s| normalize_url(&s.relay.url) == normalize_url(&relay.url));
            if !known {
                let rank = out.len() + 1;
                out.push(Selected::new(relay, rank, Role::ReadWrite, Origin::Shared));
            }
        }
        Ok(out)
    }
}
