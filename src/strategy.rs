// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! How a client picks the relays of a channel, behind one interface, so that
//! the apps' rule (`AS`) and the alternatives (`A0` to `A6`) can be swapped
//! and compared. See
//! `docs/geohash-relays-requirements-and-alternatives.md`.
//!
//! A strategy is an **immutable snapshot** (decision G2 of `PLAN_ET1.md`):
//! when its inputs change (relays found dead, presences seen, monitor
//! reports), the host builds a new one and swaps it in. No lock, no clock in
//! the crate.

use std::collections::HashMap;

use crate::error::Result;

/// A relay with its distance to a channel centre.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoRelay {
    pub url: String,
    pub lat: f64,
    pub lon: f64,
    /// Great-circle distance to the channel centre; `f64::INFINITY` when the
    /// relay has no known position (announced relays, A3).
    pub km: f64,
}

impl GeoRelay {
    /// A relay without a known position.
    pub fn unplaced(url: impl Into<String>) -> Self {
        GeoRelay {
            url: url.into(),
            lat: 0.0,
            lon: 0.0,
            km: f64::INFINITY,
        }
    }
}

/// What a client does with a chosen relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Read from and written to.
    ReadWrite,
    /// Written to only ("write wide", A0; epoch overlap, A1).
    WriteOnly,
}

/// Why a relay is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The iOS choice (`AS`).
    Ios,
    /// The Android choice (`AS`).
    Android,
    /// Both apps' choices (`AS`).
    Both,
    /// The strategy's own order on its list (A0, A1, A5).
    List,
    /// A relay shared by every cell (`includeDefaults`, A0).
    Shared,
    /// Announced in the presences of the cell (A3).
    Announced,
}

/// A server chosen for a channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Selected {
    /// The server (`url` normalised, see
    /// [`normalize_url`](crate::directory::normalize_url)).
    pub relay: GeoRelay,
    /// 1-based rank in the strategy's order.
    pub rank: usize,
    pub role: Role,
    pub origin: Origin,
    /// `AS` only: 1-based rank in the iOS choice.
    pub ios: Option<usize>,
    /// `AS` only: 1-based ranks in the Android choice: empty, one, or two
    /// when the list has the server twice (`host` and `host:443`).
    pub android: Vec<usize>,
}

impl Selected {
    /// A relay chosen by a strategy other than `AS`.
    pub fn new(relay: GeoRelay, rank: usize, role: Role, origin: Origin) -> Self {
        Selected {
            relay,
            rank,
            role,
            origin,
            ios: None,
            android: Vec::new(),
        }
    }
}

/// The relays of a channel.
/// What a strategy asks of a candidate relay, beyond the requirements every
/// relay must meet (A6): declared beside each strategy (`NEEDS`), counted by
/// the census (relays a strategy leaves out, lot 13).
///
/// Ports other than 443 and paths are never excluded: the common rule
/// ([`crate::normalize_url`]), which the Swift and Kotlin ports follow,
/// keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Needs {
    /// A place (latitude, longitude): the strategy ranks relays by distance.
    /// An `.onion` relay, or one not located, has none.
    pub place: bool,
}

pub trait Strategy: Send + Sync {
    /// Short name: `"AS"`, `"A0"`…
    fn name(&self) -> &str;

    /// The parameters, for display and for the comparison bench.
    fn describe(&self) -> String {
        self.name().to_owned()
    }

    /// Servers to read the channel from, best first.
    fn read(&self, geohash: &str) -> Result<Vec<Selected>>;

    /// Servers to write to. The same as [`read`](Self::read) unless the
    /// strategy writes wider than it reads (A0, A1 around an epoch change).
    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        self.read(geohash)
    }
}

/// What a client knows of a relay's health, from its connections (A0, A6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Unreachable, or lost and not back.
    Dead,
    /// Requires NIP-42 and refused the client.
    AuthRequired,
    /// Answered the WebSocket upgrade with a redirection (R2).
    Redirect,
    /// Its TLS certificate was refused (R1).
    BadCertificate,
    /// Refused the client's publications.
    WriteRefused,
}

/// Relays known to be unusable, by normalised URL. A relay not in the map is
/// assumed to work.
pub type RelayHealth = HashMap<String, Health>;
