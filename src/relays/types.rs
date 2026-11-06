// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The values the entry point takes and gives.

use std::fmt;

use crate::directory::{Platform, normalize_url};
use crate::error::{Error, Result};
use crate::params::{ANDROID_LIST_URL, IOS_LIST_URL};

/// A valid geohash (lowercase, 1 to 12 characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell(String);

impl Cell {
    pub fn parse(geohash: &str) -> Result<Cell> {
        crate::geohash::bbox(geohash)?;
        Ok(Cell(geohash.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A relay address, normalised as iOS does
/// ([`normalize_url`]), kept without its scheme: `wss://` is implied.
/// [`Display`](fmt::Display) writes it in full. `ws://` (local test relays)
/// is kept as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelayUrl(String);

impl RelayUrl {
    pub fn parse(raw: &str) -> Result<RelayUrl> {
        let url = normalize_url(raw).ok_or_else(|| Error::InvalidRelayUrl(raw.to_owned()))?;
        Ok(RelayUrl(
            url.strip_prefix("wss://")
                .map_or(url.clone(), str::to_owned),
        ))
    }

    /// Without the scheme when it is `wss://`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The full address: `wss://host[:port][/path]`.
    pub fn url(&self) -> String {
        if self.0.starts_with("ws://") {
            self.0.clone()
        } else {
            format!("wss://{}", self.0)
        }
    }

    /// Where its NIP-11 document is: the same address over HTTPS (HTTP for
    /// `ws://`).
    pub fn nip11_url(&self) -> String {
        match self.0.strip_prefix("ws://") {
            Some(rest) => format!("http://{rest}"),
            None => format!("https://{}", self.0),
        }
    }
}

impl fmt::Display for RelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.url())
    }
}

/// A source of relay lists, and how to read them.
///
/// Signed lists (a key beside the address) are not decided yet: today's
/// sources publish unsigned CSV files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    pub url: String,
    pub platform: Platform,
}

impl Authority {
    /// The lists the apps read: iOS's reviewed list, georelays' daily list
    /// (Android).
    pub fn defaults() -> Vec<Authority> {
        vec![
            Authority {
                url: IOS_LIST_URL.to_owned(),
                platform: Platform::Ios,
            },
            Authority {
                url: ANDROID_LIST_URL.to_owned(),
                platform: Platform::Android,
            },
        ]
    }
}

/// How private or special relays join a cell's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    /// Beside the strategy's relays.
    Append,
    /// Instead of them.
    Replace,
}

/// What a connection saw of a relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Unreachable, or lost and not back.
    Dead,
    /// Requires NIP-42 authentication, or refused it.
    AuthRefused,
    /// Refused the events (payment, restricted writes, proof of work…).
    EventRefused,
    /// Works: forget what was seen before.
    Fine,
}

/// What a client does with a relay of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    ReadWrite,
    ReadOnly,
    WriteOnly,
}

impl Role {
    pub fn reads(self) -> bool {
        self != Role::WriteOnly
    }

    pub fn writes(self) -> bool {
        self != Role::ReadOnly
    }
}

/// A relay chosen for a cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Relay {
    pub url: RelayUrl,
    /// `(latitude, longitude)`; `None` for a relay without a place (shared
    /// or extra relays).
    pub place: Option<(f64, f64)>,
    /// 1-based, in the order of the list.
    pub rank: usize,
    pub role: Role,
}

/// The relays of a cell: one list, two views.
#[derive(Debug, Clone, PartialEq)]
pub struct CellRelays {
    pub cell: Cell,
    pub strategy: super::Strategy,
    pub relays: Vec<Relay>,
}

impl CellRelays {
    pub fn read(&self) -> impl Iterator<Item = &Relay> {
        self.relays.iter().filter(|r| r.role.reads())
    }

    pub fn write(&self) -> impl Iterator<Item = &Relay> {
        self.relays.iter().filter(|r| r.role.writes())
    }
}

/// A persistent file the host gives, read and rewritten whole.
pub trait StoredFile {
    fn read(&mut self) -> std::result::Result<Vec<u8>, CommError>;
    /// Replace the whole content, atomically.
    fn write(&mut self, bytes: &[u8]) -> std::result::Result<(), CommError>;
}

/// A failure of the host's means to talk; geohash-relays decides what it
/// means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommError(pub String);

impl fmt::Display for CommError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CommError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_urls_drop_the_implied_scheme() {
        let r = RelayUrl::parse("wss://Relay.Example.com:443/").unwrap();
        assert_eq!(r.as_str(), "relay.example.com");
        assert_eq!(r.to_string(), "wss://relay.example.com");
        assert_eq!(r.nip11_url(), "https://relay.example.com");
        assert_eq!(RelayUrl::parse("relay.example.com").unwrap(), r);
        let local = RelayUrl::parse("ws://127.0.0.1:7777").unwrap();
        assert_eq!(local.to_string(), "ws://127.0.0.1:7777");
        assert_eq!(local.nip11_url(), "http://127.0.0.1:7777");
        assert!(RelayUrl::parse("wss://").is_err());
    }

    #[test]
    fn cells_are_geohashes() {
        assert_eq!(Cell::parse("u09tv").unwrap().as_str(), "u09tv");
        assert!(Cell::parse("u0a").is_err());
        assert!(Cell::parse("").is_err());
    }
}
