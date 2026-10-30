# Changelog

## [Unreleased]

### Added (lot 13, branch `strategies`)
- `census`: `relay_url` (a public relay address from free text, normalised), `relays_of` (the
  relays of a NIP-65 relay list, kind 10002), and the `Crawler` machine (relay lists of a set
  of relays, 16 at a time, the newest of each author, `EOSE`, failure or 90 s). Run by
  geohash-relays-net's `relay-census`.
- `params`: `KIND_RELAY_LIST`, `CENSUS_*`, `comm::RESOLVE_TIMEOUT`, `LIST_MAX_AGE_SECS` (the
  apps download their list every 24 h).
- `strategy::Needs` and a `NEEDS` constant beside each strategy (whether it needs a place);
  `census::{STRATEGIES, Candidate, Exclusion, exclusion}`: why a strategy cannot use a relay
  (never read, no answer, not qualified, refused by A6, `ws://`, no place). Ports and paths
  are never a reason (the common rule keeps them).
- `select::haversine_km` is public (the census measures how far its places are from the
  lists').

### Changed (lot 0 of `PLAN_ET1.md`, branch `strategies`)
- Modules: `as_` (`AS`: `directory`, `select`, `AppStrategy`, also re-exported at the root
  under their old paths), `a0` to `a6` (the alternatives).
- `Selected` is generic: `rank`, `role` (`ReadWrite`, `WriteOnly`), `origin` (`Ios`,
  `Android`, `Both`, `List`, `Shared`, `Announced`); `ios` and `android` stay for `AS`.
  `Selected` and `GeoRelay` move to `strategy`.

### Added
- Lot 10: `docs/evaluation.md` (bench, real-network measures, proposal), README section per
  strategy. Lot 9: `params::A6_CONCURRENCY`, `params::comm::NIP11_TIMEOUT`.
- `a5` (lot 8): `Federation` (read 2 members, write 1, chosen per cell by HRW), `sync_filter`,
  `omissions`, `Comparer` machine; `docs/federation.md` (strfry router and sync, NIP-77).
- `a3` (lot 7): `announce_tags` (`["relay", url]`, at most 5), `PresenceBook` (2 distinct keys,
  proof of work 8, 10 min), `Announced` (a base strategy plus the announced relays).
  Compatibility checked in both apps (only `g`, `n`, `t` read in a presence).
- `a2` (lot 6): NIP-66 `Report` (kind 30166: `d`, `R`, `k`, `n`, `g`, NIP-11 content) and
  `Announcement` (kind 10166), checked against `66.md` of 2026-10-29; `Directory` (*m* of the
  trusted monitors, findings by majority, a shared `Qualification`, `to_list` for A1,
  `guard`: results removing more than half of a list are not used); `Watcher` machine.
- `a1` (lot 5): `HrwStrategy` (salted rendezvous hashing, k = 5, p = 4, epochs of a UTC day,
  writes to both epochs within an hour of a change), `score`, `Salts`, `epoch_of`; frozen
  score vectors (checked against Python `hashlib`), example `hrw_vectors`.
- `a6` (lot 4): `Nip11` (declaration), `Probe` (test results), `Verdict` (refusals R1, R2, R5,
  R6, R7, R9; warnings R8, R11, R13; a probe prevails), `Qualification` (a local filter only
  with A0: `SharedStrategy::qualified`; a shared one for any list), `Prober` (the probe
  machine). `RelayDirectory::retain`, `Error::LocalQualification`. `serde_json` dependency.
- `a0` (lot 3): `SharedStrategy` (one list, iOS rule, write 8 / read 5, failed relays
  replaced, shared relays), `ListVersion` (SHA-256 of the normalised lines). `sha2` dependency.
  The `compare` example compares strategies with two peers each.
- `a4` (lot 2): `Republisher`, the republisher machine (one pass per event id, only to the
  relays that did not send it, 3 s to settle, token bucket), `Plan::of`, `Counters`.
- `bench` (lot 1): rendezvous between two peers, load, stability between two lists, failures,
  capture cost; deterministic draws (`Rng`, `params::BENCH_SEED`). Example `compare`. Two
  older frozen lists (`tests/data/README.md`).
- `params`: every constant, with its source (G14); `GEO_RELAY_COUNT`, `MAX_PRECISION`, list
  URLs and default relays moved there.
- `sansio`: `EventMeta`, `FilterSpec`, `Input`, `Action`, `Machine`, the form of the
  strategies that talk to relays (G13).
- `Strategy::describe`, `AppStrategy::ios` and `AppStrategy::android`, `Health` and
  `RelayHealth`.
- `deny.toml` (licences compatible with `Unlicense OR 0BSD`).

### Added
- Relay lists with each app's rules (`RelayDirectory`, `Platform`, `normalize_url`), the list
  URLs and the apps' default relays (`Platform::list_url`, `Platform::default_relays`); ported
  from bitchat-geohash without the download (bitchat-console lot 1).
- The apps' relay choice (`closest`, `select_both`, `GEO_RELAY_COUNT`), same results as
  bitchat-geohash on the frozen lists.
- `geohash` module, without dependency: bounds, centre, encoding, the 8 neighbours as iOS
  computes them (nothing beyond the poles), `with_neighbours`.
- `Strategy` trait and `AppStrategy` (`AS`: both apps, iOS only or Android only).
- `README.md` for a future proposal to the apps.
- Crate skeleton: `Error` type, own `[workspace]`, Unlicense, `AGENTS.md` (bitchat-console
  lot 0).
