# Changelog

## [Unreleased]

### Changed (lot 0 of `PLAN_ET1.md`, branch `strategies`)
- Modules: `as_` (`AS`: `directory`, `select`, `AppStrategy`, also re-exported at the root
  under their old paths), `a0` to `a6` (the alternatives).
- `Selected` is generic: `rank`, `role` (`ReadWrite`, `WriteOnly`), `origin` (`Ios`,
  `Android`, `Both`, `List`, `Shared`, `Announced`); `ios` and `android` stay for `AS`.
  `Selected` and `GeoRelay` move to `strategy`.

### Added
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
