// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Every constant of the strategies, and of the communication code of
//! `geohash-relays-net`, in one place (decision G14 of `PLAN_ET1.md`). Each
//! one says where it comes from: an app source (file and function), the
//! requirements document (`docs/geohash-relays-requirements-and-alternatives.md`,
//! `§` and `R` numbers), or
//! a decision of the plan (`G` numbers).

use std::time::Duration;

// ---------------------------------------------------------------- geohash

/// The geohash alphabet: base 32 without `a`, `i`, `l`, `o`.
pub const GEOHASH_ALPHABET: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Longest geohash accepted: 12 characters (a few centimetres).
pub const MAX_PRECISION: usize = 12;

/// Earth radius of the apps' haversine distance, in km
/// (`GeoRelayDirectory.swift`, `RelayDirectory.kt`).
pub const EARTH_RADIUS_KM: f64 = 6371.0;

// ---------------------------------------------------------------- Nostr kinds

/// Location channel message (ephemeral), §1.2.
pub const KIND_CHANNEL: u32 = 20000;
/// Presence (ephemeral), §1.2.
pub const KIND_PRESENCE: u32 = 20001;
/// Location and board notes (stored), §1.2.
pub const KIND_NOTE: u32 = 1;
/// Deletion (NIP-09), board note removal, §1.2.
pub const KIND_DELETION: u32 = 5;
/// Authentication (NIP-42).
pub const KIND_AUTH: u32 = 22242;
/// Relay discovery report of a monitor (NIP-66), A2.
pub const KIND_RELAY_DISCOVERY: u32 = 30166;
/// Monitor announcement (NIP-66), A2.
pub const KIND_MONITOR_ANNOUNCEMENT: u32 = 10166;
/// Relay list metadata (NIP-65): the relays a user reads and writes; the
/// census follows them (lot 13).
pub const KIND_RELAY_LIST: u32 = 10002;

// ---------------------------------------------------------------- AS

/// Relays an app writes a location-channel event to, and reads it from: the
/// closest to the channel centre (`closestRelays(toGeohash:count:)`,
/// `closestRelaysForGeohash`).
pub const GEO_RELAY_COUNT: usize = 5;

/// iOS list, as the app downloads it (`GeoRelayDirectory.swift`).
pub const IOS_LIST_URL: &str = "https://raw.githubusercontent.com/permissionlesstech/bitchat/refs/heads/main/relays/online_relays_gps.csv";
/// Android list: georelays' daily output, unreviewed (`RelayDirectory.kt`).
pub const ANDROID_LIST_URL: &str =
    "https://raw.githubusercontent.com/permissionlesstech/georelays/main/nostr_relays.csv";
/// Both apps download their list again after 24 h (iOS
/// `TransportConfig.swift`; Android `RelayDirectory.kt`, data older than
/// 24 h); a copy younger than this is current.
pub const LIST_MAX_AGE_SECS: i64 = 86_400;

/// iOS fixed relays (private messages): `builtInRelays` in
/// `bitchat/Nostr/NostrRelayManager.swift` (plus relays the user adds).
pub const IOS_DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://offchain.pub",
];
/// Android fixed relays: `DEFAULT_RELAYS` in `nostr/NostrRelayManager.kt`.
pub const ANDROID_DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://relay.primal.net",
    "wss://offchain.pub",
    "wss://nostr21.com",
];

// ---------------------------------------------------------------- A0 (G7)

/// "Write wide": candidates written to.
pub const A0_WRITE: usize = 8;
/// "Read narrow": reachable candidates read from.
pub const A0_READ: usize = 5;
/// Shared relays that can be added to every cell (`includeDefaults` of
/// Android), at most this many.
pub const A0_SHARED_MAX: usize = 2;
/// The candidates for shared relays: the default relays both apps have
/// (§1.1).
pub const A0_SHARED_CANDIDATES: &[&str] = &[
    "wss://relay.damus.io",
    "wss://relay.primal.net",
    "wss://offchain.pub",
];

/// A relay found failed is avoided this long, then tried again.
pub const A0_HEALTH_TTL_SECS: i64 = 600;

// ---------------------------------------------------------------- A1 (G8)

