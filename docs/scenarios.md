# Relay scenarios: what the map shows, and what we conclude

For the users of [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) (the map) and of
[bitchat-launcher](https://github.com/wvmscs/bitchat-launcher), who see a **relay strategy**
selector in the map's panel and want to know what each choice means. State of 2026-11-05.

## Why it matters

BitChat's location channels have no central server. To talk in a place (a **cell**, written as a
geohash such as `u09tv`), every phone must guess **the same few Nostr relays** for that place,
publish there and listen there. Two phones that guess differently never see each other's
messages, without any error. How a phone guesses is its **strategy**. The map lets you see, cell
by cell, which relays each strategy would choose and whether its phones would still exchange
messages with the BitChat apps as they are today.

## How to read the map

- **The selector** (under the area, in the panel) picks the strategy. "Today (AS)" is what the
  BitChat apps do now; the others are the alternatives this project studies.
- **The relays** shown on the map, and the lines from a hovered cell, are those the selected
  strategy would choose for that cell.
- **The badges** in a cell's details: green when a phone using the strategy would exchange
  messages with **bitchat iOS** / **bitchat Android** as released today, red when it would not
  (no working relay in common), grey when that app's relay list is not loaded. Under "Today
  (AS)", one badge: whether iOS and Android phones exchange with each other.
- **The panel** sums it up over the watched cells: "exchange in 31 of 33 cells".
- Relays found dead by the map are set aside: a strategy that picks dead relays shows it.

## The scenarios

| In the selector | In one line | Used by |
|---|---|---|
| Today (AS) | each app picks the 5 relays nearest the cell, each from its own list and with its own rule | bitchat iOS, bitchat Android |
| AirHop (A8) | the 10 nearest relays, from the list Android uses | AirHop, a third app compatible with BitChat |
| A0 | one list for everybody, the same rule, 8 relays to publish to and 5 to read, dead relays replaced | proposed |
| A1 | relays drawn by a hash of the cell instead of by distance | proposed |
| A2 to A6 | listed, not selectable (see below) | studied |

The figures below compare a phone using each scenario with the apps as released, over 500
cells drawn at random. "On paper": every relay of the lists works. "Working relays": only the
relays that answered our census of 2026-10-30 carry anything; the lists still name the others.

| Scenario | Exchanges with bitchat iOS (paper / working) | with bitchat Android | Two users of the scenario (working) |
|---|---|---|---|
| Today (AS): iOS with Android | 95 % / **68 %** | — | iOS 97 %, Android 98 % |
| AirHop (A8) | 96 % / 78 % | 100 % / 94 % | 100 % |
| A0 | 96 % / 68 % | 100 % / **98 %** | 98 %, 100 % with dead relays replaced |
| A1 | 5 % / **2 %** | 7 % / **4 %** | 99 % |

### Today (AS)

What the apps do. iOS reads a list reviewed by hand, last updated on **2026-07-26**; Android
reads the list regenerated every day. Each app also has its own way of breaking ties.

- **Problem:** the iOS list has aged. Half of its 326 relays no longer answer: an iPhone has on
  average **2 working relays** out of 5 in a cell. iOS and Android users meet in 95 % of the
  cells on paper, in **two cells out of three** in reality.
- **Gain:** none to expect without changing the apps.

### AirHop (A8)

AirHop takes the 10 nearest relays, the 5 BitChat picks and 5 more as backups, from the list
Android uses, frozen in each release of the app.

- **Gain:** more relays per cell make up for dead ones: it meets Android in 94 % of the cells and
  iOS in 78 %, better than iOS and Android meet each other.
- **Problem:** the list is frozen in the app: a phone that is not updated drifts as iOS has.
  Twice as many connections per cell.

### A0: one list, the same rule, more relays

Every client reads the same list with the same rule, publishes to 8 relays and reads 5, and
replaces a relay it finds dead with the next one.

- **Gain:** all A0 users meet in every cell, even with dead relays and lists a month apart. It
  stays compatible with today's Android app (98 %), which already reads the same list.
- **Problem:** it meets today's iOS app only where iOS's old list agrees (68 %), until iOS
  reads a fresh list. Like today, an attacker who rents a few servers near a city can take that
  city's cells over.

### A1: relays drawn by a hash

Each cell gets relays drawn by a hash of the cell and of a shared secret value (a salt), among
all the listed relays, wherever they are.

- **Gain:** to take a city over, an attacker would need more than 60 servers instead of 5; the
  load spreads over 332 servers instead of 119; a list change moves fewer cells. All A1 users
  meet (99 %).
- **Problem:** **it breaks compatibility**: a phone using A1 exchanges with today's apps in only
  2 to 4 % of the cells. Its relays may also be far away (slower). It needs the salt to be shared
  by all clients.

### Not selectable

| Scenario | What it is | Why the map cannot show it |
|---|---|---|
| A2 | relays chosen from the reports of NIP-66 monitors | it needs trusted monitors that test writes through Tor; few exist |
| A3 | relays announced by the users of a cell in their presences | the apps do not announce them yet |
| A4 | republishers: peers that copy events between the iOS and Android relay sets | not a choice of relays; it runs beside the apps (`bitchat-console --republish`) |
| A5 | a federation of relays that synchronise with each other | no federation runs yet |
| A6 | qualification: set aside the relays that refuse what BitChat needs | a filter added to any strategy; the map already sets aside the relays it finds dead |

## Our conclusions

1. **Today's biggest problem is the age of the lists, not the rules.** iOS and Android lose a third
   of their encounters because the iOS list is three months old. **Gain without changing any
   code:** refresh the iOS list. A list qualified and published by a few trusted parties, read by
   every client, would remove the drift for good.
2. **Without changing the apps:** republishers (A4) connect the iOS and Android relay sets in the
   cells they follow. It is the only immediate lever on the apps as released.
3. **For a new or updated client: A0 with qualification (A6).** It brings redundancy and the
   replacement of dead relays, and keeps exchanging with today's Android app; with today's iOS
   app as soon as iOS reads a fresh list.
4. **A1 is the target, not a first step.** It resists capture and spreads the load, but cuts the
   link with today's apps. It needs every client to move together, or a transition period during
   which clients publish to both the old and the new relays.
5. **New clients copy the apps' quirks.** AirHop chose more relays; others freeze old lists in
   their code. The more clients there are, the more a shared, qualified list and a written rule
   matter.

## Learn more

- [clients.md](clients.md): what each app does exactly, with sources.
- [evaluation.md](evaluation.md): the comparison of the strategies on frozen lists and on the
  real network.
- [geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md):
  the requirements for a relay and the alternatives A0 to A6 in detail.

The figures come from `cargo run --release --example clients` and the bench of this repository,
on the relay lists frozen in `tests/data` and the census of 2026-10-30. The census is one reading
through Tor, from one place, on one day: a relay that was slow that day counts as not working.
