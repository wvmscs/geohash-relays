# Relay scenarios: what the map shows, and what we conclude

The tools [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) (the map) and
[bitchat-launcher](https://github.com/wvmscs/bitchat-launcher) show the outcome of the
**relay strategies** that we study.

The goal is to agree on a **relay strategy** that improves the ability of the different
implementations to work together.

Compiled versions of the tools are available on the
[current release](https://github.com/wvmscs/bitchat-launcher/releases/latest).

## Why it matters

BitChat's location channels have no central server. To talk, every phone must guess **the same
few Nostr[^1] relays**. BitChat associates a list of relays with each geohash cell (such as
`u09tv`), and makes this cell a place for users to meet.

Two phones in the same cell that get different relays may not see each other's messages, and no
error shows it.

How a phone guesses is its **strategy**. The map lets you see, cell by cell, which relays each
strategy would choose, and whether its phones would still exchange messages with the BitChat apps
released today.

[^1]: [Nostr](https://github.com/nostr-protocol/nips): an open protocol where clients publish
    signed messages to relays, servers that anybody can run. What BitChat needs from it: the
    relays store and forward the messages, so two phones do not have to be online at the same
    time; more than 3,000 relays are known, and their number grows with use; the protocol keeps
    evolving (for example, payments over Lightning).

## How to use the map

- **The selector** (under the area, in the panel) picks the strategy. "Today (AS)" is what the
  BitChat apps do now; the others are the alternatives this project studies.
- **The relays** shown on the map, and the lines from a hovered cell, are those the selected
  strategy would choose for that cell.
- **The badges** in a cell's details: green when a phone using the strategy would exchange
  messages with **bitchat iOS** / **bitchat Android** as released today, red when it would not
  (no working relay in common), grey when that app's relay list is not loaded. Under "Today
  (AS)", one badge: whether iOS and Android phones exchange with each other.
- **The panel** sums it up over the watched cells: "exchange in 31 of 33 cells".
- Relays that the map finds dead, or that ask for authentication, are set aside: a strategy that
  picks them shows it.

## The scenarios

These are the scenarios that we study for Project B2 (provisional name).

We added our understanding of the AirHop strategy to this problem, and we are analysing the
solution that Sonar proposes.

| In the selector | In one line | Used by |
|---|---|---|
| Today (AS) | each app picks the 5 relays nearest the cell, each from its own list and with its own rule | bitchat iOS, bitchat Android |
| AirHop (A8) | the 10 nearest relays, from the list Android uses | AirHop, a third app compatible with BitChat |
| A0 | one list for everybody, the same rule, 8 relays to publish to and 5 to read, dead relays replaced | proposed |
| A1 | relays drawn by a hash of the cell instead of by distance | proposed |
| A2 to A6 | listed, work in progress (see below) | |

The figures below compare a phone using each scenario with the bitchat apps as released today,
over 500 cells drawn at random. "On paper": every relay of the lists works. "Working relays":
only the relays that answered our check of 2026-10-30 count. Phones still choose the others, but
nothing gets through them.

| Scenario | Exchanges with bitchat iOS (paper / working) | with bitchat Android | Two users of the scenario (working) |
|---|---|---|---|
| Today (AS): iOS with Android | 95 % / **68 %** | — | iOS 97 %, Android 98 % |
| AirHop (A8) | 96 % / 78 % | 100 % / 94 % | 100 % |
| A0 | 96 % / 68 % | 100 % / **98 %** | 98 %, 100 % with dead relays replaced |
| A1 | 5 % / **2 %** | 7 % / **4 %** | 99 % |

### Today (AS)

What the apps do. iOS reads a list reviewed by hand, last updated on **2026-07-26**; Android
reads the list regenerated every day. Each app also has its own way of breaking ties.

- **Gain:** none to expect without frequent and synchronised updates of the lists.
- **Problem:** the iOS list has aged. Half of its 326 relays no longer answer: an iPhone has on
  average **2 working relays** out of 5 in a cell. iOS and Android users meet in 95 % of the
  cells on paper, in **two cells out of three** in reality.

  A single gatekeeper.

### AirHop (A8)

AirHop takes the 10 nearest relays, the 5 BitChat picks and 5 more as backups, from the list
Android uses, frozen in each release of the app.

- **Gain:** more relays per cell make up for dead ones: it meets Android in 94 % of the cells and
  iOS in 78 %, better than iOS and Android meet each other.
- **Problem:** the list is frozen in the app: a phone that is not updated drifts as iOS has.
  Twice as many connections per cell and per phone also put an extra load on the Nostr relays.

### A0: one list, the same rule, more relays

Every client reads the same list with the same rule, publishes to 8 relays and reads 5, and
replaces a relay it finds dead with the next one.

- **Gain:** all A0 users meet in every cell, even with dead relays and lists a month apart. It
  stays compatible with today's Android app (98 %), which already reads the same list. It is the
  best short-term proposal.
- **Problem:** it meets today's iOS app only where iOS's old list agrees (68 %), until iOS reads
  a fresh list. As today, an adversary who rents a few servers near a city can take that city's
  cells over. And it is still one list (see Today).

### A1: relays drawn by a hash

Each cell gets relays drawn by a hash of the cell and of a shared secret value (a salt), among
all the listed relays, wherever they are.

- **Gain:** to take a city over, an adversary would need more than 60 servers instead of 5; the
  load spreads over 332 servers instead of 119; a list change moves fewer cells. All A1 users
  meet (99 %). These figures come from simulations on the real relay lists: the servers an
  adversary must place at a city's centre point to take its cells, and the number of different
  servers chosen over the 1,024 cells of precision 2
  ([details](https://github.com/wvmscs/geohash-relays/blob/strategies/docs/evaluation.md)).
- **Problem:** **it breaks compatibility**: a phone using A1 exchanges with today's apps in only
  2 to 4 % of the cells. Its relays may also be far away (possibly slower, but Tor circuits are
  already slow). It needs the salt to be shared by all clients, and a deployment synchronised in
  all implementations.

### Not selectable today

We are still studying these.

| Scenario | What it is | Why the map cannot show it |
|---|---|---|
| A2 | relays chosen from the reports of NIP-66 monitors | it needs trusted monitors that test writes through Tor. The existing monitors are made for Nostr in general: they check reading and NIP-11. BitChat needs anonymous writes through Tor, and nobody runs a monitor for that today. |
| A3 | relays announced by the users of a cell in their presences | the apps do not announce them yet |
| A4 | republishers: peers that copy events between the iOS and Android relay sets | not a choice of relays: it runs beside the apps, and its effect depends on the cells the republishers follow. `bitchat-console --republish` implements it; the map does not simulate it yet. |
| A5 | a federation of relays that synchronise with each other | it goes against the current philosophy of Nostr; no federation runs yet |
| A6 | qualification: set aside the relays that refuse what BitChat needs | not a strategy but a filter added to any strategy; the map already sets aside the relays that do not answer or that ask for authentication |

## Our conclusions

1. **Today's biggest problem is the age of the lists, not the rules.** iOS and Android lose a
   third of their encounters because the iOS list is three months old. Four clients now read
   four lists of different ages, two of them frozen in the app. **Gain without changing any
   code:** refresh the iOS list. A list qualified and published by a few trusted parties, signed
   and read by every client, would remove the drift and the dependence on a single source.
2. **For a migration, without changing all the apps at the same time:** republishers (A4) that
   connect all the clients, third-party ones included, for a short period. This needs
   volunteers, and perhaps trusted relays that implement it.
3. **For a new or updated client: A0 with qualification (A6).** It brings redundancy and the
   replacement of dead relays, and keeps exchanging with today's bitchat Android app, and with
   today's bitchat iOS app as soon as iOS reads a fresh list.
4. **A1: an option for later**, if capture becomes a real threat, and only with a deployment
   plan that does not break the existing clients during the transition. It resists capture and
   spreads the load, but cuts the link with today's apps.
5. **A2 to A6 do not all need the apps to change.** A3 is upward compatible: older apps ignore
   what it adds. A4 and A5 need no change to the apps. A6 is a filter. A2, A4 and A5 depend on
   the goodwill of the people who run the Nostr network: monitors, republishers, relay
   operators. A4 is the one that each user, or volunteers with the means, can run to reinforce
   the network.

## Learn more

The technical reference is
[geohash-relays-requirements-and-alternatives.md](https://github.com/wvmscs/geohash-relays/blob/strategies/docs/geohash-relays-requirements-and-alternatives.md).
Three sections are enough for this page:

- [§4](https://github.com/wvmscs/geohash-relays/blob/strategies/docs/geohash-relays-requirements-and-alternatives.md#4-is-the-selection-of-5-relays-necessary):
  why the choice by distance lets an adversary take a cell;
- [§5](https://github.com/wvmscs/geohash-relays/blob/strategies/docs/geohash-relays-requirements-and-alternatives.md#5-alternative-approaches):
  the alternatives A0 to A6, and the criteria to compare them;
- [§6](https://github.com/wvmscs/geohash-relays/blob/strategies/docs/geohash-relays-requirements-and-alternatives.md#6-recommendation):
  the steps that we propose.

The figures come from the bench of the
[geohash-relays](https://github.com/wvmscs/geohash-relays/tree/strategies) repository, on the
relay lists that we saved on these dates, and on the census of 2026-10-30. The census is one
reading through Tor, from one place, on one day: a relay that was slow that day counts as not
working. It has to be repeated periodically to get a more accurate view. How to run it:
[relay-census](https://github.com/wvmscs/geohash-relays-net#census-of-relays-relay-census).
