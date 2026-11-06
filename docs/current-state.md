# Current state of the location channels: clients, lists, encounters (2026-11-06)

Who writes to the geohash channels today, how each client chooses its relays, and whether their
users meet. It completes and summarises section 1 of
[geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md).
It updates [geohash-relays-updates.md](geohash-relays-updates.md) and
[relay-selection-corrections.md](relay-selection-corrections.md) (2026-10-27) to the latest
releases, and adds the clients not covered by the first study. How the numbers are made: see
[Sources](#sources).

## Summary

- **bitchat iOS** and **bitchat Android** choose their relays as on 2026-10-27: nothing changed
  in their relay choice, in the releases (iOS `v1.8.0`, Android `v2.2.0`) nor on their `main`.
- **iOS still reads a list of 2026-07-26.** Of its 326 relays, **164** were found working by the
  census of 2026-10-30: a cell gets **2.1 working relays** out of 5 on average.
- **AirHop**, a third client wire-compatible with bitchat, chooses **10** relays (the 5 bitchat
  picks, then 5 backups), from georelays' raw list frozen in each release.
- **Sonar**, a bitchat-compatible client with a Rust core, chooses **12** relays with
  Android's rule (Android and desktop apps) from georelays' list **of 2026-02-15**, copied by
  hand into its core on 2026-06-15, and sends a channel's events to every relay of its pool,
  not only to the cell's. Its iOS app still runs bitchat iOS's older code.
- Users of iOS and Android meet in **95 %** of the cells when every relay works, but in only
  **67 %** on the relays that actually answered. AirHop meets Android everywhere (same source,
  twice as many relays) and iOS in 87 to 92 % of the cells. Sonar meets Android in only
  **42 %** of the cells on working relays: its list is nearly nine months old.

## 1. The clients

| Client | Version studied | List | Read | Choice |
|---|---|---|---|---|
| bitchat iOS | release `v1.8.0` (2026-10-23) = `main` `04995f68e4` | bitchat's reviewed `relays/online_relays_gps.csv`, fetched from `main` at runtime; last changed 2026-07-26 (`fb8fe39713`) | one entry per normalised host | 5 nearest the cell's centre, ties by host |
| bitchat Android | release `v2.2.0` (2026-10-28); `main` `998f168487` (2026-11-01) | georelays' raw `nostr_relays.csv`, fetched at runtime once a day; a copy in the app for the first launch | every line as is (`host` and `host:443` are two) | 5 nearest, ties in list order (stable sort) |
| AirHop | release `v1.0.10` (2026-11-02) = `main` | georelays' raw `nostr_relays.csv`, **frozen in the app** at build time, never fetched; a weekly job proposes the new one as a pull request | one entry per canonical address (rule ported from iOS) | **10** nearest, ties by URL |
| Sonar (core: Android, desktop) | pre-release `v0.1-alpha.15.5` (2026-10-29); `main` `fdf091c` (2026-11-01): same relay code | georelays' raw list of **2026-02-15**, copied into the core on 2026-06-15; the weekly refresh writes the source file, which the core does not read | every line as is (`host` and `host:443` are two) | **12** nearest, ties in list order; events sent to the whole pool |
| Sonar iOS | the same | georelays' raw `nostr_relays.csv`, fetched at runtime once a day; the repository's weekly file in the app for the first launch | one entry per line, scheme removed, no lowercasing | 5 nearest (bitchat iOS's older code), ties in an order that changes at each launch |

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
  exactly what bitchat iOS would pick on the same list (our tests); the next 5 are
  "warm backups", written to and read from even while the first 5 work, so that two users on
  either side of a partial outage still share a relay.
- The user may add up to 5 relays by hand, or turn discovery off and use only those.

Modelled in geohash-relays as `AirhopStrategy`.

### Sonar

A bitchat-compatible client ([hedwig-corp/bitchat-to-sonar](https://github.com/hedwig-corp/bitchat-to-sonar),
Unlicense) whose apps (Android, iOS, desktop) share a Rust core. What its code does, release
`v0.1-alpha.15.5` and `main` `fdf091c` alike (the relay code last changed on 2026-06-16 and
2026-10-03); its comments say it mirrors bitchat iOS, which it does not:

- **The list is frozen in the core.** `core/sonar-core/src/relays_gps.csv`, read with
  `include_str!`, has 313 relays, 133 found working by the census.
  - **Where it comes from.** A weekly job (`.github/workflows/fetch_georelays.yml`, Sundays at
    06:00 UTC) downloads georelays' raw `nostr_relays.csv` from its `main` into
    `relays/online_relays_gps.csv`, a file of the bitchat fork Sonar started from, and commits
    it when it changed. On 2026-06-15 (`85629a8`), a developer copied that file into the core;
    the job had not run since 2026-02-15, so the core holds georelays' list of **2026-02-15**.
  - **How it is renewed.** By hand only: the core's comment calls fetching and caching the file
    "a future improvement", and nothing copies it again. The job has updated the source file
    every week since 2026-06-21; the core has not followed. Nothing is fetched at runtime. This
    study will take the core's next copy as soon as it lands.
- **Android's reading and rule, not iOS's.** Each line is kept as it is, `wss://` put in front
  (no lowercasing, `host` and `host:443` stay two entries, no duplicate removed, no host
  refused); the relays are sorted by distance to the cell's centre with a stable sort, so ties
  keep the list's order (`closest_relays_for_geohash`, `relay_directory.rs`). Its first 5 are
  exactly what bitchat Android picks on the same list (our tests).
- **12 relays, not 5** (`ensure_geohash_relays`, `client.rs`): "wider than bitchat's 5 so the two
  overlap on at least one relay even when the directories don't perfectly agree".
- **One pool for everything.** The 12 relays are added to the client's single `nostr-sdk` pool,
  and a channel's subscription and every event it sends (`subscribe_geohash`, `send_geohash`,
  `send_event`) go to **every relay of the pool**: the relays the app starts with (five public
  relays when it has nothing else, `DEFAULT_RELAY_URLS` in `RelayDiagnostics.kt`) and the 12 of
  every cell opened before. A relay far from the cell then receives its events, and each relay
  of the pool sees all the cells the user follows. The core has no Tor support (no Tor or SOCKS
  dependency): it connects directly.
- **The pool stays on the phone.** Nothing removes a relay from it during the session, and
  nothing tells it to anyone: no Nostr event carries it (no relay list, kind 10002, in the core)
  and no Bluetooth message either. Another user benefits only when, by chance, they read one
  of these relays for the same cell.
- **Sonar's iOS app is another client.** Its channels go through the bitchat iOS code it
  started from (`ios/bitchat/Nostr/GeoRelayDirectory.swift`, older than bitchat's current
  one): the 5 nearest relays, from georelays' raw list fetched once a day (the repository's
  weekly file serves the first launch). Its entries are kept in a set, so equal distances fall
  in an order that changes at each launch. It is not modelled below.

Modelled in geohash-relays as `SonarStrategy`: the cell's 12, `with_pool()` for the five
default relays, and a simulation of the cells opened before: `with_history()` for given cells,
`with_levels()` for the coarser channels of the same place (region, province, city…, as the
channel picker offers them).


## 2. The lists

| List | Entries | Servers | Working (census 2026-10-30) |
|---|---|---|---|
| bitchat iOS (2026-07-26) | 326 | 326 | **164** |
| bitchat Android (georelays 2026-10-29) | 392 | 346 | 213 |
| AirHop `v1.0.10` (georelays, week of 2026-11-02) | 339 | 339 | 211 |
| Sonar core (georelays 2026-02-15, copied 2026-06-15) | 313 | 313 | **133** |

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
| bitchat iOS – Sonar | 92.0 % | 65.2 % (65.8 % with the pool) | 91.2 % | 65.6 % (65.8 %) |
| bitchat Android – Sonar | 81.5 % | **42.1 %** (42.1 %) | 78.4 % | **40.2 %** (40.2 %) |
| AirHop – Sonar | 94.8 % | 93.2 % (93.2 %) | 92.4 % | 91.0 % (91.0 %) |

Two users of the **same** client, working relays only:

| Client | p2 | p5 | Working relays per cell (p2) |
|---|---|---|---|
| bitchat iOS | 97.2 % | 97.0 % | **2.1** of 5 |
| bitchat Android | 98.4 % | 98.2 % | 2.4 of 5 |
| AirHop | 100 % | 100 % | 5.9 of 10 |
| Sonar | 100 % | 100 % | 4.0 of 12 (6.0 of 17 with the pool) |

What it shows:

- **The lists' age now weighs more than the rules.** On paper, iOS and Android meet in 95 % of
  the cells (93 % on 2026-10-27, [evaluation.md](evaluation.md)); once the dead relays are taken out, in two
  cells out of three. An iOS user's 5 relays are on average less than half working.
- **More relays cover for dead ones.** AirHop's 10 relays keep it in reach of both bitchat apps
  more often than they reach each other. It is the redundancy A0 proposes (write 8, read 5),
  without A0's replacement of the failed relays.
- **A frozen list ages with the release.** AirHop meets Android everywhere today because their
  lists are a few days apart; a phone that does not update the app drifts as iOS has. Sonar
  shows the end of it: with the same rule as Android and more relays, a list nearly nine months
  old leaves it in reach of Android in only 42 % of the cells. With its own refreshed file (the one
  its weekly job already writes), the same code would meet Android in 96 % of them.
- **Writing wide is not the same as choosing well.** Sonar's pool adds relays to every cell, but
  not the ones the other clients read there: its default relays change nothing measurable.
  Simulated with the coarser channels of the same place opened first, the pool gains at most
  0.6 point (p5, working relays: Android 40.4 %, iOS 66.0 %, AirHop 91.6 %), while every relay
  of the pool learns the user's cells. The idea itself, a pool that every cell writes to, is
  discussed as an approach in
  [geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md#sonars-pool-studied-to-be-explored).

## 4. What it changes in the proposal

The proposal of [evaluation.md](evaluation.md) stands, and is more urgent on two points:

1. **A6 (qualification) before anything else**: a client must not keep writing to relays that
   do not answer. The census's eligibility file is the list the clients need.
2. **A0's redundancy and replacement**: AirHop shows that more relays per cell keep clients
   together in spite of dead relays; A0 adds the replacement, so that a dead relay does not
   take a place.
3. **One list for all clients**: four clients now read four copies of
   two sources of different ages, the oldest compiled into an app. A few trusted entities publishing one qualified list, which
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
- Sonar: [hedwig-corp/bitchat-to-sonar](https://github.com/hedwig-corp/bitchat-to-sonar),
  pre-release `v0.1-alpha.15.5`, `main` `fdf091c`: `core/sonar-core/src/relay_directory.rs`,
  `core/sonar-core/src/relays_gps.csv` (copied in `85629a8`, identical to
  `relays/online_relays_gps.csv` of `c1f5868`, 2026-02-15), `core/sonar-core/src/client.rs`,
  `apps/sonar/composeApp/src/commonMain/kotlin/chat/bitchat/sonar/RelayDiagnostics.kt`,
  `.github/workflows/fetch_georelays.yml` and the history of `relays/online_relays_gps.csv`,
  `ios/bitchat/Nostr/GeoRelayDirectory.swift`, `ios/bitchat/Services/TransportConfig.swift`,
  `ios/bitchat/Protocols/LocationChannel.swift`.
- Lists: [permissionlesstech/georelays](https://github.com/permissionlesstech/georelays)
  `nostr_relays.csv` at `e7c6e88edf` (2026-10-29); census of [geohash-relays-net](https://github.com/wvmscs/geohash-relays-net#census-of-relays-relay-census) (`relay-census`,
  2026-10-30).
- This study: the bench [`examples/clients.rs`](https://github.com/wvmscs/geohash-relays/blob/strategies/examples/clients.rs) of geohash-relays
  (`cargo run --release --example clients`), the models
  [`src/as_/airhop.rs`](https://github.com/wvmscs/geohash-relays/blob/strategies/src/as_/airhop.rs) and [`src/as_/sonar.rs`](https://github.com/wvmscs/geohash-relays/blob/strategies/src/as_/sonar.rs),
  and the relay lists frozen in [`tests/data`](https://github.com/wvmscs/geohash-relays/tree/strategies/tests/data).
