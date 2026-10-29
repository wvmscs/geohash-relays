// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A0`: correct the current design and keep its principle
//! (`bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`,
//! §5 A0; decision G7 of `PLAN_ET1.md`).
//!
//! * **One list and one rule** for both apps: the iOS rule (one entry per
//!   host, sorted by distance then host, `GeoRelayDirectory.swift`), on one
//!   list with a version and a digest ([`ListVersion`]).
//! * **Write wide, read narrow**: write to the first *w* candidates of the
//!   shared order, read from the first *r* reachable ones (w = 8, r = 5).
//! * **Failed relays replaced** by the next ones in the order, for reading
//!   ([`RelayHealth`]): safe together with "write wide", since two peers
//!   that lost fewer than *r* different relays among the first *w* still
//!   share one.
//! * **Shared relays** (`includeDefaults` of Android's
//!   `NostrRelayManager.kt`, unused there): 0 to 2 relays added to every
//!   cell, which always meet.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::directory::{Platform, RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::{A0_READ, A0_SHARED_CANDIDATES, A0_SHARED_MAX, A0_WRITE};
use crate::select::closest;
use crate::strategy::{GeoRelay, Origin, RelayHealth, Role, Selected, Strategy};

/// Version and digest of the shared list. The digest is SHA-256 of the
/// normalised lines (trimmed, LF, header and blank lines left out), so that
/// the same list gives the same digest whatever its line endings. The
/// signature of the list is the host's business (it has the keys).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListVersion {
    pub version: u64,
    /// Lowercase hex.
    pub sha256: String,
}

impl ListVersion {
    pub fn of(version: u64, csv: &str) -> Self {
        let mut h = Sha256::new();
        for line in csv.lines().map(str::trim) {
            if line.is_empty() || line.to_lowercase().starts_with("relay url") {
                continue;
            }
            h.update(line.as_bytes());
            h.update(b"\n");
        }
        let mut sha256 = String::with_capacity(64);
        for b in h.finalize() {
            let _ = write!(sha256, "{b:02x}");
        }
        ListVersion { version, sha256 }
    }
}

/// `A0`: one list, the iOS rule, write wide, read narrow.
#[derive(Debug, Clone)]
pub struct SharedStrategy {
    list: RelayDirectory,
    version: Option<ListVersion>,
    write: usize,
    read: usize,
    shared: Vec<String>,
    health: RelayHealth,
}

impl SharedStrategy {
    /// On `csv`, read with the iOS rule, w = 8, r = 5, no shared relay.
    pub fn new(csv: &str) -> Self {
        SharedStrategy {
            list: RelayDirectory::parse(csv, Platform::Ios),
            version: None,
            write: A0_WRITE,
            read: A0_READ,
            shared: Vec::new(),
            health: RelayHealth::new(),
        }
    }

    /// The same list, already read with the iOS rule.
    pub fn from_directory(list: RelayDirectory) -> Self {
        let mut s = SharedStrategy::new("");
        s.list = list;
        s
    }

    pub fn with_version(mut self, version: ListVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Other *w* and *r* (`read` ≤ `write`).
    pub fn with_widths(mut self, write: usize, read: usize) -> Self {
        self.write = write.max(1);
        self.read = read.clamp(1, self.write);
        self
    }

    /// The first `n` of [`A0_SHARED_CANDIDATES`] as shared relays (at most
    /// [`A0_SHARED_MAX`]).
    pub fn with_shared(mut self, n: usize) -> Self {
        self.shared = A0_SHARED_CANDIDATES
            .iter()
            .take(n.min(A0_SHARED_MAX))
            .map(|s| (*s).to_owned())
            .collect();
        self
    }

    /// What the peer knows of failed relays (normalised URLs).
    pub fn with_health(mut self, health: RelayHealth) -> Self {
        self.health = health;
        self
    }

    /// Without the relays a qualification refuses (A6). A local one (this
    /// peer's own tests) is accepted: this strategy writes wide.
    pub fn qualified(mut self, q: &crate::a6::Qualification) -> Self {
        self.list = q.filter_any(self.list);
        self
    }

    pub fn version(&self) -> Option<&ListVersion> {
        self.version.as_ref()
    }

    fn order(&self, geohash: &str) -> Result<Vec<GeoRelay>> {
        let mut all = closest(&self.list, geohash, self.list.len())?;
        for r in &mut all {
            r.url = normalize_url(&r.url).unwrap_or_else(|| r.url.clone());
        }
        Ok(all)
    }

    fn alive(&self, url: &str) -> bool {
        !self.health.contains_key(url)
    }

    fn chosen(&self, geohash: &str) -> Result<(Vec<Selected>, Vec<Selected>)> {
        let order = self.order(geohash)?;
        let reading: Vec<Selected> = order
            .iter()
            .enumerate()
            .filter(|(_, r)| self.alive(&r.url))
            .take(self.read)
            .map(|(i, r)| Selected::new(r.clone(), i + 1, Role::ReadWrite, Origin::List))
            .collect();
        let mut writing = reading.clone();
        for (i, r) in order.iter().enumerate().take(self.write) {
            if self.alive(&r.url) && !writing.iter().any(|s| s.relay.url == r.url) {
                writing.push(Selected::new(
                    r.clone(),
                    i + 1,
                    Role::WriteOnly,
                    Origin::List,
                ));
            }
        }
        writing.sort_by_key(|s| s.rank);
        let base = order.len();
        let add = |out: &mut Vec<Selected>| {
            for (j, url) in self.shared.iter().enumerate() {
                if self.alive(url) && !out.iter().any(|s| &s.relay.url == url) {
                    out.push(Selected::new(
                        GeoRelay::unplaced(url.clone()),
                        base + j + 1,
                        Role::ReadWrite,
                        Origin::Shared,
                    ));
                }
            }
        };
        let (mut reading, mut writing) = (reading, writing);
        add(&mut reading);
        add(&mut writing);
        Ok((reading, writing))
    }
}

impl Strategy for SharedStrategy {
    fn name(&self) -> &str {
        "A0"
    }

    fn describe(&self) -> String {
        let v = self
            .version
            .as_ref()
            .map(|v| format!(", list v{} {}", v.version, &v.sha256[..8]))
            .unwrap_or_default();
        format!(
            "A0 (iOS rule, w = {}, r = {}, {} shared{v})",
            self.write,
            self.read,
            self.shared.len()
        )
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        Ok(self.chosen(geohash)?.0)
    }

    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        Ok(self.chosen(geohash)?.1)
    }
}
