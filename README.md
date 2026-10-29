# geohash-relays

Relay selection for BitChat location channels (geohash), isolated from any client.

A geohash channel has no server of its own. Each BitChat app picks, for every channel, the
5 relays nearest to the centre of the cell, from a relay list. All the peers of a channel must
make the same choice, or they do not see each other. This crate writes that choice down, with
the exact rules of each app, so that it can be tested, compared with alternatives, and shared.

## What it does

| Function | Rule | App source |
|---|---|---|
| `RelayDirectory::parse(csv, Platform::Ios)` | one entry per normalised host, first coordinates kept; a host at two places makes the list invalid (`conflict`) | `bitchat/Nostr/GeoRelayDirectory.swift` |
| `RelayDirectory::parse(csv, Platform::Android)` | every line kept as is (`host` and `host:443` are two entries) | `app/src/main/java/com/bitchat/android/nostr/RelayDirectory.kt` |
| `closest(dir, geohash, 5)` | iOS: sort by (distance, host); Android: stable sort by distance; exactly 5 | `closestRelays(toLat:lon:count:)`, `closestRelaysForGeohash` |
| `select_both(ios, android, geohash)` | union of the two choices, one entry per server, with each app's ranks | — |
| `geohash::neighbours(g)` | centre plus or minus one cell, longitude wraps, nothing beyond the poles | `Geohash.neighbors(of:)` (`bitchat/Protocols/Geohash.swift`) |
| `Platform::default_relays()` | the fixed relays of private messages | `builtInRelays`, `DEFAULT_RELAYS` |

Distances are great-circle (haversine, radius 6371 km) from the centre of the cell.

The `Strategy` trait puts the choice behind one interface: `AppStrategy` (`AS`, today's apps,
iOS, Android or both) now; the alternatives A0 to A6 of
[docs/geohash-relays-requirements-and-alternatives.md](docs/geohash-relays-requirements-and-alternatives.md) later.

## Why the two apps drift apart

iOS reads the reviewed `relays/online_relays_gps.csv` of the bitchat repository; Android reads
the daily, unreviewed `nostr_relays.csv` of georelays. The lists differ, and the rules differ
(deduplication, tie-break). For the same channel the two apps can choose different relays, and
then iOS and Android users do not see each other. Measured on real lists: see
[relay-selection-corrections.md](docs/relay-selection-corrections.md) and
[docs/geohash-relays-updates.md](docs/geohash-relays-updates.md).

## Tests

`cargo test`: the expected choices on the frozen lists of `tests/data/` are those of
bitchat-geohash and of an earlier prototype, after the selection corrections of 27/10/2026
([docs/relay-selection-corrections.md](docs/relay-selection-corrections.md)). The neighbours are checked against the `geohash` crate.

## Licence

Unlicense OR 0BSD, at your choice: public domain like the BitChat iOS app, so that both apps can
take it, and 0BSD where a public-domain dedication does not hold (in France, for one). The
frozen relay lists in `tests/data/` keep their own licences (Unlicense, MIT). Every file says so with its SPDX tags ([REUSE](https://reuse.software)): a header, or `REUSE.toml`;
the licence texts are in `LICENSES/`.
