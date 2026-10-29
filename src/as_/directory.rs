// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Relay lists (`host,lat,lon` CSV): parsing with each app's rules and
//! normalisation. Ported from bitchat-geohash (`src/directory.rs`, lots 1 and
//! 11 bis); downloading is left to the caller.

use std::collections::HashMap;

/// Which app reads a relay list: iOS reads bitchat's reviewed
/// `relays/online_relays_gps.csv`, Android georelays' daily `nostr_relays.csv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Platform {
    Ios,
    Android,
}

impl Platform {
    /// `"ios"` or `"android"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
        }
    }
}

/// A relay of a list, with the coordinates the list gives it (estimated from
/// its IPv4 address upstream, hence many relays on the same point).
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryRelay {
    /// The address as the app uses it: `wss://host` normalised for iOS, the
    /// raw text (with `wss://` added when there is no scheme) for Android.
    /// [`normalize_url`] gives the server it designates.
    pub url: String,
    pub lat: f64,
    pub lon: f64,
}

/// A parsed relay list, read with its app's rules. Parse once, then use it
/// for every channel.
#[derive(Debug, Clone)]
pub struct RelayDirectory {
    platform: Platform,
    relays: Vec<DirectoryRelay>,
    rejected: usize,
    duplicates: usize,
    conflict: Option<String>,
}

impl RelayDirectory {
    /// Parses a `host,lat,lon` list like `platform`'s app.
    ///
    /// Both: blank lines and header lines (starting with `relay url`, any
    /// case) are skipped; lines without a host, with fewer than 3 fields or
    /// with invalid coordinates are rejected and counted. Out-of-range
    /// coordinates are rejected too, although Android accepts them (decision
    /// D19 of bitchat-geohash).
    ///
    /// iOS keeps one entry per normalised host (`host`, `HOST:443` and
    /// `wss://host/` are one), with the first coordinates seen; later lines
    /// are counted as duplicates. A host listed at other coordinates is a
    /// [`conflict`](Self::conflict): iOS rejects such a list.
    ///
    /// Sources: iOS `bitchat/Nostr/GeoRelayDirectory.swift`; Android
    /// `app/src/main/java/com/bitchat/android/nostr/RelayDirectory.kt` (see bitchat-envelope
    /// `Documents/relay-selection-corrections.md`).
    ///
    /// Android keeps every line as is: no lowercasing, no `:443` removal, no
    /// deduplication, so `host` and `host:443` are two entries.
    pub fn parse(csv: &str, platform: Platform) -> Self {
        let mut dir = RelayDirectory {
            platform,
            relays: Vec::new(),
            rejected: 0,
            duplicates: 0,
            conflict: None,
        };
        let mut seen: HashMap<String, (f64, f64)> = HashMap::new();
        for line in csv.lines().map(str::trim) {
            if line.is_empty() || line.to_lowercase().starts_with("relay url") {
                continue;
            }
            let Some(relay) = parse_line(line, platform) else {
                dir.rejected += 1;
                continue;
            };
            if platform == Platform::Ios {
                match seen.get(&relay.url) {
                    Some(&at) => {
                        if at != (relay.lat, relay.lon) && dir.conflict.is_none() {
                            let host = relay.url.split_once("://").map_or("", |(_, h)| h);
                            dir.conflict = Some(host.to_owned());
                        }
                        dir.duplicates += 1;
                        continue;
                    }
                    None => {
                        seen.insert(relay.url.clone(), (relay.lat, relay.lon));
                    }
                }
            }
            dir.relays.push(relay);
        }
        dir
    }

    /// The app whose rules this list was read with.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Relays in list order.
    pub fn relays(&self) -> &[DirectoryRelay] {
        &self.relays
    }

    /// Lines that could not be read.
    pub fn rejected(&self) -> usize {
        self.rejected
    }

    /// iOS: lines dropped because their host was already listed. Always 0
    /// for Android.
    pub fn duplicates(&self) -> usize {
        self.duplicates
    }

    /// iOS: the first host listed at two different coordinates. The app
    /// rejects such a list and keeps its previous one. Only this cause of
    /// rejection is checked. Always `None` for Android.
    pub fn conflict(&self) -> Option<&str> {
        self.conflict.as_deref()
    }

    pub fn len(&self) -> usize {
        self.relays.len()
    }

    /// The same list without the relays `keep` refuses (A6, A2).
    pub fn retain(mut self, keep: impl Fn(&DirectoryRelay) -> bool) -> Self {
        self.relays.retain(|r| keep(r));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }
}

/// The server a relay address designates, as iOS keys it: `host`,
/// `wss://host`, `host/` and `host:443` are one server, case-insensitive.
/// Returns `wss://host` (`wss://host:port` for another port).
///
/// An explicit `ws://` is kept, for local test relays without TLS: the
/// published lists never contain one.
pub fn normalize_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("ws://") {
        let host = rest.trim_end_matches('/').to_lowercase();
        return (!host.is_empty()).then(|| format!("ws://{host}"));
    }
    let host = raw.split_once("://").map_or(raw, |(_, rest)| rest);
    let host = host.trim_end_matches('/');
    let host = host.strip_suffix(":443").unwrap_or(host).to_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(format!("wss://{host}"))
    }
}

fn parse_line(line: &str, platform: Platform) -> Option<DirectoryRelay> {
    let mut p = line.split(',').map(str::trim);
    let raw = p.next()?;
    let url = match platform {
        Platform::Ios => normalize_url(raw)?,
        // Android only adds a missing scheme. An empty host is still
        // rejected: nothing to connect to.
        Platform::Android => {
            normalize_url(raw)?;
            if raw.contains("://") {
                raw.to_owned()
            } else {
                format!("wss://{raw}")
            }
        }
    };
    let lat: f64 = p.next()?.parse().ok()?;
    let lon: f64 = p.next()?.parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some(DirectoryRelay { url, lat, lon })
}

pub use crate::params::{ANDROID_LIST_URL, IOS_LIST_URL};

impl Platform {
    /// The URL of the list the app downloads.
    pub fn list_url(self) -> &'static str {
        match self {
            Platform::Ios => IOS_LIST_URL,
            Platform::Android => ANDROID_LIST_URL,
        }
    }

    /// The app's fixed relays, used for private messages (not for location
    /// channels). iOS: `builtInRelays` in `bitchat/Nostr/NostrRelayManager.swift`
    /// (plus relays the user adds); Android: `DEFAULT_RELAYS` in
    /// `nostr/NostrRelayManager.kt`.
    pub fn default_relays(self) -> &'static [&'static str] {
        match self {
            Platform::Ios => crate::params::IOS_DEFAULT_RELAYS,
            Platform::Android => crate::params::ANDROID_DEFAULT_RELAYS,
        }
    }
}
