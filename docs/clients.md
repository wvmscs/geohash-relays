# The clients of the location channels (2026-11-05)

Who writes to the geohash channels today, how each client chooses its relays, and whether their
users meet. It updates [geohash-relays-updates.md](geohash-relays-updates.md) and
[relay-selection-corrections.md](relay-selection-corrections.md) (2026-10-27) to the latest
releases, and adds the clients that appeared since. Reproduce the numbers with
`cargo run --release --example clients`; the lists are frozen in `tests/data` (see its README).

## Summary

- **bitchat iOS** and **bitchat Android** choose their relays as on 2026-10-27: nothing changed
  in their relay choice, in the releases (iOS `v1.8.0`, Android `v2.2.0`) nor on their `main`.
- **iOS still reads a list of 2026-07-26.** Of its 326 relays, **164** were found working by the
  census of 2026-10-30: a cell gets **2.1 working relays** out of 5 on average.
- **AirHop**, a third client wire-compatible with bitchat, chooses **10** relays (the 5 bitchat
  picks, then 5 backups), from georelays' raw list frozen in each release.
- **B2 (BitChat II)** does not choose relays by geohash: it uses the relays its owner
  configures.
- Users of iOS and Android meet in **95 %** of the cells when every relay works, but in only
  **67 %** on the relays that actually answered. AirHop meets Android everywhere (same source,
  twice as many relays) and iOS in 87 to 92 % of the cells.

## 1. The clients

| Client | Version studied | List | Read | Choice |
|---|---|---|---|---|
| bitchat iOS | release `v1.8.0` (2026-10-23) = `main` `04995f68e4` | bitchat's reviewed `relays/online_relays_gps.csv`, fetched from `main` at runtime; last changed 2026-07-26 (`fb8fe39713`) | one entry per normalised host | 5 nearest the cell's centre, ties by host |
| bitchat Android | release `v2.2.0` (2026-10-28); `main` `998f168487` (2026-11-01) | georelays' raw `nostr_relays.csv`, fetched at runtime once a day; a copy in the app for the first launch | every line as is (`host` and `host:443` are two) | 5 nearest, ties in list order (stable sort) |
| AirHop | release `v1.0.10` (2026-11-02) = `main` | georelays' raw `nostr_relays.csv`, **frozen in the app** at build time, never fetched; a weekly job proposes the new one as a pull request | one entry per canonical address (rule ported from iOS) | **10** nearest, ties by URL |
| B2 (BitChat II) | `main` `8386885` | none for geohash channels | — | the relays its owner configures (`b2 run --nostr wss://…`) |

### bitchat iOS

No commit since the study of 2026-10-27 (`04995f68e4` is still the head of `main`, one commit
after `v1.8.0`: the globe picker, which changes nothing in the relay choice). Every channel,
presence, note and mesh-bridge event goes to the 5 relays nearest the cell's centre
(`TransportConfig.nostrGeoRelayCount = 5`, `GeoRelayDirectory.closestRelays(toGeohash:count:)`).

The reviewed list was last updated on 2026-07-26: more than three months. The census of
2026-10-30 found only half of it working (§3).

### bitchat Android

From `eed88190c0` (2026-10-27) to `v2.2.0` and `main`, the relay choice
(`RelayDirectory.closestRelaysForGeohash`) and the list refresh (once a day, from georelays'
`main`) did not change. Two things did:

- **The globe's activity sweep** (`GeohashActivityMonitor.kt`, `v2.2.0`). While the globe
  picker is open, the app reads kinds 20000 (last hour) and 20001 (last 5 minutes) **without a
  `g` filter**, so the request says nothing about where the user is, on up to 36 relays: the
  default relays, the 2 nearest to each of 22 fixed cells spread over the world (24 at most),
  and the 3 nearest to the precision-3 cell the globe rests on. It only counts activity; it sends
  nothing. Every event is checked (`VerifiedEventFilter`).
- **The copy of the list in the app** was refreshed on `main` (2026-11-01): it only serves the
  first launch, before the daily fetch.

### AirHop

