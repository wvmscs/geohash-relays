# BitChat geohash channels on Nostr: app behaviour, relay requirements and alternatives

> **About this copy.** Written on 2026-10-27 while studying the apps with an earlier,
> unpublished prototype: a Rust crate whose examples were `geohash_map` (relay selection and
> read tests, with a map) and `geohash_bot` (a bot on the union of both apps' relays). Copied
> here on 2026-10-29; names such as `examples/…`, "this crate" or "our tool" in the text
> refer to the prototype. The rules it describes are implemented in geohash-relays
> (`src/as_/`, strategy `AS`) and used by [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) and
> [bitchat-console](https://github.com/wvmscs/bitchat-console).

**Status:** draft specification, version 0.2. Use it as a starting point. It is not complete.
**Date:** 2026-10-27
**Sources:** the official repositories, cloned on 2026-10-27.
- Swift app (iOS and macOS use the same code): `permissionlesstech/bitchat`, commit `04995f6`.
- Android app: `permissionlesstech/bitchat-android`, commit `eed8819`.

File paths in this document are relative to these repositories. Related documents:
- [geohash-relays-updates.md](geohash-relays-updates.md): how the relay lists are updated.
- [relay-selection-corrections.md](relay-selection-corrections.md): the exact selection rule of each app.

## Writing rules

This document follows the spirit of ASD-STE100 (Simplified Technical English):
- Each sentence is short and has one topic.
- The text uses the active voice.
- Each term has one meaning. Annex A and Annex B define the terms.

There is one exception. The requirements use the keywords **MUST**, **MUST NOT**, **SHOULD**,
**SHOULD NOT** and **MAY** as RFC 2119 and RFC 8174 define them.

This document keeps three types of statement apart:
- **Facts** come from the source code.
- **Measurements** come from tests on 2026-10-27.
- **Proposals** are in sections 5 and 6.

## Contents

1. [Reverse engineering: what the apps do with relays](#1-reverse-engineering-what-the-apps-do-with-relays)
2. [What the design assumes about relays](#2-what-the-design-assumes-about-relays)
3. [Requirements for a geohash channel relay (v0.2)](#3-requirements-for-a-geohash-channel-relay-v02)
4. [Is the selection of 5 relays necessary?](#4-is-the-selection-of-5-relays-necessary)
5. [Alternative approaches](#5-alternative-approaches)
6. [Recommendation](#6-recommendation)
7. [Open questions and measurements to do](#7-open-questions-and-measurements-to-do)
- [Annex A: Definitions of the requested terms](#annex-a-definitions-of-the-requested-terms)
- [Annex B: Other terms](#annex-b-other-terms)
- [Annex C: Source files](#annex-c-source-files)

---

## 1. Reverse engineering: what the apps do with relays

### 1.1 Two sets of relays

Each app uses two different sets of relays.

| Set | iOS/macOS | Android | Use |
|---|---|---|---|
| **Default relays** (fixed in the code) | `relay.damus.io`, `nos.lol`, `relay.primal.net`, `offchain.pub`, and relays that the user adds | `relay.damus.io`, `relay.primal.net`, `offchain.pub`, `nostr21.com` | Private messages (A.1), courier drops (A.2) |
| **Geohash relays** (5 for each cell, calculated) | Reviewed list `relays/online_relays_gps.csv` | Unreviewed list `georelays/nostr_relays.csv` | All public traffic that is related to a location |

Sources: `bitchat/Nostr/NostrRelayManager.swift` (`builtInRelays`) and
`app/src/main/java/com/bitchat/android/nostr/NostrRelayManager.kt` (`DEFAULT_RELAYS`).

The two apps have only three default relays in common: `damus`, `primal` and `offchain`.

### 1.2 List of Nostr data flows

| Flow | Kind | Tags | Content | Relays | Subscription filter | iOS | Android |
|---|---|---|---|---|---|---|---|
| Channel message | 20000 (ephemeral) | `g`=geohash, `n`=nickname (optional), `t`=`teleport` (optional), `nonce` (NIP-13, optional) | text | 5 geohash relays | `kinds:[20000,20001]`, `#g:[gh]`, `since: −1 h`, `limit: 200` | ✓ | ✓ (separate subscription for 20000 only) |
| Presence | 20001 (ephemeral) | `g` only | empty | 5 geohash relays | iOS: same as the channel message. Android: `#g`, `since: −1 h` or `−24 h`, `limit: 200` | ✓ | ✓ |
| Activity sampling | read only | — | — | 5 geohash relays **for each sampled geohash** | `kinds:[20000,20001]`, `#g`, `since: −5 min`, `limit: 100` | ✓ | — |
| Location note | 1 (stored) | `g` (precision 8), `n` | text | 5 geohash relays of the centre cell | `kinds:[1]`, `#g:[cell + 8 neighbours]`, no `since`, `limit: 200` | ✓ | ✓ (one subscription for each cell) |
| Board note (B.6) | 1 (stored) | `g`, `n`, `expiration` (NIP-40), `t`=`urgent` (optional) | text | 5 geohash relays | same as location notes | ✓ | — |
| Board note removal | 5 (NIP-09) | `e` | empty | 5 geohash relays | — | ✓ | — |
| Mesh bridge (A.3) | 20000 / 20001 | `r`=cell (precision 6), `n`, `m`=[stable ID, mesh ID, timestamp in ms] | text / empty | 5 geohash relays **of the cell** | `kinds:[20000,20001]`, `#r:[cell + 8 neighbours]`, `since: −15 min`, `limit: 200` | ✓ (option) | — |
| Private message (A.1) | 1059 → 13 → 14 | `p` = recipient | encrypted, BitChat format | **default relays** | `kinds:[1059]`, `#p:[channel key]`, `since: −24 h`, `limit: 100` | ✓ | ✓ |
| Courier drop (A.2) | 1401 (stored) | `x` = rotating tag, `expiration` | sealed envelope | **default relays** | `kinds:[1401]`, `#x:[…]` | ✓ | — |

Main sources:
- iOS: `bitchat/Nostr/NostrProtocol.swift`, `bitchat/ViewModels/GeohashSubscriptionManager.swift`,
  `bitchat/Services/LocationNotesManager.swift`, `bitchat/Services/Board/BoardManager.swift`,
  `bitchat/Services/Gateway/BridgeService.swift`, `bitchat/ViewModels/ChatViewModelBootstrapper.swift`.
- Android: `nostr/NostrProtocol.kt`, `nostr/NostrFilter.kt`, `nostr/NostrSubscriptionManager.kt`,
  `nostr/NostrBackgroundRuntime.kt`, `nostr/LocationNotesManager.kt`.

Important points:

- **The geohash relays carry all public traffic that is related to a location.** This includes:
  - messages and presence;
  - location notes and board notes;
  - the mesh bridge.

  Private messages do not use the geohash relays.
- **The `g` tag contains a cell of precision 2 to 8**, in lowercase.
  - The apps use precisions 2 (region), 4 (province), 5 (city), 6 (neighbourhood), 7 (block) and
    8 (building).
  - The apps do not use precision 3.
- **One device opens many connections.**
  - In the background, iOS samples each available level, down to the building level.
  - iOS also samples the bookmarked geohashes.
  - Each geohash has its own 5 relays, and the default relays come in addition.
  - Thus one iOS device can connect to more than 20 relays at the same time.
- **Android does not have the newer iOS functions.** It has:
  - no mesh bridge;
  - no courier drops;
  - no board notes;
  - no NIP-40 expiration;
  - no NIP-09 removal.

### 1.3 Identity: one key for each cell

Each cell has its own key. The app derives the key from a seed that it keeps on the device:
`HMAC-SHA256(seed, geohash ‖ counter)`. Sources:
- iOS: `bitchat/Nostr/NostrIdentityBridge.swift`, `deriveIdentity(forGeohash:)`.
- Android: `NostrIdentityBridge.deriveIdentity`.

Results for relays:
- The public keys are **new and have no history**. They have:
  - no profile;
  - no contact list;
  - no reputation;
  - no payment.
- A relay that filters by allow list, web of trust or paid subscription **rejects BitChat**.
- A rate limit for each public key **does not stop abuse**, because an attacker can make new keys
  at no cost.
- A rate limit that is too low **causes problems for honest users**.
- A relay cannot link two cells through the key.
- The relay can see the IP address of the user. Tor hides this address (section 1.5).

### 1.4 Relay selection for a cell

The related documents give the details. In summary:
- The app selects the 5 relays that are nearest to the centre of the cell.
- iOS and Android use different lists. They also break distance ties differently.
- The apps **do not replace** a relay that fails.
- Android uses the default relays only when the selection is **empty**.

The app uses the same calculation to **write** and to **read**. Thus the rendezvous (B.7)
operates only if all peers in a cell calculate the same set.

The Android code has an unused function. `sendEventToGeohash` and `subscribeForGeohash` have an
`includeDefaults` parameter, which adds the default relays to the 5 relays of the cell. This
parameter is `false` in all calls.

### 1.5 Transport and relay dialogue

| Item | iOS/macOS | Android |
|---|---|---|
| Transport | WebSocket (`URLSession`) | WebSocket (OkHttp) |
| Tor | **on by default** (`userTorEnabled = true`). The app sends nothing until Tor is ready. | **on by default** (Arti, `TorMode.ON`) |
| Relay messages that the app reads | `EVENT`, `EOSE`, `OK`, `NOTICE` | `EVENT`, `EOSE`, `OK`, `NOTICE` |
| `AUTH` (NIP-42), `CLOSED` | **ignored** | **ignored** |
| Receive limits | frame ≤ 512 KiB, ≤ 64 tags, ≤ 16 values for each tag, ≤ 1 KiB for each value; private envelope ≤ 64 KiB | — |
| Reconnection | backoff from 1 s, × 2, up to 300 s, ±20 %, **10 attempts**. A DNS error or an invalid server response is a permanent failure. The app tries again after **10 min**. | backoff from 1 s, × 2, up to 300 s, **no limit** |
| End of history | `EOSE`, or after 10 s | `EOSE` |
| Write acknowledgement | waits for `OK` only for courier drops (10 s) | no |

Results:
- A relay that requires NIP-42 accepts the connection. Then it refuses the requests, but the app
  does not know this. The app does not read the `AUTH` challenge or the `CLOSED` message.
- Tor is on by default in the two apps. Thus most users cannot use a relay that blocks Tor exits.
  An anti-bot check or a Tor exit block list has this effect.

### 1.6 What the clients check

- **Signature.** The apps check the signature of each event. iOS does this check outside the main
  thread.
- **Proof of work (NIP-13).**
  - iOS calculates 8 bits for each channel message. It stops after 2 s. If it does not reach
    8 bits, it sends the message without the full proof of work.
  - When iOS receives an event, it rejects nothing. If the event has a valid proof of work of
    8 bits or more, iOS applies a less strict rate limit to the sender. Source:
    `bitchat/Nostr/NostrPoW.swift`.
  - By default, Android does not calculate a proof of work.
  - The user can enable proof of work on Android (default: 12 bits). Then Android **rejects**
    messages below this level. Source: `nostr/GeohashMessageHandler.kt`.
  - In this configuration, Android **rejects iOS messages**, because they have only 8 bits.
- **Timestamp.**
  - iOS changes a timestamp in the future to the current time.
  - The mesh bridge (A.3) rejects events that are older than 15 min.
- **Duplicates.** The apps remove duplicates by event ID. When 5 relays send the same message,
  the app shows it one time.

The clients do not check the **relay** itself. They do not check:
- what the relay can do;
- how long it keeps events;
- which events it does not send.

### 1.7 Measurements on 2026-10-27

The test request was a `REQ` with `kinds:[20000,20001]`, `#g`, `since: −1 h`, `limit: 200`.
The prototype's `geohash_map` (see the note at the top) sent it without Tor. "Events" is the number of events that the relay
sent before `EOSE`.

| Cell | Relay (iOS rank / Android rank) | Events |
|---|---|---|
| `u0` | `nostr.sathoarder.com` (2/2), `nostr.hifish.org` (4/4) | **200** (the requested maximum) |
| `u0` | `nostr.0x7e.xyz` (–/5) | 0 |
| `u0` | `relay.pyramid.li` (3/–) | no answer in 8 s |
| `u09tv` | `relay.noeudlibre.fr` (–/3) | 4 |
| `u09tv` | 4 other relays that the test reached | 0 (`relay.mmwaves.de` is one of them, and it sends an AUTH challenge) |
| `u09tv` | `treuzkas.branruz.com` (5/–) | HTTP 302 redirect |
| `9q` | `nostr.2b9t.xyz` (4/3+4), `articles.layer3.news` (–/5) | 8 |
| `9q` | 4 other relays | 0 (`herbstmeister.com` is one of them, and it sends an AUTH challenge) |

How to read these results:
- "0" has two possible causes:
  - no user wrote in the cell;
  - the relay does not keep ephemeral events.

  This measurement cannot tell which cause applies (see A.5).
- **Only some relays give the history of the last hour.** These relays are not always the relays
  that the two apps share.
- **Two relays that iOS selects send NIP-42 challenges.** One of them also sends a redirect.

---

## 2. What the design assumes about relays

The current design has **six implicit assumptions**. Each assumption is weak.

| # | Assumption | Finding |
|---|---|---|
| H1 | All peers in a cell calculate **the same** relays. | False between iOS and Android. The two apps use two lists and two rules. The lists change at different rates. Measured: `#u09tv` has 8 servers, and only 2 of them are shared. |
| H2 | Each listed relay is **reachable and accepts writes** without authentication. | False for part of the list. Causes: NIP-42, redirects, invalid certificates, failures. |
| H3 | Each relay **keeps ephemeral events** for at least one hour. | Not standard: NIP-01 does not expect relays to store kinds 20000–29999. But the clients request `since: −1 h`. Measured: few relays send events. Annex A.5 gives the details. |
| H4 | Each relay **accepts Tor connections**. | The lists do not check this. The `georelays` crawler tests without Tor. |
| H5 | The **location** of a relay shows where it serves. | The location comes from IP geolocation. Many relays get the centre point of a city. Thus many distances are exactly equal. |
| H6 | The **list** is honest and up to date. | There is only one source, on GitHub. Android gets the list each day, and nobody reviews it. iOS reviews the list, but weeks late. |

---

## 3. Requirements for a geohash channel relay (v0.2)

These requirements describe a relay that the two apps **can use now, without changes**. Each
requirement refers to the fact that justifies it.

### 3.1 Connection

| Id | Requirement | Justification |
|---|---|---|
| R1 | The relay **MUST** serve `wss://` on port 443. It **MUST** use a public certificate that is valid for its name. | A certificate error is a permanent failure (section 1.5). |
| R2 | The relay **MUST** accept the WebSocket upgrade at the listed address, **with no redirect**. | A 301 or 302 response stops the connection (section 1.7). |
| R3 | The relay **MUST** accept connections from Tor exits. It **MUST NOT** send a browser challenge. | Tor is on by default in the two apps (section 1.5). |
| R4 | The relay **SHOULD** have **one** listed host name only (not `host` and `host:443`) and one location only. | Android counts duplicate entries two times. iOS rejects the full list if the locations are different. |

### 3.2 Access

| Id | Requirement | Justification |
|---|---|---|
| R5 | The relay **MUST** accept reads and writes **without NIP-42**. | The apps ignore `AUTH` and `CLOSED` (section 1.5). |
| R6 | The relay **MUST** accept unknown public keys. It **MUST NOT** require payment, an allow list or a web of trust. | Each cell has a new key (section 1.3). |
| R7 | If the relay requires proof of work, the level **MUST NOT** be more than 8 bits. The relay **SHOULD NOT** require proof of work. | iOS calculates 8 bits. Android calculates 0 bits by default (section 1.6). |
| R8 | For each connection, the rate limits **SHOULD** accept: <br>• one presence event about each 40 s, for each of 3 cells; <br>• bursts of messages; <br>• **more than 20** subscriptions at the same time. | Presence frequency and sampling (section 1.2). iOS keeps up to 64 pending subscriptions for each relay. |

### 3.3 Events and filters

| Id | Requirement | Justification |
|---|---|---|
| R9 | The relay **MUST** accept kinds **20000 and 20001**. It **MUST** send them to live subscribers. | These kinds are the core of the channel. |
| R10 | The relay **MUST** index the single-letter tags **`g`** and **`r`**. It **MUST** accept filters with 9 or more values. | Filters `#g` and `#r` on a cell and its 8 neighbours (section 1.2). |
| R11 | The relay **SHOULD keep kinds 20000 and 20001 for at least 1 h**, and kind 20001 for 24 h if possible. It **SHOULD** send up to 200 events before `EOSE`. | The two apps request this history (section 1.2). This is **a deliberate difference from NIP-01**. The relay should declare it (R16). Annex A.6 discusses this choice. |
| R12 | The relay **MUST** store kind **1** events that have a `g` tag. It **SHOULD** apply NIP-40 (expiration) and NIP-09 (deletion). | Location notes and board notes (section 1.2). |
| R13 | The relay **SHOULD** accept events of at least **64 KiB**. Measurements must confirm this value (section 7). | iOS accepts private envelopes up to 64 KiB, and courier drops up to 20 KiB. Nobody has measured the maximum size of a channel message. |
| R14 | The relay **MUST** send `OK` for each `EVENT` and `EOSE` for each `REQ`. | Without `EOSE`, iOS waits 10 s. Courier drops need `OK`. |
| R15 | The relay **SHOULD NOT** reject an event whose `created_at` is a few minutes in the future. | Phone clocks are not exact. iOS accepts this difference. |

### 3.4 Transparency

| Id | Requirement | Justification |
|---|---|---|
| R16 | The relay **SHOULD** publish a NIP-11 document that declares its limits and its retention. The document should include `limitation.auth_required`, `payment_required`, `restricted_writes`, `min_pow_difficulty`, `max_subscriptions`, `max_message_length`, and `retention` for kinds 20000, 20001 and 1. | Clients can then qualify relays **automatically** (section 5, A6). |
| R17 | The relay address **SHOULD** stay the same over time. | The lists and the app caches use the address. |

### 3.5 What this specification does not require

- A relay for BitChat only is not necessary. A general relay that agrees with R1 to R15 is
  sufficient.
- The relay does not need to be trusted for content. All events are signed, and the clients check
  them.
- The relay can see the cell. The `g` tag is in clear text, and the design does not hide it. Tor
  hides the IP address.

---

## 4. Is the selection of 5 relays necessary?

**No, not in its current form.** The real need is a **rendezvous** (B.7): a message that one peer
writes in a cell must reach the other peers of the cell. "The 5 nearest relays, from a shared
list" is only one method to get a rendezvous.

This method mixes four objectives. Each objective must be examined separately.

| Objective | Real need | Does the current selection give it? |
|---|---|---|
| **Rendezvous** | Writers and readers share at least one relay **that operates** | Only if all peers have the same list, the same rule and the same live relays (H1, H2) |
| **Load distribution** | Different cells use different relays | Badly. Many relays have the same city centre point. With the Android rule at precision 2, 322 cells of 1,024 have only 4 different servers. |
| **Proximity** | Lower latency; neighbour cells on the same relays | Yes. But Tor adds its own latency, so the gain is small. Proximity is useful for location notes, which read 9 neighbour cells. |
| **Resilience** | No single point of failure, censorship or capture | No (see below) |

Security weaknesses of selection by distance:

- **Capture of a cell.**
  - When distances are equal, the first entry wins. On iOS, this is the lowest host name in
    alphabetical order. On Android, this is the position in the CSV file.
  - Thus a person can rent some servers that geolocate at the centre point of a city. These
    servers can then take the 5 places of this cell.
  - On the Android list, this occurs automatically, because nobody reviews the list.
  - If a server is listed as `host` and as `host:443`, it counts two times.
  - This scenario is **theoretical**. Nobody has tested it.
  - A capture lets the attacker **censor** the cell (no forwarding) or **observe** it (keys,
    times, and IP addresses when Tor is off).
- **Saturation.** One cell depends on 5 servers or fewer. If these servers stop, the cell stops.
  No other relay replaces them.
- **Single point of control.** One organisation publishes the list, on one platform. The entity
  that controls the list controls the rendezvous of all cells.

---

## 5. Alternative approaches

This section examines each approach against seven criteria:
- **Decentralised:** there is no single authority.
- **Tamper-proof:** nobody can forge the data or change the relay selection.
- **Saturation-resistant:** a targeted denial of service does not stop the cell.
- **Capture-resistant:** a third party cannot take the relays of a cell.
- **Privacy.**
- **Compatibility** with the current apps.
- **Cost.**

### A0. Correct the current design, and keep its principle

- **Use one list and one rule** for the two apps, for example the iOS ones. Give the list a
  version number and a signature.
- **Write wide, read narrow.** Write to the first *w* candidates and read from the first *r*
  candidates (for example w = 8, r = 5).
  - All peers sort the candidates in the same order. But each peer can lose some of them: dead
    relays, or relays that are not in its copy of the list.
  - The reader reads the first *r* relays that it can reach. All of them are in the first *w*
    positions of the shared order.
  - The rendezvous operates while the writer has lost fewer than *r* of these relays.
  - Cost: w/r times more writes.
- **Replace failed relays** (dead, or NIP-42 required) with the next relay in the order. This
  replacement is safe only together with "write wide". Then two peers that see different dead
  relays still have a rendezvous.
- **Use `includeDefaults`** (already in the Android code). This adds 1 or 2 relays that all cells
  share. It makes sure that the rendezvous operates. The disadvantage: more load on these relays.

| Criterion | Result |
|---|---|
| Decentralised | no (one list) |
| Tamper-proof | partly (if the list is signed) |
| Saturation-resistant | partly |
| Capture-resistant | no |
| Compatibility | small change in the two apps |
| Cost | low |

### A1. Rendezvous hashing (HRW) instead of distance

Each candidate relay gets a score. Each peer keeps the *k* best scores:

```text
score(relay, cell) = H(epoch_salt ‖ prefix(cell, p) ‖ normalised_url(relay))
```

- **Uniform distribution.** A large city does not overload 5 servers. There are no equal scores in
  practice.
- **Stability.** When a relay goes into the list or out of it, only the cells that selected this
  relay change. Two lists with small differences give almost the same *k* relays. This is the
  iOS/Android case.
- **Prefix *p*** (for example 4). A cell and its neighbours usually share their relays. Location
  notes need this, because they read 9 cells at the same time.
- **Epoch salt.**
  - Without a salt, an attacker can try many host names until one wins a given cell.
  - The salt changes at each epoch (for example each day), and nobody can know it in advance.
    One possible source is the hash of a Bitcoin block at a fixed height.
  - Thus host names that the attacker prepares in advance have no value.
- **Disadvantage:** the selection does not use proximity. A distant relay can serve a cell. With
  Tor, this effect is small.

| Criterion | Result |
|---|---|
| Decentralised | no (it still uses the list) |
| Tamper-proof | yes for the calculation, if salted |
| Saturation-resistant | better distribution |
| Capture-resistant | good, if salted |
| Compatibility | new rule; the two apps must change at the same time |
| Cost | low |

### A2. Decentralised directory from monitors (NIP-66)

With NIP-66, independent **monitors** publish relay discovery events on Nostr:
- kind 30166 for each relay, with tags for supported NIPs, requirements (auth, payment, proof of
  work), latency and geohash;
- kind 10166 to announce the monitor itself.

The tag details must be checked against the current version of NIP-66.

- The app replaces the GitHub list with the **intersection of the reports** from many monitors.
- A relay is a candidate only if:
  - *m* of *n* monitors report it;
  - it agrees with the requirements of section 3 (no auth, no payment, Tor accepted, sufficient
    retention).
- The monitors **test** the relays themselves. They use Tor, write a kind 20000 event and read it
  back. Thus the monitors do not depend only on NIP-11 documents, which the relays write about
  themselves.
- This approach can work together with A1: the directory gives the candidates, and HRW selects
  from them.

| Criterion | Result |
|---|---|
| Decentralised | yes (many monitors) |
| Tamper-proof | yes (signed, *m*-of-*n* threshold) |
| Saturation-resistant | yes |
| Capture-resistant | medium (a hostile relay that agrees with the rules can get in) |
| Compatibility | new source for the list |
| Cost | medium: independent monitors are necessary |

### A3. Relays announced in presence events (self-organisation)

- Each presence event (kind 20001) adds the relays where its author writes, for example with a
  `["relay", url]` tag.
- A reader adds a relay to its read set when *k* different keys of the cell announce it.
- A proof of work on presence events makes fake keys expensive. Without it, a Sybil attack (A.4)
  is easy.
- The cell **converges by itself** to a shared set, even if the initial lists are different. An
  iOS peer and an Android peer find each other.
- **Disadvantages:**
  - A first shared relay is necessary at the start: default relays or A1.
  - A Sybil attack (A.4) is possible if the proof of work is weak.
  - The current apps ignore the tag, so the change must stay compatible with them.

| Criterion | Result |
|---|---|
| Decentralised | yes |
| Tamper-proof | yes for the announcement (signed), but anybody can announce |
| Saturation-resistant | yes (the set can grow) |
| Capture-resistant | medium |
| Compatibility | upward compatible (older apps ignore the tag) |
| Cost | low |

### A4. Republisher peers: connect the relay sets

- The events are **signed**. Thus any peer can publish them again, and nobody can change them.
- A client or a bot **subscribes to the union** of the iOS relays and the Android relays of a cell.
  It publishes on each set the events that it receives from the other set.
- The prototype's bot (`geohash_bot`) **already read the union**. bitchat-console publishes
  the events again (`--republish`).
- To limit amplification:
  - remove duplicates by event ID;
  - publish only to the relays that do not have the event;
  - set a limit for each minute.
- **Result:** the two sets connect, and the apps do not change. This is **the only approach that
  you can use now.**

| Criterion | Result |
|---|---|
| Decentralised | partly (it depends on the republisher peers) |
| Tamper-proof | yes |
| Saturation-resistant | medium |
| Capture-resistant | helps against censorship of one of the two sets |
| Compatibility | **no change to the apps** |
| Cost | low |

### A5. Federation of synchronised relays (NIP-77, negentropy)

- Relays in the same region **synchronise** with each other, for example with `strfry sync` and
  NIP-77. They exchange kinds 20000, 20001 and 1 that have a `g` tag.
- A client does not need *the same* relay as its neighbours. It needs only one relay of the
  federation.
- The events are signed. A federation relay can only **omit** events. It cannot change them. A
  comparison of relays shows an omission.
- **Disadvantage:** the operators must cooperate. This is infrastructure, not a client protocol.

| Criterion | Result |
|---|---|
| Decentralised | partly (many operators) |
| Tamper-proof | yes |
| Saturation-resistant | yes (redundancy) |
| Capture-resistant | medium |
| Compatibility | no change to the apps (if the relays are in the lists) |
| Cost | medium |

### A6. Automatic relay qualification (NIP-11 and tests)

This approach does not replace the rendezvous. It is a **filter** to use together with A0, A1
or A2.
- Before the client uses a relay, it reads the NIP-11 document of the relay. It removes relays
  that declare `auth_required`, `payment_required`, `restricted_writes`, or
  `min_pow_difficulty` more than 8.
- The client can also remove relays that keep kinds 20000 and 20001 for less than 1 h.
- NIP-11 is **declarative**. A real test must confirm it: write a kind 20000 event, then read it
  back through Tor. The monitors of A2 can do this test.
- **Caution:** if each client filters with its own tests, the peers do not see the same set. Then
  "write wide" from A0 is necessary, or a shared filter that the monitors publish.

### A7. Mechanisms that already exist outside relay selection

- **BLE mesh** and **gateway** (B.5): iOS already sends signed events from a peer without Internet
  to a peer with Internet. Source: `Services/Gateway/GatewayService.swift`.
- **Mesh bridge** (A.3), with the `r` tag.

These mechanisms do not replace Nostr. But they show that the app can already move signed events
through more than one path.

### Summary

| | Decentralised | Tamper-proof | Saturation-resistant | Capture-resistant | No app change | Cost |
|---|---|---|---|---|---|---|
| A0 Correct the design | ✗ | ~ | ~ | ✗ | ✗ (small) | low |
| A1 Salted HRW | ✗ | ✓ | ~ | ✓ | ✗ | low |
| A2 NIP-66 monitors | ✓ | ✓ | ✓ | ~ | ✗ | medium |
| A3 Relays in presence | ✓ | ~ | ✓ | ~ | ~ (upward) | low |
| A4 Republisher peers | ~ | ✓ | ~ | ~ | **✓** | low |
| A5 Synchronised federation | ~ | ✓ | ✓ | ~ | ✓ | medium |
| A6 Qualification | — | — | — | — | ✗ | low |

✓ yes · ~ partly · ✗ no

---

## 6. Recommendation

Use three steps, from the most immediate to the most structural.

1. **Now, with no change to the apps (A4).** Use one or more republisher peers. They publish the
   events of the cells that they follow between the iOS set and the Android set.
   bitchat-console's `--republish` does this.
2. **Proposals for the two apps (A0 + A6):**
   - one shared rule and one shared list;
   - write wide, read narrow;
   - replace failed relays;
   - read `AUTH` and `CLOSED`;
   - set `includeDefaults` to add 1 or 2 shared relays.
3. **Target (A1 + A2, then A3):**
   - select relays with salted hashing, from candidates that many NIP-66 monitors report against
     the requirements of section 3;
   - then let each cell converge through the relays that peers announce in their presence events.

Each step stays useful after the next step.

---

## 7. Open questions and measurements to do

1. **Retention of ephemeral events (assumption H3, Annex A.5).**
   - For each relay of the two lists, write a kind 20000 event. Then read it back after 5 min,
     1 h and 24 h.
   - This measurement confirms or rejects H3.
   - The test must write to a channel. Use a test cell, or a test kind.
2. **Access through Tor.** Do the reachability tests again through Tor. Compare the results with
   the tests without Tor.
3. **NIP-11.** Collect the NIP-11 documents of the listed relays. Count the relays that A6 would
   remove.
4. **Cost of a capture.** For some large cities, count the servers that an attacker must put at the
   centre point to take the 5 places. Do this for iOS and for Android.
5. **Real frequency.** Measure the rate of kinds 20000 and 20001 in active cells. Use the results
   to set the values of R8 and to calculate the cost of A0 ("write wide") and A4 (republication).
6. **NIP-66.** Check the current format of kinds 30166 and 10166. Make a list of the monitors that
   exist.
7. **Relay tag name (A3).** Select a name that is different from `r`. The iOS mesh bridge already
   uses `r`.

---

## Annex A: Definitions of the requested terms

Each definition has a functional part (what the user gets) and a technical part (how it operates).
It also gives the related NIPs and RFCs.

### A.1 Private message

**Functional.** A message from one person to one other person. Only the recipient can read it. In
a geohash channel, the user opens a private message from the list of participants of the cell.

**Technical.**
- The message uses three nested layers:
  - a rumor: kind 14, unsigned, with the real time;
  - a seal: kind 13, signed by the sender, encrypted to the recipient;
  - a gift wrap: kind 1059, signed by a one-time key, with a `p` tag for the recipient.
- In a geohash channel, the sender and the recipient use the keys of the cell (section 1.3).
- The apps send private messages to the **default relays**, not to the geohash relays.
- The app subscribes with `kinds:[1059]`, `#p:[own key]`, `since: −24 h`.
- The seal and the wrap have random timestamps. Only the rumor has the real time.

**References.**
- NIP-01 (events, signatures).
- NIP-17 and NIP-59 define the kind numbers 14, 13 and 1059.
- **The BitChat format is not compatible with NIP-17, NIP-44 or NIP-59.** The key derivation and
  the payload are BitChat-specific (source: `bitchat/Nostr/NostrProtocol.swift`). bitchat-console
  implements this format.
- RFC 5869 (HKDF), RFC 8439 (ChaCha20-Poly1305; BitChat uses the XChaCha20 variant), BIP-340
  (Schnorr signatures).

### A.2 Courier drop

**Functional.**
- A message for a person who is not near the sender on the BLE mesh.
- The message waits on relays until the recipient, or a gateway near the recipient, collects it.
- This function is on iOS/macOS only, and only when the user enables the mesh bridge.

**Technical.**
- The event has kind 1401. It is a stored (regular) kind.
- The content is a courier envelope, encrypted and authenticated with the Noise protocol
  (pattern X) to the static key of the recipient. The maximum size is 20 KiB.
- The tag `x` contains a 16-byte recipient tag that changes each UTC day. Only a party that knows
  the static Noise key of the recipient can calculate it.
- The tag `expiration` makes the relays remove the event when the envelope expires.
- A new one-time key signs each drop. Thus relays cannot link the drops of one sender.
- The apps publish drops to the **default relays**, because the sender and the recipient do not
  share a cell.
- The app waits for `OK` from the relay before it considers the drop as delivered.
- Source: `bitchat/Services/Gateway/BridgeCourierService.swift`.

**References.**
- NIP-01 (regular events, `OK`).
- NIP-40 (expiration).
- No NIP defines kind 1401, as far as we know. It is BitChat-specific.
- Noise Protocol Framework (noiseprotocol.org, revision 34). It is not an RFC.

### A.3 Mesh bridge ("the bridge" in section 1.6)

**Functional.**
- An option on iOS/macOS. It connects BLE mesh "islands" that are in the same place but out of
  radio range of each other.
- Public mesh messages of one island appear in the other islands. They have a "bridged" mark.
- A device without Internet can give its messages to a nearby device that has the gateway option
  on (B.5).

**Technical.**
- The device signs each public mesh message again as a kind 20000 event, with a key that is
  derived for the cell.
- The event has these tags:
  - `r` = the cell, geohash precision 6 (about 1.2 km);
  - `n` = nickname;
  - `m` = [stable message ID, mesh sender ID, timestamp in ms].
- Presence uses kind 20001 with the `r` tag only.
- The `r` tag is different from `g`. Thus mesh bridge traffic does not appear in the geohash
  channels, which filter on `#g`.
- The device publishes to the 5 geohash relays of its cell. It subscribes to `#r` for its cell and
  the 8 neighbour cells, `since: −15 min`.
- The device rejects events that are older than 15 min (`maxEventAgeSeconds`). This is the check
  in section 1.6.
- Rules prevent loops. For example, the device never publishes again an event that it got from
  the mesh.
- Source: `bitchat/Services/Gateway/BridgeService.swift`.

**References.**
- NIP-01 (ephemeral kinds, single-letter tags).
- No NIP defines this use of the `r` tag or of the `m` tag.
- In NIP-01 practice, `r` usually contains a URL. BitChat uses `r` for a geohash.

### A.4 Sybil attack (approach A3)

**Functional.** One attacker makes many false identities. The attacker then uses them to look like
many independent users. In A3, the false identities can announce a relay that the attacker
controls. Thus the attacker can move a cell to this relay.

**Technical.**
- In BitChat, a new identity costs nothing. A key is 32 random bytes, and relays do not check
  identity (section 1.3).
- A rule like "accept a relay when *k* different keys announce it" is weak if keys are free.
- Countermeasures:
  - make each identity expensive, for example with a proof of work (NIP-13) on each presence
    event;
  - weight the announcements by age or activity in the cell;
  - always keep a minimum of shared relays from another source (A1, A2).

**References.**
- J. R. Douceur, "The Sybil Attack", IPTPS 2002. This is the original definition.
- NIP-13 (proof of work) is a cost countermeasure.
- No RFC or NIP defines the Sybil attack itself.

### A.5 Assumption H3 (section 7, item 1)

**Statement.** "Each geohash relay keeps ephemeral events (kinds 20000 and 20001) for at least one
hour, and sends them in reply to a request with `since`."

**Why it is important.**
- When a user opens a channel, the app requests the last hour (`since: −1 h`, `limit: 200`).
- Android also requests 24 h of presence.
- If the relays do not keep these events, a new user sees an empty channel. The user sees only the
  messages that arrive after the subscription.
- Presence counts are then also wrong.

**Why it is uncertain.** NIP-01 says that relays are "not expected to store" ephemeral events.
Thus each relay chooses its own behaviour:
- some relays forward the events and keep nothing;
- some relays keep them for some minutes;
- some relays keep them like regular events.

The apps do not check which behaviour applies.

**How to test it.**
- A read-only test (section 1.7) cannot separate "no event was written" from "the relay keeps
  nothing".
- A real test must write a known event, then read it back after a known delay (5 min, 1 h, 24 h).
- The test must also read back through a different connection, because some relays send the
  event only to live subscriptions.

### A.6 Why ephemeral kinds when the protocol needs history

**The contradiction.** BitChat uses ephemeral kinds (20000, 20001) for channel messages and
presence. NIP-01 does not expect relays to store these kinds. But the apps request one hour of
history. Thus the protocol depends on relay behaviour that the standard does not give.

**Probable reasons for ephemeral kinds.** The code does not state the reasons. These reasons are
deduced from the code and the design:
1. **Privacy.**
   - A location chat should not leave a permanent public record.
   - Ephemeral kinds tell relays not to keep the events.
   - This agrees with the design of BitChat: one key for each cell, Tor by default, and a "panic
     wipe" function.
2. **Load.**
   - Presence events go out about every 40 to 80 s, from each device, for each cell.
   - If relays stored all of them, storage would grow fast for no value.
3. **Acceptance.**
   - Some relays reject or limit regular events from unknown keys.
   - Ephemeral events cost less to the relay, so relays often accept them more easily.
4. **Live use.** The main use is real-time chat. History is a convenience at channel opening. The
   protocol does not depend on history for correctness.

**Consequence.** History is "best effort". It depends on each relay. Requirement R11 makes this
need explicit.

**Options for the specification** (proposals):

| Option | Description | Advantages | Disadvantages |
|---|---|---|---|
| E1. Keep ephemeral kinds, and declare retention | Relays declare `retention` for kinds 20000 and 20001 in NIP-11 (R16). Clients prefer relays that keep events for at least 1 h (A6). | No change to the event format. Privacy stays the default. | Depends on relay goodwill and on correct declarations. |
| E2. Regular kind with NIP-40 expiration for messages | Messages use a stored kind with `expiration` = now + 1 h (or 24 h). Presence stays ephemeral. | History becomes standard behaviour. | A relay that ignores NIP-40 keeps the messages forever, which is a privacy risk. It needs a new kind and a change in the two apps. |
| E3. History from peers, not from relays | A new user asks the peers of the cell for recent messages (for example through A4 republishers or a request event). | Relays need no retention. | More complex. History from peers can be incomplete. A filter against spam is necessary. |

Our recommendation is E1 now, because it does not change the apps. E2 needs a decision of the
BitChat developers on privacy.

**References.**
- NIP-01: event kind ranges (regular, replaceable, ephemeral 20000–29999, addressable).
- NIP-11: relay information document, with `limitation` and `retention`.
- NIP-40: expiration timestamp.

---

## Annex B: Other terms

| # | Term | Definition | Reference |
|---|---|---|---|
| B.1 | Geohash | A text code for a rectangle on the Earth. Each additional character makes the rectangle about 32 times smaller. | Public domain algorithm (G. Niemeyer, 2008); no RFC |
| B.2 | Cell | The rectangle of one geohash. A channel is attached to one cell. | — |
| B.3 | Geohash relays | The 5 relays that an app calculates for a cell from its relay list (section 1.4). | — |
| B.4 | Default relays | The relays that are fixed in the app code, plus the relays that the user adds (section 1.1). | — |
| B.5 | Gateway | An iOS/macOS option. The device publishes to relays the signed events of mesh peers that have no Internet. It also sends relay events to the mesh. The gateway cannot change the events. | Source: `Services/Gateway/GatewayService.swift` |
| B.6 | Board note | A post on the BitChat bulletin board (mesh), also published as kind 1 with an expiration. Removal uses kind 5. | NIP-01, NIP-09, NIP-40 |
| B.7 | Rendezvous | The condition where a writer and a reader in the same cell use at least one shared relay that operates. | — |
| B.8 | Ephemeral event | An event of kind 20000 to 29999. Relays are not expected to store it. | NIP-01 |
| B.9 | Proof of work | Leading zero bits in the event ID, found by changing a `nonce` tag. | NIP-13 |
| B.10 | HRW | Highest Random Weight hashing (rendezvous hashing). Each client calculates the same top *k* without coordination. | Thaler and Ravishankar, 1998; no RFC |
| B.11 | Negentropy | A protocol that finds the differences between two sets of events with little data. | NIP-77 |
| B.12 | Relay monitor | A service that tests relays and publishes signed reports on Nostr. | NIP-66 |
| B.13 | Republisher peer | A client or bot that publishes signed events again between relay sets (approach A4). | — |
| B.14 | Teleport | A user who reads or writes in a cell where the device is not. The app adds the tag `t`=`teleport`. | BitChat-specific |

---

## Annex C: Source files

**iOS/macOS** (`permissionlesstech/bitchat` @ `04995f6`):
- `bitchat/Nostr/NostrProtocol.swift`: kinds, tags, event construction
- `bitchat/Nostr/NostrRelayManager.swift`: default relays, Tor, reconnection, relay messages, filters
- `bitchat/Nostr/GeoRelayDirectory.swift`: relay list and selection
- `bitchat/Nostr/NostrPoW.swift`: proof of work
- `bitchat/Nostr/NostrIdentityBridge.swift`: one key for each cell
- `bitchat/Services/TransportConfig.swift`: constants (history, limits, backoff)
- `bitchat/Services/GeohashPresenceService.swift`: presence (40–80 s, precisions 2, 4, 5)
- `bitchat/Services/LocationNotesManager.swift`: location notes
- `bitchat/Services/Board/BoardManager.swift`: board notes, NIP-40, NIP-09
- `bitchat/Services/Gateway/BridgeService.swift`, `GatewayService.swift`, `BridgeCourierService.swift`: mesh bridge, gateway, courier drops
- `bitchat/ViewModels/GeohashSubscriptionManager.swift`, `GeoChannelCoordinator.swift`, `ChatViewModelBootstrapper.swift`, `NostrInboundPipeline.swift`: subscriptions, sampling, wiring
- `bitchat/Services/NetworkActivationService.swift`: Tor on by default

**Android** (`permissionlesstech/bitchat-android` @ `eed8819`, under `app/src/main/java/com/bitchat/android/`):
- `nostr/NostrProtocol.kt`, `nostr/NostrFilter.kt`: events and filters
- `nostr/NostrRelayManager.kt`: default relays, `includeDefaults`, send and subscribe by geohash
- `nostr/RelayDirectory.kt`: relay list and selection
- `nostr/NostrSubscriptionManager.kt`, `nostr/NostrBackgroundRuntime.kt`: subscriptions, presence
- `geohash/GeohashNostrPrivacyPolicy.kt`: presence only at precision 5 or less
- `mesh/PowerManager.kt`: presence frequency (40–80 s in the foreground, up to 15–16.5 min in the background)
- `nostr/PoWPreferenceManager.kt`, `nostr/GeohashMessageHandler.kt`: proof of work (off by default, 12 bits)
- `nostr/LocationNotesManager.kt`: location notes
- `net/TorPreferenceManager.kt`, `net/ArtiTorManager.kt`: Tor on by default
