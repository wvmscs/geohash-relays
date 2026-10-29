# Geohash relay selection: corrections to port

> **About this copy.** Written on 2026-10-27 while studying the apps with an earlier,
> unpublished prototype: a Rust crate whose examples were `geohash_map` (relay selection and
> read tests, with a map) and `geohash_bot` (a bot on the union of both apps' relays). Copied
> here on 2026-10-29; names such as `examples/…`, "this crate" or "our tool" in the text
> refer to the prototype. The rules it describes are implemented in geohash-relays
> (`src/as_/`, strategy `AS`) and used by [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) and
> [bitchat-console](https://github.com/wvmscs/bitchat-console).

**Date:** 2026-10-27
**Scope:** `examples/common/geo.rs`, `examples/geohash_map.rs`, `examples/common/map_overlay.html`,
`examples/geohash_bot.rs`, `Cargo.toml`
**Applies to:** any implementation derived from the prototype's `geohash_map` / `geohash_bot`
examples. In geohash-relays: `src/as_/`, and the tests of C6 in `tests/select.rs`.

## 1. The defect

The previous code used **one selection function for both apps**. It applied iOS-style
normalisation to every list, then added any relay tied with the 5th one, "to reach both apps":

```text
closest_relays(csv, geohash, count):        # BEFORE: used for the iOS list AND the Android list
    for each row after the first line:
        host = strip "wss://", strip trailing "/", strip ":443", lowercase
        entry = ("wss://" + host, lat, lon, km = haversine(centre, lat, lon))
    sort entries by (km, url)
    remove later entries with an already-seen url
    last = km of entry #count
    return every entry with index < count OR km == last     # ties beyond 5 included
```

This is wrong for Android, which does **no** normalisation, **no** deduplication and **no**
tie-breaking. As a result, the "Android" selection shown by the tools and used by the bot was not
the one the Android app actually uses:

| `#u1` (lists below, §6) | Android selection |
|---|---|
| Shown before | `nostr.stakey.net`, `relay.mccormick.cx`, `borgar.lol`, `relay.cyberguy.fyi`, `relay.satsmarkt.club`, + ties `test.relays.nuts.cash`, `vidar.nestoris.online` |
| Real (after) | `nostr.stakey.net`, `nostr.stakey.net:443`, `relay.mccormick.cx`, `test.relays.nuts.cash`, `relay.cyberguy.fyi` |

The iOS selection was also slightly off: iOS returns exactly 5 entries, never the ties after them.

## 2. Reference behaviour (what to reproduce)

Read from the app sources: iOS `bitchat/Nostr/GeoRelayDirectory.swift` at commit `04995f68e4`,
Android `app/src/main/java/com/bitchat/android/nostr/RelayDirectory.kt` at commit `eed88190c0`.

### 2.1 iOS: `closestRelays(toLat:lon:count:)`

1. **Entry key** (`validatedDirectoryAddress`): prepend `wss://` if the text has no `://`, parse it
   as a URL, take the host **lowercased**. Keep the port only if it is not 443 (`host:port`).
2. **One entry per host** (`validatedEntries`). If the same host appears with **different
   coordinates**, iOS **rejects the whole list** and keeps its previous one (cache or bundled
   copy). Any malformed line also rejects the whole list.
3. **Sort** by `(distance_km, host)`, ascending.
4. **Take the first `count` (5)** entries. URL is `wss://<host>`.

### 2.2 Android: `closestRelaysForGeohash(geohash, nRelays)`

1. **Parsing** (`parseCsv`): skip blank lines and any line whose lowercase form starts with
   `relay url`; split on `,`; skip lines with fewer than 3 fields or unparseable coordinates. Bad
   lines are skipped one by one; the list is never rejected as a whole.
2. **Entry URL** (`normalizeRelayUrl`): trimmed text; if it has no `://`, prepend `wss://`.
   **Nothing else**: no lowercasing, `:443` kept, no deduplication. `host` and `host:443` are two
   entries.
3. **Sort** by distance only, with a **stable** sort (`sortedBy`): ties keep CSV order.
4. **Take the first `nRelays` (5)**.

## 3. Corrections made

### C1. One selection function per app (`examples/common/geo.rs`)

`closest_relays` is removed and replaced by two functions:

```text
normalize_host(raw):                        # the iOS key; also used to tell two entries are one server
    s = trim(raw)
    if s contains "://": s = part after "://"
    s = strip trailing "/" characters
    s = strip a final ":443"
    return lowercase(s)

rows(csv):                                  # shared, lenient parsing (Android rules, §2.2 step 1)
    for each trimmed line, skipping blanks and lines starting (case-insensitively) with "relay url":
        fields = split on ",", each trimmed
        yield (fields[0], parse fields[1], parse fields[2])   # skip the line if any of these fails

ios_selection(csv, geohash, count):
    by_host = map; for (addr, lat, lon) in rows(csv): by_host[normalize_host(addr)] ??= (lat, lon)
    entries = [("wss://" + host, lat, lon, km) for each host]
    sort entries by (km, url)
    return first count

android_selection(csv, geohash, count):
    entries = [(addr if addr contains "://" else "wss://" + addr, lat, lon, km) for each row]
    STABLE sort entries by km only
    return first count
```

Port notes:

- The sort in `android_selection` **must be stable**. Rust's `sort_by` and Kotlin's `sortedBy` are;
  `sort_unstable_by`, C's `qsort` and some others are not.
- `ios_selection` keeps the **first** coordinates seen for a host. iOS would reject the list
  instead (see C3).
- The tie-inclusion step (`km == last`) is removed for both apps.

### C2. The union is keyed by server (`select_both`)

```text
select_both(ios_csv?, android_csv?, geohash):
    out = []                                         # one entry per server
    add(relay, rank, app):
        url = "wss://" + normalize_host(relay.url)
        find or append the entry for url (first relay added gives lat/lon/km)
        if app == iOS:  entry.ios = rank
        else:           entry.android.append(rank)   # Android can rank one server twice
    for i, r in ios_selection(ios_csv, geohash, 5):         add(r, i + 1, iOS)
    for i, r in android_selection(android_csv, geohash, 5): add(r, i + 1, Android)
    sort out by (km, url)
```

Type change in `Selected`:

| Field | Before | After |
|---|---|---|
| `ios` | `Option<usize>`, could be > 5 (tie) | `Option<usize>`, 1–5 |
| `android` | `Option<usize>`, could be > 5 (tie) | `Vec<usize>`, 0, 1 or 2 ranks |
| `relay.url` | normalised `wss://host` | normalised `wss://host` (unchanged); one entry per server |

Consequences:
- The bot opens **one connection per server**, even when Android counts it twice.
- Each server is probed once.
- When a server is in both lists, its displayed coordinates and distance come from the iOS list
  (added first). The two lists can give the same host different coordinates (seen on `#9q`:
  `relay.earthly.city`).

### C3. Warning when iOS would reject its list (`ios_conflict`)

New function: returns the first normalised host listed at two different coordinates, or none.
`geohash_map` prints:

```text
warning: <host> is listed at two places in <file>: iOS would reject this list and keep its previous copy
```

Only this rejection cause is checked. iOS's other checks (header, size, host syntax, minimum
entry count, overlap with the previous list) are **not** reproduced.

### C4. Display (`examples/geohash_map.rs`, `examples/common/map_overlay.html`)

- Terminal: the `*` tie marker is removed. The Android column shows every rank joined with `+`
  (`1+2` = same server picked twice). The header reads
  `rank in each app's choice; Android "2+3" = the same server picked twice`.
- Overlay JSON: the `slots` field is removed; `android` is now an **array** of ranks; `ios` stays a
  number or `null`.
- Overlay page: the dashed "tie" pin style is removed. Details read `Android #2 + #3 (picked twice)`.
  A pin is square when only one app uses the server (`ios == null` or `android` empty).

### C5. Bot (`examples/geohash_bot.rs`)

Only the platform label changed: `iOS+Android` / `iOS` / `Android` is now based on
`android.is_empty()`. The relay list it connects to comes from `select_both` (C2).

### C6. Tests (`examples/common/geo.rs`, `Cargo.toml`)

Four unit tests were added. `Cargo.toml` gets the following so that `cargo test` runs them (Cargo
builds examples but doesn't run their tests by default):

```toml
[[example]]
name = "geohash_map"
test = true
```

Fixture, and expected results to port as-is (geohash `u1`; the first three rows share one point,
so only names and order decide):

```text
Relay URL,Latitude,Longitude
zeta.example,52.0,5.0
alpha.example:443,52.0,5.0
alpha.example,52.0,5.0
Far.Example,10.0,10.0
```

| Test | Call | Expected |
|---|---|---|
| iOS dedup + host tie-break | `ios_selection(csv, "u1", 2)` | `wss://alpha.example`, `wss://zeta.example` |
| iOS lowercases | last of `ios_selection(csv, "u1", 5)` | `wss://far.example` (3 entries in total) |
| Android raw + CSV order | `android_selection(csv, "u1", 3)` | `wss://zeta.example`, `wss://alpha.example:443`, `wss://alpha.example` |
| Union by server | `select_both(csv, csv, "u1")` | 3 servers; `alpha.example`: `ios = 1`, `android = [2, 3]` |
| iOS conflict | `ios_conflict(csv)` / `ios_conflict(csv + "alpha.example,1.0,1.0")` | none / `alpha.example` |

## 4. Verification

The new Android selection was checked against an **independent** re-implementation (a Python
script written from the Kotlin source, not from the Rust code), on the real lists from §6. Both
give the same results:

| Cell | iOS (5) | Android (5, in order) |
|---|---|---|
| `u1` | circum.space, cyberguy.fyi, satsmarkt.club, agorist.space, mccormick.cx | **stakey.net, stakey.net:443**, mccormick.cx, nuts.cash, cyberguy.fyi |
| `gb` | iskarion.ddns.net, 88mph.life, chaima.info, bornheimer.app, qstr.app | 88mph.life, **plebeian.market:443, plebeian.market**, nearhood.co.uk, underorion.se |
| `9q` | earthly.city, herbstmeister.com, zimage.com, 2b9t.xyz, syb.lol | zimage.com, earthly.city, **2b9t.xyz, 2b9t.xyz:443**, layer3.news |

(Host names shortened.)

Over the 1,024 precision-2 cells, the Android rule yields only 4 distinct servers in **322** cells
with this Android list. A port can use this count as a regression check.

`cargo test --all-features` and `cargo clippy --all-targets --all-features -- -D warnings` pass.

## 5. Porting checklist

- [ ] Replace the single selection function with `ios_selection` and `android_selection` (C1).
- [ ] Use a **stable** sort for Android, keyed on distance only.
- [ ] Remove the tie-inclusion step for both apps.
- [ ] Key the union by `normalize_host`, store Android ranks as a list (C2).
- [ ] Connect to and probe each server once.
- [ ] Add the iOS conflict warning (C3), optional.
- [ ] Update displays and overlay JSON: `android` array, no `slots`, no tie styling (C4).
- [ ] Port the four tests and the fixture (C6), and make sure your build runs them.
- [ ] Re-run the §4 cells against the §6 lists and compare.

Prerequisites from earlier changes, in case the derived implementation predates them: both lists
loaded (`--relays-csv` for iOS, `--android-csv` for Android), the shared probe in
`examples/common/probe.rs`, and permanent failures dropped by default (`--no-drop-dead`).

## 6. Data used

| List | Source | Lines | SHA-256 |
|---|---|---|---|
| iOS | `https://raw.githubusercontent.com/permissionlesstech/bitchat/main/relays/online_relays_gps.csv` | 442 | `811523100064820b024714eaf3bbe0a9ba99a3c257e453e01bc1bc0f3bf8401a` |
| Android | `https://raw.githubusercontent.com/permissionlesstech/georelays/main/nostr_relays.csv` (downloaded 2026-10-26) | 382 | `f503bae5ba44ae77851d3d1073258a91195025e472eef94b5cdc4c47fcde9a01` |

The Android list changes daily: with a newer copy, the §4 results and the 322 count will differ.
Use these hashes to tell whether you are comparing against the same data.

## 7. Known limits (unchanged by this correction)

- Distances use the haversine formula in km with `asin`; Android uses metres with `atan2`. The
  results only differ in the last floating-point digits, which can matter only for two relays at
  almost (not exactly) the same distance. Exact ties, the common case, are unaffected.
- iOS's full list validation is not reproduced (see C3).
- Neither app's NIP-42, `CLOSED` handling or reconnection policy is modelled by the selection
  code. Those are documented in [geohash-relays-updates.md](geohash-relays-updates.md).