A React Native port of bitchat ([areebahmeddd/airhop](https://github.com/areebahmeddd/airhop),
MIT), wire-compatible on Bluetooth and on the location channels
(`src/core/nostr/geo-relay.ts`):

- It never fetches a list: "a third party who is asking for relays" would learn something,
  and the app must work offline. The list is georelays' raw file of the week of the build, so
  **a phone keeps the list of the release it runs**: weeks, then months, as with iOS.
- It reads the list as iOS does (one entry per canonical address, IPv4 and private names
  refused), takes the **10 nearest** to the cell's centre and breaks ties by URL. Its first 5 are
  exactly what bitchat iOS would pick on the same list (test `tests/airhop.rs`); the next 5 are
  "warm backups", written to and read from even while the first 5 work, so that two users on
  either side of a partial outage still share a relay.
- The user may add up to 5 relays by hand, or turn discovery off and use only those.

Modelled here as `AirhopStrategy` (`src/as_/airhop.rs`).

### B2 (BitChat II)

B2 carries its own packets in ephemeral Nostr events (kind 20420) on the relays its owner gives
it, usually a private relay; it does not choose relays by geohash and does not write to bitchat's
location channels. Its protocol reads bitchat's announce field `bridge_geohash` (the cell of
bitchat's mesh bridge) for compatibility. It refers to these documents for its future relay
choice (`docs/nostr-relays/` of its repository); nothing is implemented yet.

## 2. The lists

| List | Entries | Servers | Working (census 2026-10-30) |
|---|---|---|---|
| bitchat iOS (2026-07-26) | 326 | 326 | **164** |
| bitchat Android (georelays 2026-10-29) | 392 | 346 | 213 |
| AirHop `v1.0.10` (georelays, week of 2026-11-02) | 339 | 339 | 211 |

Servers in common: iOS and Android 200, iOS and AirHop 198, Android and AirHop 323.

"Working": the address answered the census through Tor (as bitchat iOS connects by default) and
the qualification (A6) refused nothing (writes, payment, NIP-42, proof of work). The census is
one reading, from one place, on one day: a relay that refuses Tor exits, or was slow that day,
counts as not working. The relay base was not rebuilt for this study.

## 3. Do their users meet?

Two users meet in a cell when a message of each reaches the other: one writes to a relay the
other reads. p2: the 1,024 cells of precision 2; p5: 500 cells of precision 5 drawn at random.
"Working relays only": the relays the census found not working carry nothing (the clients still
choose them).

| Pair | p2, every relay | p2, working only | p5, every relay | p5, working only |
|---|---|---|---|---|
| bitchat iOS – bitchat Android | 95.0 % | **67.2 %** | 95.4 % | **68.2 %** |
| bitchat iOS – AirHop | 91.6 % | 87.0 % | 93.6 % | 88.8 % |
| bitchat Android – AirHop | 100 % | 93.1 % | 100 % | 93.6 % |

Two users of the **same** client, working relays only:

| Client | p2 | p5 | Working relays per cell (p2) |
|---|---|---|---|
| bitchat iOS | 97.2 % | 97.0 % | **2.1** of 5 |
| bitchat Android | 98.4 % | 98.2 % | 2.4 of 5 |
| AirHop | 100 % | 100 % | 5.9 of 10 |

What it shows:

- **The lists' age now weighs more than the rules.** On paper, iOS and Android meet in 95 % of
  the cells (93 % on 2026-10-27, `evaluation.md`); once the dead relays are taken out, in two
  cells out of three. An iOS user's 5 relays are on average less than half working.
- **More relays cover for dead ones.** AirHop's 10 relays keep it in reach of both bitchat apps
  more often than they reach each other. It is the redundancy A0 proposes (write 8, read 5),
  without A0's replacement of the failed relays.
- **A frozen list ages with the release.** AirHop meets Android everywhere today because their
  lists are a few days apart; a phone that does not update the app drifts as iOS has.

## 4. What it changes in the proposal

The proposal of [evaluation.md](evaluation.md) stands, and is more urgent on two points:

1. **A6 (qualification) before anything else**: a client must not keep writing to relays that
   do not answer. The census's eligibility file is the list the clients need.
2. **A0's redundancy and replacement**: AirHop shows that more relays per cell keep clients
   together in spite of dead relays; A0 adds the replacement, so that a dead relay does not
   take a place.
3. **One list for all clients** (G16, `PLAN_ET1.md`): three clients now read three copies of
   two sources of different ages. A few trusted entities publishing one qualified list, which
   every client reads, removes the drift.

## Sources

- bitchat iOS: [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat), release
  `v1.8.0`, `main` `04995f68e4`: `bitchat/Nostr/GeoRelayDirectory.swift`,
  `bitchat/Services/TransportConfig.swift`, `relays/online_relays_gps.csv`.
- bitchat Android: [permissionlesstech/bitchat-android](https://github.com/permissionlesstech/bitchat-android),
  release `v2.2.0`, `main` `998f168487`: `RelayDirectory.kt`, `NostrRelayManager.kt`,
  `GeohashActivityMonitor.kt`, `VerifiedEventFilter.kt`, `app/src/main/assets/nostr_relays.csv`.
- AirHop: [areebahmeddd/airhop](https://github.com/areebahmeddd/airhop), release `v1.0.10`:
  `src/core/nostr/geo-relay.ts`, `src/core/nostr/geo-relay-source.ts`,
  `.github/workflows/sync-relays.yml`, `assets/data/nostr_relays.csv`.
- B2: [wvmscs/BitChat-II](https://github.com/wvmscs/BitChat-II), `main` `8386885`:
  `crates/b2-transport-nostr`, `crates/b2-proto/src/wire_payload.rs`, `docs/nostr-relays/`.
- Lists: [permissionlesstech/georelays](https://github.com/permissionlesstech/georelays)
  `nostr_relays.csv` at `e7c6e88edf` (2026-10-29); census of geohash-relays-net (`relay-census`,
  2026-10-30).
