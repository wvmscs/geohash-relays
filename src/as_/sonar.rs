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
//!   (`include_str!`): georelays' raw list of 2026-06-15. The weekly job of
//!   the repository refreshes another file (`relays/online_relays_gps.csv`),
//!   which the core does not read.
//! - **Reading:** every line as is, `wss://` put in front: no lowercasing,
//!   no `:443` removed, no duplicate removed, no host refused (Android's
//!   reading, [`Platform::Android`]).
//! - **Choice:** the [`SONAR_GEO_RELAY_COUNT`] (12) nearest to the cell's
//!   centre, sorted by distance only with a stable sort: ties keep the
//!   list's order (Android's rule, not iOS's).
//! - **Use:** they are added to the client's one relay pool, and a channel
//!   event is sent to, and subscribed on, every relay of the pool: the
//!   default relays ([`SONAR_DEFAULT_RELAYS`] when the app has nothing else)
//!   and the relays of every cell opened before. [`SonarStrategy::with_pool`]
//!   adds the default relays; the other cells depend on the user's history.

use crate::directory::{Platform, RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::{SONAR_DEFAULT_RELAYS, SONAR_GEO_RELAY_COUNT};
use crate::select::closest;
use crate::strategy::{GeoRelay, Origin, Role, Selected, Strategy};

/// Sonar: the 12 nearest relays of its compiled-in list, and its pool.
#[derive(Debug, Clone)]
pub struct SonarStrategy {
    list: RelayDirectory,
    pool: Vec<String>,
}

impl SonarStrategy {
    /// `csv`: the list compiled into the core. The cell's 12 relays only.
    pub fn new(csv: &str) -> Self {
        SonarStrategy {
            list: RelayDirectory::parse(csv, Platform::Android),
            pool: Vec::new(),
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

    pub fn list(&self) -> &RelayDirectory {
        &self.list
    }
}

impl Strategy for SonarStrategy {
    fn name(&self) -> &str {
        "AS"
    }

    fn describe(&self) -> String {
        format!(
            "AS (Sonar, {SONAR_GEO_RELAY_COUNT} nearest{})",
            if self.pool.is_empty() { "" } else { " + pool" }
        )
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        let mut out: Vec<Selected> = closest(&self.list, geohash, SONAR_GEO_RELAY_COUNT)?
            .into_iter()
            .enumerate()
            .map(|(i, r)| Selected::new(r, i + 1, Role::ReadWrite, Origin::List))
            .collect();
        for url in &self.pool {
            let known = out
                .iter()
                .any(|s| normalize_url(&s.relay.url) == normalize_url(url));
            if !known {
                let rank = out.len() + 1;
                out.push(Selected::new(
                    GeoRelay::unplaced(url),
                    rank,
                    Role::ReadWrite,
                    Origin::Shared,
                ));
            }
        }
        Ok(out)
    }
}
