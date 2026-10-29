# Geohash channel relays: how the lists are updated on iOS and Android

> **About this copy.** Written on 2026-10-27 while studying the apps with an earlier,
> unpublished prototype: a Rust crate whose examples were `geohash_map` (relay selection and
> read tests, with a map) and `geohash_bot` (a bot on the union of both apps' relays). Copied
> here on 2026-10-29; names such as `examples/…`, "this crate" or "our tool" in the text
> refer to the prototype. The rules it describes are implemented in geohash-relays
> (`src/as_/`, strategy `AS`) and used by [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) and
> [bitchat-console](https://github.com/wvmscs/bitchat-console).

*As of 2026-10-27, based on BitChat's public sources (iOS: `permissionlesstech/bitchat` at commit
`04995f68e4`; Android: `permissionlesstech/bitchat-android` at commit `eed88190c0`; upstream list:
`permissionlesstech/georelays`) and on measurements made with `examples/geohash_map.rs`.*

## Summary

- A geohash channel's messages go only to the **5 relays closest** to the centre of the cell. These
  relays are taken from a list of geolocated relays.
- **iOS and Android do not read the same list.** iOS reads a hand-reviewed copy that is updated
  irregularly. Android reads the raw list, which is regenerated every day.
- **Nor do they apply the same selection rule.** iOS removes duplicates and breaks ties by host
  name. Android keeps entries as they are and breaks ties by file order, so it can count **the same
  server twice**.
- The two apps can therefore **pick different relays for the same channel**. A message posted from
  Android may never reach an iOS user in the same channel, and vice versa.
- Neither app authenticates (NIP-42) or replaces a relay that is down.

## 1. Where the list comes from: the `georelays` repository

The [permissionlesstech/georelays](https://github.com/permissionlesstech/georelays) repository
produces `nostr_relays.csv` (one `host,latitude,longitude` line per relay). A GitHub Action
regenerates it **every day**, in three steps:

| Step | Tool | What it does |
|---|---|---|
| Discovery | `nostr_relay_discovery.py` | Crawls the Nostr network from a seed relay, through contact lists (kind 3) and relay lists (kind 10002). Each candidate is tested with a `REQ`. |
| BitChat filter | `filter_bitchat_relays.sh` (`nak`) | Keeps only relays that accept both reading **and** writing a kind 20000 event. The test **answers NIP-42 challenges**. |
| Location | `relays_geo_lookup.py` | Coordinates **estimated from the relay's IPv4 address**. |

This method has three limits:

- **Approximate positions.** IP geolocation often returns a city centre. Many relays therefore share
  exactly the same coordinates: for example 6 relays at the same point in London (51.5072, -0.1276)
  and several at the centre of Paris. This creates **ties** in the distance ranking.
- **Relays the apps cannot use.** The filter authenticates (NIP-42), but neither app does. A relay
  that requires authentication therefore passes the filter while remaining unusable.
- **Duplicate entries.** The list contains the same server in two forms, `host` and `host:443`,
  with the same coordinates: 49 cases in `nostr_relays.csv` as of 2026-10-26.

## 2. iOS: a reviewed copy, updated through PRs

The iOS app's repository keeps its own copy, `relays/online_relays_gps.csv`.

- The `fetch_georelays.yml` workflow runs **every Sunday at 06:00 UTC**. It fetches the upstream
  `nostr_relays.csv` (at a pinned commit), then has it checked by `scripts/validate_georelays.py`:
  encoding, schema, size, `wss` hosts, coordinates, duplicates, and how far it differs from the
  previous version.
- If anything changed, the workflow **opens a PR that is not merged automatically**. It only takes
  effect once a maintainer reviews and merges it.
- **Last merged update: 2026-07-26.** As of 2026-10-27, the iOS list is therefore about three
  months behind the upstream list.

In the app (`GeoRelayDirectory.swift`, `TransportConfig.swift`):

| Setting | Value |
|---|---|
| Downloaded source | `https://raw.githubusercontent.com/permissionlesstech/bitchat/refs/heads/main/relays/online_relays_gps.csv` |
| Download frequency | 24 h |
| Freshness check | every hour |
| Retry after a failure | from 1 min up to 1 h at most |
| Fallback | local cache (`georelays_cache_v2.csv`), then the CSV bundled with the app |
| Network | through Tor when Tor is on; if Tor is not ready, the download is **skipped** (so as not to expose the IP), and the app keeps its cache |
| Validation on load | strict: **the whole list is rejected** if any line is invalid or if one host appears at two different positions. The app then keeps its previous list. |

## 3. Android: the raw list, directly

In `RelayDirectory.kt`:

| Setting | Value |
|---|---|
| Downloaded source | `https://raw.githubusercontent.com/permissionlesstech/georelays/refs/heads/main/nostr_relays.csv`, **the upstream list, unreviewed** |
| Download frequency | when the data is more than 24 h old |
| Fallback | CSV bundled with the app (assets) |
| Network | not checked |
| Validation on load | lenient: invalid lines are skipped one by one |

## 4. How the 5 relays are chosen

| | iOS (`closestRelays`) | Android (`closestRelaysForGeohash`) |
|---|---|---|
| Entry form | host lowercased, `:443` removed, one entry per host | raw CSV text, `wss://` prepended if there is no scheme, no other normalisation |
| `host` / `host:443` duplicates | merged | **counted as two relays** |
| Sort | by (distance, host name) | by distance (stable sort) |
| Distance tie | broken by host name | **CSV line order** |
| Result | first 5 | first 5 |

## 5. How failing relays are handled

| | iOS | Android |
|---|---|---|
| NIP-42 authentication | no | no: `AUTH` and `CLOSED` are classified as `Unknown` and ignored |
| Subscription closed with `auth-required:` | not detected | not detected: the app believes it is subscribed while nothing arrives |
| Rejected event (`OK false`) | not checked | logged, with no retry |
| Replacing a relay that is down | no | no: falls back to the default relays **only when the selection is empty** |
| Reconnection | not checked | forever: the delay doubles after each failure (1 s, 2 s, 4 s…) up to **5 min**, never giving up, even after a DNS error |
| Permanent vs transient failures | not checked | **no distinction** |

The Android code explicitly justifies endless reconnection (`RelayReconnectPolicy.kt`): a phone
often loses its network (tunnel, airplane mode), and giving up would cut off the channels until the
app is restarted. The trade-off: a relay with an invalid certificate keeps its slot forever and
costs one connection attempt every 5 minutes.

## 6. Side by side

| | iOS | Android |
|---|---|---|
| List used | reviewed copy in `bitchat` | upstream `georelays` |
| Actual pace of changes | at each merged PR (last on 2026-07-26) | daily |
| Human review | yes | no |
| Lag as of 2026-10-27 | about 3 months | about 1 day |
| `host` / `host:443` duplicates | merged | counted twice |
| Tie for a slot | broken by host name | CSV order |
| NIP-42 | no | no |
| Relay that is down | kept | kept, reconnected every 5 min at most |

## 7. Consequences for channels

### 7.1 The two apps don't always meet on the same relays

Each app takes the 5 closest relays **from its own list and with its own rule**. A channel is
really shared between iOS and Android only through the relays common to both selections.

Measurements from 2026-10-27 (`geohash_map`, one line per server, "iOS rank / Android rank"
columns, `-` = not picked, `1+2` = picked twice by Android):

**`#u09tv` (Paris)**: 8 servers in total, **only 2 shared by both apps**.

| Relay | iOS | Android | Status |
|---|---|---|---|
| nostr.wild-vibes.ts.net | 1 | 2 | usable |
| relay-fra.zombi.cloudrodion.com | 2 | 1 | usable (no answer within 8 s the day before) |
| relay.cypherflow.ai | 3 | - | **dead**: certificate from an unknown issuer |
| relay.mmwaves.de | 4 | - | usable (sends AUTH challenges) |
| treuzkas.branruz.com | 5 | - | unreachable: HTTP 302 redirect (certificate issued for another name the day before) |
| relay.noeudlibre.fr (Lille) | - | 3 | usable, **real traffic** (9 recent events; 26 to 30 the day before) |
| nostr-relay.xbytez.io (Lille) | - | 4 | usable |
| r.senx.io (Lille) | - | 5 | usable (no answer within 8 s the day before) |

What this shows:
- iOS spends 2 of its 5 slots on relays that are out of service and already gone from the upstream
  list.
- The relays where the channel is active are picked by Android alone.

**`#u1` (Netherlands)**: 7 servers in total, **only 2 shared** (`cyberguy`, `mccormick`).

| Relay | iOS | Android | Status |
|---|---|---|---|
| nostrelay.circum.space | 1 | - | usable |
| relay.cyberguy.fyi | 2 | 5 | usable (sends AUTH challenges) |
| relay.satsmarkt.club | 3 | - | usable, 72 recent events |
| nostr.stakey.net | - | **1+2** | usable, 64 recent events |
| relay.agorist.space | 4 | - | unreachable: HTTP 502 |
| relay.mccormick.cx | 5 | 3 | usable, 84 recent events |
| test.relays.nuts.cash | - | 4 | usable, 11 recent events |

Android uses `nostr.stakey.net` **twice** there (`nostr.stakey.net` and `nostr.stakey.net:443`): it
actually has only 4 distinct servers for this channel.

> Correction: the 2026-10-26 version of this document listed `borgar.lol` and
> `relay.satsmarkt.club` for Android on `#u1`. Our tool then applied the iOS rule to both lists. The
> real Android selection is the one above.

### 7.2 Effects users see

- **Messages that don't cross platforms**: a message only reaches the other platform through the
  relays both apps picked. If those are down or there are none, the channel is split in two.
- **Incomplete presence**: presence signals (kind 20001) take the same path. An iOS user's "people"
  list may miss some Android users, and vice versa.
- **Changes over time**: the Android list changes every day, the iOS list in jumps. The overlap
  between the two selections therefore shifts without users doing anything.
- **No catching up**: kinds 20000/20001 are ephemeral. Relays generally don't store them, so a
  message that did not reach a relay when it was sent is lost for those reading that relay.

### 7.3 Aggravating factors

- **Distance ties**: with identical coordinates, iOS breaks the tie by host name and Android by CSV
  order. The selections can therefore differ even with the same list.
- **Relays requiring NIP-42**: present in the list, but unusable, since neither app authenticates.
  Android does not even notice the refusal (`CLOSED` is ignored).
- **No replacement**: neither app replaces a relay that is down with the next one. Each relay that
  is down directly reduces the number of relays the channel actually has. Android keeps
  reconnecting to it forever.
- **Servers counted twice (Android)**: with no normalisation, `host` and `host:443` take two slots
  for the same server. Out of the 1,024 precision-2 cells, **322** therefore have only 4 distinct
  servers on Android (simulation with `nostr_relays.csv` from 2026-10-26). Examples: `#u1`
  (`nostr.stakey.net`), `#gb` (`relay.staging.plebeian.market`), `#9q` (`nostr.2b9t.xyz`).

## 8. What the prototype did

- `examples/common/geo.rs` reproduces **each app's exact rule** (§4): `ios_selection` and
  `android_selection`. `select_both` merges them, one line per server.
- `examples/geohash_map.rs` shows, for a geohash, the iOS selection **and** the Android selection,
  with each server's rank in each and the result of a channel read test. The `overlay` mode
  produces a map. Relays that are down for good (certificate, DNS, connection refused, HTTP 4xx)
  are set aside by default; `--no-drop-dead` keeps them. The tool warns when iOS would reject the
  list (one host at two positions).
- `examples/geohash_bot.rs` reads both lists (`--relays-csv`, `--android-csv`) and connects to
  **the union** of both selections, once per server, to reach users of both platforms. It also
  sets aside relays that are down for good by default.

Today, geohash-relays (`src/as_/`) holds each app's rule, and
[bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) shows both selections on a map:
`bitchat-geohash u09tv`, then the relay layer of the page.

## 9. Suggestions (for BitChat)

- Have both apps read **the same list**: either Android switches to the reviewed copy, or iOS
  accepts the automatically validated upstream list.
- Merge the weekly PRs more often, or merge them automatically when the validator reports only a
  small change.
- Align Android on the iOS rule: normalise entries (lowercase, no `:443`), remove duplicates, and
  break ties by host name.
- Remove relays that require NIP-42 from the list, since the apps do not authenticate. The
  upstream filter could test without authentication.
- On Android, handle `CLOSED` (at least `auth-required:`) so as not to stay subscribed to a relay
  that sends nothing.

## Sources

- iOS (commit `04995f68e4`, 2026-10-25): `bitchat/Nostr/GeoRelayDirectory.swift` (`closestRelays`,
  `validatedEntries`, `validatedDirectoryAddress`), `bitchat/Services/TransportConfig.swift`,
  `.github/workflows/fetch_georelays.yml`, `.github/workflows/validate_georelays.yml`. History of
  `relays/online_relays_gps.csv`: latest commits on 2026-05-31, 06-07, 06-14, 06-21 and 07-26.
- Android (commit `eed88190c0`, 2026-10-26): `nostr/RelayDirectory.kt` (`closestRelaysForGeohash`,
  `normalizeRelayUrl`, `parseCsv`), `nostr/NostrRelayManager.kt`, `nostr/NostrRequest.kt`
  (`NostrResponse.fromJsonArray`), `nostr/RelayReconnectPolicy.kt`, under
  `app/src/main/java/com/bitchat/android/`.
- Upstream list: README and scripts of the `permissionlesstech/georelays` repository (last updated
  on 2026-10-25).
- Measurements: `geohash_map`, on 2026-10-27. Relay status changes from one day to the next.
