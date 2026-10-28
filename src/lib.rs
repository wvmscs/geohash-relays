//! Relay selection for BitChat location channels (geohash).
//!
//! A geohash channel has no server of its own: each app picks the relays
//! closest to the channel's centre from a relay list. This crate holds that
//! choice, isolated so that other strategies can replace it (see
//! `bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`):
//!
//! * `AS`: what the apps do today, iOS and Android rules;
//! * `A0` to `A6`: the alternatives, later.
//!
//! Written to be proposed to the BitChat apps; public domain (Unlicense),
//! like the iOS app.

mod error;

pub use error::{Error, Result};
