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

The `Strategy` trait puts the choice behind one interface. A strategy is an immutable
snapshot: when what it knows changes, the host builds a new one.

| Module | Strategy |
|---|---|
| `as_` | `AS`: today's apps, iOS and Android, each on its own list (`AppStrategy`) |
| `a0` | one list and one rule, write wide and read narrow, failed relays replaced, shared relays |
| `a1` | salted rendezvous hashing (HRW) |
| `a2` | directory from NIP-66 monitors |
| `a3` | relays announced in presences |
| `a4` | republication between the iOS and Android relay sets |
| `a5` | synchronised federation (NIP-77), omissions |
| `a6` | relay qualification (NIP-11, probes) |

`params` holds every constant, with its source. Strategies that talk to relays are machines
without I/O (`sansio`); [geohash-relays-net](https://github.com/wvmscs/geohash-relays-net)
runs them with the communication of the iOS app, the Android app or bitchat-console.
The alternatives come from
`bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`; progress is in
`PLAN_ET1.md` (branch `strategies`).

## Why the two apps drift apart

iOS reads the reviewed `relays/online_relays_gps.csv` of the bitchat repository; Android reads
the daily, unreviewed `nostr_relays.csv` of georelays. The lists differ, and the rules differ
(deduplication, tie-break). For the same channel the two apps can choose different relays, and
then iOS and Android users do not see each other. Measured on real lists: see bitchat-geohash
and bitchat-envelope `Documents/relay-selection-corrections.md`.

## The strategies

Results on the frozen lists and on the real network: [docs/evaluation.md](docs/evaluation.md).

### `AS`: today's apps (`as_`)

Each app picks the 5 relays nearest to the centre of the cell, from its own list, with its own
rule (above). An iOS peer and an Android peer meet in 93 % of the cells of precision 2; 322 of
these 1,024 cells rest on 4 servers or fewer; five servers named to win iOS's alphabetical
tie-break take a cell.

### `A0`: one list, one rule, write wide (`a0::SharedStrategy`)

The iOS rule on one versioned list (`ListVersion`: SHA-256 of its lines); write to the first
8 candidates, read from the first 5 reachable ones; a failed relay (`RelayHealth`) is replaced
by the next one; 0 to 2 shared relays in every cell (`includeDefaults` of Android). Two peers
that lost fewer than 5 relays among the first 8 always meet; in the bench, every peer pair
meets, with lists a month apart and up to 3 dead relays. No resistance to capture.

### `A1`: salted rendezvous hashing (`a1::HrwStrategy`)

`SHA-256(salt ‖ 0 ‖ prefix(cell, 4) ‖ 0 ‖ url)`, the 5 highest scores; one salt per UTC day,
unknown in advance; writes to both days' relays within an hour of the change. Spreads the cells
over 332 servers (the busiest has 30 cells of 1,024), keeps more of the choice when a list
changes, and an iOS list and an Android list give nearly the same choice (98 %). Names ground
in advance are worthless with the salt. Frozen score vectors: `tests/a1.rs`,
`examples/hrw_vectors.rs`.

### `A2`: directory of NIP-66 monitors (`a2`)

Kind 30166 reports and kind 10166 announcements (format of `66.md`, 2026-10-29). A relay is a
candidate when *m* trusted monitors report it, the findings counted by majority; a shared
qualification for any list, guarded as NIP-66 asks (not used when it would remove more than
half of a list). `Watcher` reads the monitors.

### `A3`: relays announced in presences (`a3`)

`["relay", url]` tags in presences (at most 5); a relay announced by 2 distinct keys of the
cell, with 8 bits of proof of work, within 10 min, is added to the base strategy
(`Announced`). Both apps ignore these tags (they read `g`, `n`, `t`). Peers that miss each
other today meet once two keys on each side announce.

### `A4`: republishers (`a4::Republisher`)

Reads the union of the iOS and Android relays of each cell and publishes each event, once, on
the relays of its cell that did not send it, at most 60 per minute. Needs no change to the
apps.

### `A5`: federation of synchronised relays (`a5`)

`Federation` reads 2 members and writes to 1, chosen per cell; `Comparer` and `omissions` find
what a member lacks; [docs/federation.md](docs/federation.md) gives the strfry configuration
(router for live events, NIP-77 sync for stored ones).

### `A6`: relay qualification (`a6`)

`Verdict` per requirement of §3 from a NIP-11 document (`Nip11`) and a probe (`Probe`: live
delivery R9, then read-back R11), the probe prevailing; `Prober` runs probes, 16 at a time.
A filter only this peer applies is accepted only by A0 (`SharedStrategy::qualified`), a shared
one (A2) by any strategy.

## Tests

`cargo test`: the expected choices on the frozen lists of `tests/data/` are those of
bitchat-geohash and of bitchat-envelope's `geohash_map`, after the selection corrections of
27/10/2026. The neighbours are checked against the `geohash` crate.

## Licence

Unlicense OR 0BSD, at your choice: public domain like the BitChat iOS app, so that both apps can
take it, and 0BSD where a public-domain dedication does not hold (in France, for one). The
frozen relay lists in `tests/data/` keep their own licences (Unlicense, MIT). Every file says so with its SPDX tags ([REUSE](https://reuse.software)): a header, or `REUSE.toml`;
the licence texts are in `LICENSES/`.
