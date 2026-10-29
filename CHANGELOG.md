# Changelog

## [Unreleased]

## [0.1.1] - 2026-10-29

Public opening: same code as 0.1.0.

### Added
- The study documents in `docs/` (requirements and alternatives, relay selection corrections,
  relay list updates, 2026-10-27), copied from an earlier, unpublished prototype.

### Changed
- README, `AGENTS.md`, rustdoc and comments cite only what a public reader can reach: the
  documents of `docs/`, GitHub URLs. Rules of the author's local checkout move to
  `AGENTS.local.md`, not committed.

## [0.1.0] - 2026-10-28

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
