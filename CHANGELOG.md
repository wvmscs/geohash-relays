# Changelog

## [Unreleased]

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
