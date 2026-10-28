//! Relay selection for BitChat location channels (geohash).
//!
//! A geohash channel has no server of its own: each app picks the relays
//! closest to the channel's centre from a relay list. This crate holds that
//! choice, isolated so that other strategies can replace it (see
//! `bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`):
//!
//! * `AS`: what the apps do today, iOS and Android rules ([`AppStrategy`]);
//! * `A0` to `A6`: the alternatives, later.
//!
//! Also the geohash arithmetic the apps use ([`geohash`]): centre, bounds,
//! the 8 neighbours. Pure and synchronous: reading the lists from the
//! network is left to the caller.
//!
//! Written to be proposed to the BitChat apps; public domain (Unlicense),
//! like the iOS app.

pub mod directory;
mod error;
pub mod geohash;
pub mod select;
pub mod strategy;

pub use directory::{DirectoryRelay, Platform, RelayDirectory, normalize_url};
pub use error::{Error, Result};
pub use select::{GeoRelay, Selected, closest, select_both};
pub use strategy::{AppStrategy, Apps, Strategy};

/// Relays an app writes a location-channel event to, and reads it from: the
/// closest to the channel centre, per app (Android may count one server
/// twice).
pub const GEO_RELAY_COUNT: usize = 5;
