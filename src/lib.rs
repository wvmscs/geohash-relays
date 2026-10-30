// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Relay selection for BitChat location channels (geohash).
//!
//! A geohash channel has no server of its own: each app picks the relays
//! closest to the channel's centre from a relay list. This crate holds that
//! choice, isolated so that other strategies can replace it (see
//! `docs/geohash-relays-requirements-and-alternatives.md`):
//!
//! * [`as_`]: what the apps do today, iOS and Android rules (`AS`);
//! * [`a0`] to [`a6`]: the alternatives;
//! * [`census`]: the relays users name in their relay lists (NIP-65).
//!
//! Also the geohash arithmetic the apps use ([`geohash`]), every constant
//! ([`params`]), and the form of the strategies that talk to relays
//! ([`sansio`]). Pure and synchronous: the network is left to the caller
//! (`geohash-relays-net`).
//!
//! Written to be proposed to the BitChat apps; public domain (Unlicense, or
//! 0BSD), like the iOS app.

pub mod a0;
pub mod a1;
pub mod a2;
pub mod a3;
pub mod a4;
pub mod a5;
pub mod a6;
pub mod as_;
pub mod bench;
pub mod census;
mod error;
pub mod geohash;
pub mod params;
pub mod sansio;
pub mod strategy;

pub use as_::{AppStrategy, Apps, directory, select};
pub use directory::{DirectoryRelay, Platform, RelayDirectory, normalize_url};
pub use error::{Error, Result};
pub use params::GEO_RELAY_COUNT;
pub use select::{closest, select_both};
pub use strategy::{GeoRelay, Health, Origin, RelayHealth, Role, Selected, Strategy};
