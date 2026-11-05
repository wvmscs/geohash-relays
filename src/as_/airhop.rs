// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! AirHop's relay choice: a third client of the location channels, a React
//! Native port of bitchat ([areebahmeddd/airhop](https://github.com/areebahmeddd/airhop),
//! MIT), wire-compatible with it.
//!
//! Sources, release `v1.0.10` (2026-11-02): `src/core/nostr/geo-relay.ts`
//! (`GeoRelayDirectory.loadEntries`, `nearestRelays`,
//! `closestRelaysToGeohash`, `GEO_RELAY_COUNT`), `src/core/nostr/geo-relay-source.ts`,
//! `.github/workflows/sync-relays.yml`.
//!
//! - **List:** georelays' raw `nostr_relays.csv` (Android's source, not
//!   iOS's reviewed copy), vendored in the app at build time and never
//!   fetched at runtime; a weekly job proposes the new snapshot as a pull
//!   request, so a phone keeps the list of the release it runs.
//! - **Reading:** each address canonicalised (`validateRelayUrl`, ported
//!   from iOS: `host`, `host:443` and `wss://host/` are one), the first line
//!   of each kept: [`Platform::Ios`]'s reading of a list.
//! - **Choice:** the [`AIRHOP_GEO_RELAY_COUNT`] (10) nearest to the cell's
//!   centre, ties broken by URL, written to and read from: the 5 bitchat
//!   picks, then 5 backups that carry traffic too.

use crate::directory::{Platform, RelayDirectory};
use crate::error::Result;
use crate::params::AIRHOP_GEO_RELAY_COUNT;
use crate::select::closest;
use crate::strategy::{Origin, Role, Selected, Strategy};

/// AirHop: the 10 nearest relays of its vendored list.
#[derive(Debug, Clone)]
pub struct AirhopStrategy {
    list: RelayDirectory,
}

impl AirhopStrategy {
    /// `csv`: the list of the release (`assets/data/nostr_relays.csv`).
    pub fn new(csv: &str) -> Self {
        AirhopStrategy {
            list: RelayDirectory::parse(csv, Platform::Ios),
        }
    }

    /// From georelays' list as another client read it (Android keeps every
    /// line): read again with AirHop's rule.
    pub fn from_directory(list: &RelayDirectory) -> Self {
        let mut csv = String::from("Relay URL,Latitude,Longitude\n");
        for r in list.relays() {
            csv.push_str(&format!("{},{},{}\n", r.url, r.lat, r.lon));
        }
        Self::new(&csv)
    }

    pub fn list(&self) -> &RelayDirectory {
        &self.list
    }
}

impl Strategy for AirhopStrategy {
    fn name(&self) -> &str {
        "AS"
    }

    fn describe(&self) -> String {
        format!("AS (AirHop, {AIRHOP_GEO_RELAY_COUNT} nearest)")
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        Ok(closest(&self.list, geohash, AIRHOP_GEO_RELAY_COUNT)?
            .into_iter()
            .enumerate()
            .map(|(i, r)| Selected::new(r, i + 1, Role::ReadWrite, Origin::List))
            .collect())
    }
}