/// Relays kept per cell.
pub const A1_K: usize = GEO_RELAY_COUNT;
/// Prefix of the cell that is hashed: a cell and its neighbours usually share
/// it, hence their relays.
pub const A1_PREFIX: usize = 4;
/// One epoch (one salt) per UTC day.
pub const A1_EPOCH_SECS: i64 = 86_400;
/// Around an epoch change, writes go to both epochs' relays for this long on
/// each side.
pub const A1_OVERLAP_SECS: i64 = 3_600;

// ---------------------------------------------------------------- A2

/// A relay is a candidate when at least this many distinct monitors report
/// it (*m* of *n*).
pub const A2_MIN_MONITORS: usize = 2;
/// Reports older than this are ignored.
pub const A2_REPORT_MAX_AGE_SECS: i64 = 3 * 86_400;
/// NIP-66 risk mitigation: monitor results that would remove more than this
/// share of a list are discarded.
pub const A2_MAX_REMOVED_SHARE: f64 = 0.5;
/// Reports asked for per monitor relay at subscription.
pub const A2_HISTORY_LIMIT: u32 = 5_000;

// ---------------------------------------------------------------- A3 (G9)

/// Tag name of an announced relay (not `r`: the iOS mesh bridge uses it).
pub const A3_TAG: &str = "relay";
/// Announced relays per presence, at most (iOS accepts 64 tags per event).
pub const A3_MAX_TAGS: usize = 5;
/// Distinct keys of a cell that must announce a relay.
pub const A3_MIN_KEYS: usize = 2;
/// Proof of work a presence needs for its announcements to count (NIP-13).
pub const A3_MIN_POW_BITS: u32 = 8;
/// An announcement counts this long after its presence (presences come every
/// 40 to 80 s, `GeohashPresenceService.swift`).
pub const A3_ANNOUNCE_TTL_SECS: i64 = 600;

// ---------------------------------------------------------------- A4 (G10)

/// Kinds republished: channel messages, presences, notes, deletions.
pub const A4_KINDS: &[u32] = &[KIND_CHANNEL, KIND_PRESENCE, KIND_NOTE, KIND_DELETION];
/// Older events are not republished (the apps read `since: -1 h`).
pub const A4_MAX_AGE_SECS: i64 = 3_600;
/// Mesh bridge events (`r` tag) are rejected after 15 min by iOS.
pub const A4_BRIDGE_MAX_AGE_SECS: i64 = 900;
/// Republications per minute, at most (token bucket).
pub const A4_PER_MINUTE: u32 = 60;
/// Event ids remembered for deduplication.
pub const A4_SEEN_CAPACITY: usize = 10_000;
/// An event is republished this long after it is first seen: the relays
/// that already have it have time to send it too.
pub const A4_SETTLE_SECS: i64 = 3;
/// History asked for at subscription (the apps read `limit: 200`).
pub const A4_HISTORY_LIMIT: u32 = 200;

// ---------------------------------------------------------------- A5 (G11)

/// Kinds a federation synchronises (with a `g` tag).
pub const A5_KINDS: &[u32] = &[KIND_CHANNEL, KIND_PRESENCE, KIND_NOTE];
/// Federation members read from.
pub const A5_READ: usize = 2;
/// Federation members written to.
pub const A5_WRITE: usize = 1;
/// Window compared to find omissions.
pub const A5_WINDOW_SECS: i64 = 3_600;

// ---------------------------------------------------------------- A6

/// Highest proof of work a relay may require (R7: iOS mines 8 bits).
pub const A6_MAX_POW_BITS: u32 = 8;
/// Retention of kinds 20000 and 20001 a relay should give (R11).
pub const A6_MIN_EPHEMERAL_RETENTION_SECS: i64 = 3_600;
/// Subscriptions a connection must allow, at least (R8: more than 20).
pub const A6_MIN_SUBSCRIPTIONS: u64 = 21;
/// Message size a relay should accept, in bytes (R13).
pub const A6_MIN_MESSAGE_BYTES: u64 = 64 * 1024;
/// Delays after which a probe reads its test event back (§7, question 1).
pub const A6_PROBE_DELAYS_SECS: &[i64] = &[300, 3_600, 86_400];
/// The first read-back, right after the `OK` (R9: live subscribers and
/// storage of the ephemeral kinds).
pub const A6_PROBE_FIRST_READ_SECS: i64 = 5;
/// The cell probes write to: open sea (South Atlantic), nobody's channel.
pub const A6_PROBE_CELL: &str = "7zzzzzzz";
/// A probe gives up on a relay that answers nothing for this long.
pub const A6_PROBE_TIMEOUT_SECS: i64 = 30;
/// NIP-11 fetches, and probes, in flight at once (Tor builds one circuit per
/// relay: hundreds at once fail).
pub const A6_CONCURRENCY: usize = 16;

// ---------------------------------------------------------------- census (lot 13, G15)

/// Relay lists (kind 10002) asked of each relay, newest first.
pub const CENSUS_LIST_LIMIT: u32 = 500;
/// A relay that has not ended its stored events (`EOSE`) after this long is
/// left: many never answer (lot 9). Through Tor the circuit alone can take
/// up to a minute (`comm::CONNECT_TIMEOUT`), then up to
/// [`CENSUS_LIST_LIMIT`] lists come: 30 s left nos.lol out (29/10/2026).
pub const CENSUS_TIMEOUT_SECS: i64 = 90;
/// Relays read at once (one Tor circuit each, as for A6).
pub const CENSUS_CONCURRENCY: usize = 16;
/// A relay list dated further than this in the future is not valid.
pub const CENSUS_MAX_FUTURE_SECS: i64 = 900;
/// Rounds of a refresh at most: each reads the relays the previous one
/// found, until none is new.
pub const CENSUS_MAX_ROUNDS: usize = 8;
/// Precision of the geohash recorded for a relay (as the map's finest
/// level).
pub const CENSUS_GEOHASH_PRECISION: usize = 6;
/// A relay URL longer than this is not one (NIP-65 tags are free text).
pub const CENSUS_MAX_URL_BYTES: usize = 256;

// ---------------------------------------------------------------- comparison bench

/// Seed of the bench's deterministic draws (failures).
pub const BENCH_SEED: u64 = 0x6765_6f68_6173_6821;

// ---------------------------------------------------------------- communication

/// Constants of the communication code (`geohash-relays-net`), per
/// implementation.
pub mod comm {
    use super::Duration;

    /// Reconnection delays: from 1 s, doubled, up to 5 min (both apps,
    /// `NostrRelayManager.swift`, `NostrRelayManager.kt`; the console).
    pub const BACKOFF_MIN: Duration = Duration::from_secs(1);
    pub const BACKOFF_MAX: Duration = Duration::from_secs(300);
    /// iOS randomises each delay by ±20 %.
    pub const IOS_BACKOFF_JITTER: f64 = 0.2;
    /// iOS gives up after 10 attempts…
    pub const IOS_MAX_ATTEMPTS: u32 = 10;
    /// … and tries again after 10 min.
    pub const IOS_RETRY_AFTER_GIVING_UP: Duration = Duration::from_secs(600);
    /// iOS ends a history without `EOSE` after 10 s.
    pub const IOS_EOSE_TIMEOUT: Duration = Duration::from_secs(10);
    /// Wait for the `OK` of a publication (iOS waits 10 s for courier drops;
    /// all features wait for it, G13).
    pub const OK_TIMEOUT: Duration = Duration::from_secs(10);
    /// A SOCKS proxy must answer the start-up check this fast.
    pub const PROXY_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
    /// Opening a stream (through Tor: slow circuits).
    pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
    /// Publications kept to be sent again after a NIP-42 authentication.
    pub const RECENT_PUBLICATIONS: usize = 64;
    /// Largest relay list accepted, in bytes.
    pub const MAX_LIST_BYTES: usize = 2 * 1024 * 1024;
    /// `User-Agent` of the HTTP requests (relay lists, NIP-11).
    pub const HTTP_USER_AGENT: &str = "bitchat-console";
    /// A NIP-11 document must come within this time (A6; found in lot 9:
    /// some servers never answer, and hold a place).
    pub const NIP11_TIMEOUT: Duration = Duration::from_secs(20);
    /// Largest NIP-11 document accepted, in bytes.
    pub const MAX_NIP11_BYTES: usize = 64 * 1024;
    /// A host name must resolve within this time (census, lot 13; through
    /// Tor, a circuit first).
    pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);
    /// iOS receive limits (`NostrRelayManager.swift`): tags per event…
    pub const MAX_EVENT_TAGS: usize = 64;
    /// … values per tag…
    pub const MAX_EVENT_TAG_VALUES: usize = 16;
    /// … bytes per value.
    pub const MAX_EVENT_TAG_VALUE_BYTES: usize = 1024;
}
