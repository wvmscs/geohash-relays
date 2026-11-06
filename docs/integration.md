<!-- Target design. Code: module `relays` of this branch (`api`). The "Status" comments say what
     the code does today. -->

# Using geohash-relays in your Nostr client

> **Working document.**

For developers of a Nostr client that wants to talk in **BitChat's location channels**, or to
give its own users channels by place, and needs to know **which relays to use for a place**.

## The problem, in one paragraph

A location channel is a place on the earth, written as a **geohash** (`u09tv` is a few
kilometres of Paris). Its messages are Nostr events tagged with that geohash. There is no
central server: every participant must **guess the same few relays** for the place, publish
there and listen there. If two phones guess differently, they never see each other's messages.

**geohash-relays** holds **all** the logic of that guess: getting the relay lists, reading,
sorting, rejecting and qualifying relays, setting dead ones aside, checking signatures. One
implementation, so that clients stop drifting apart. Where a binding would be a burden
(constrained devices, mobile apps), it is ported to other languages, and shared test vectors
certify that every port behaves the same.

geohash-relays does no I/O itself: **your client gives it the means to talk**, an object that
implements the trait `RelayComm` (Tor, direct HTTPS or anything else).

**geohash-relays-net** provides ready-made `RelayComm` implementations, for clients that have
none: Tor inside the process (Arti), a Tor daemon (SOCKS5), direct.

## Words used here

| Word | Meaning |
|---|---|
| cell | a geohash: a place, from a country (2 characters) to a street (7) |
| relay list | a CSV file of relays with their position (`Relay URL,Latitude,Longitude`). Any list in this format works. Today the BitChat apps read two: a list reviewed by hand in the bitchat repository (iOS), and the list that [georelays](https://github.com/permissionlesstech/georelays), a crawler of the BitChat team, regenerates every day (Android). It is where the apps look today, not an authority |
| directory authority | a source of signed relay lists (a key and an address); defaults are given in the crate |
| strategy | a rule that turns (list, cell) into the relays to use. `AS` and `AirHop` are what the BitChat and AirHop apps do today; `A0`, `A1`… are the alternatives studied in geohash-relays |
| read / write | the relays to listen to / to publish to. The BitChat apps read and write the same 5, AirHop the same 10; `A0` reads 5 and writes 8, so that two users who each miss different dead relays still share one |
| Tor isolation | each cell gets its own Tor circuit, so a relay cannot tell that the cells it sees are followed by the same user |

## Which crate do you need?

| You want | Take | Licence |
|---|---|---|
| the relays of a cell; you have your own way to talk to relays and to HTTPS sites | `geohash-relays` alone: [path 1](#path-1-geohash-relays-alone) | Unlicense or 0BSD: anything goes |
| the same, with Tor or direct connections ready-made | `geohash-relays` + `geohash-relays-net`: [path 2](#path-2-with-geohash-relays-net) | GPL-3.0-or-later for your client |

The two paths differ only in where the `RelayComm` object comes from: you write it (path 1), or
geohash-relays-net gives it (path 2).

## Path 1: geohash-relays alone

| You give | You get |
|---|---|
| the strategies you build in (Cargo features) | their code only |
| an object implementing `RelayComm`: your means to talk | the lists fetched, kept and refreshed; relays qualified; dead relays replaced; signatures checked |
| a cell (`"u09tv"`) | the relays of that cell, each marked read, write or both |
| optional: a home region (constrained devices) | only the relays that region needs are stored |

### Strategies are features

- Each strategy is a Cargo feature: a release carries only the code it uses.
- In production, one strategy.
- During a migration, two: the current one and the target one.
- For demonstrations and evaluations, several or all.

### Async or blocking

| Feature | For | geohash-relays works |
|---|---|---|
| `async` | phones, desktops, servers | in the background (`run`), on your runtime: it imposes none |
| `blocking` | microcontrollers | when you call `refresh` (at start-up, then now and then); `get` only fetches when no list is kept |

The trait and the API are the same in both (`geohash_relays::relays` and
`geohash_relays::relays::blocking`), without `async`/`.await` and without `sleep` in
`blocking`.

### Step 1: implement `RelayComm`

Your object only carries bytes. geohash-relays decides everything else: what to fetch, when,
what to keep, and whether a relay that did not answer is dead.

| Method | geohash-relays calls it to |
|---|---|
| `connect_relay` | open a link to a relay, for its own needs: reachability today; probes (`A6`), monitors (`A2`), republishing (`A4`), comparing (`A5`) later |
| `fetch` | an HTTPS GET: a signed list from an authority, or a relay's NIP-11 document (a GET on the relay's address with `Accept: application/nostr+json`) |
| `file` | a persistent file it reads and rewrites itself: the kept lists, the relays set aside |
| `now`, `sleep` | know the time and wait |
| `log` | optional: send its log lines somewhere (default: nowhere) |

```rust
use geohash_relays::relays::{Cell, CommError, Link, RelayComm, RelayUrl, StoredFile};

struct MyComm { /* your Tor client, your HTTP client, a directory */ }

impl RelayComm for MyComm {
    type Link = MyLink;
    type File = MyFile;

    /// A WebSocket to `relay` (`relay.url()`: `wss://…`), isolated per `cell`
    /// (one Tor circuit per cell).
    async fn connect_relay(&self, relay: &RelayUrl, cell: &Cell) -> Result<MyLink, CommError> { … }

    /// An HTTPS GET of `url`, with this Accept header; the body.
    async fn fetch(&self, url: &str, accept: &str) -> Result<Vec<u8>, CommError> { … }

    /// The persistent file called `name` (empty if absent).
    fn file(&self, name: &str) -> Result<MyFile, CommError> { … }

    /// Unix time, in seconds.
    fn now(&self) -> i64 { … }
    async fn sleep(&self, seconds: u64) { … }

    /// Optional.
    fn log(&self, line: &str) { … }
}

/// A link to a relay: text frames in and out. Dropping it closes it.
impl Link for MyLink {
    async fn send(&mut self, frame: &str) -> Result<(), CommError> { … }
    /// The next frame; `None` when the relay closed the link.
    async fn recv(&mut self) -> Result<Option<String>, CommError> { … }
}

/// A persistent file.
impl StoredFile for MyFile {
    fn read(&mut self) -> Result<Vec<u8>, CommError> { … }
    /// Replace the whole content, atomically.
    fn write(&mut self, bytes: &[u8]) -> Result<(), CommError> { … }
}
```

### Step 2: give it to geohash-relays, ask for a cell

```rust
use geohash_relays::relays::{Cell, CellRelays, Extra, Relays, Strategy};

let comm: MyComm = MyComm::new(/* … */);
let relays: Relays<MyComm> = Relays::new(comm, Strategy::A0)
    .migrating_to(Strategy::A1(salts))               // optional
    .home(Cell::parse("u09t")?, 10)                  // optional: constrained devices
    .extra_relays(my_relays, Extra::Append);         // optional: private or special relays

// Its own work (refreshing lists, qualifying relays): spawn it on your runtime.
spawn(relays.clone().run());

let cell: Cell = Cell::parse("u09tv")?;
let chosen: CellRelays = relays.get(&cell).await?;
for relay in chosen.read() { /* listen there, with your own Nostr code */ }
for relay in chosen.write() { /* publish there */ }
```

`chosen` is one list of relays; each relay says whether to read it, write it, or both.
`read()` and `write()` are two views of that list.

### The API

```rust
impl<C: RelayComm> Relays<C> {
    // Building. Each `Strategy` value exists only if its feature is on.
    pub fn new(comm: C, strategy: Strategy) -> Relays<C>;
    /// Compile error unless the features of both strategies are on.
    pub fn migrating_to(self, target: Strategy) -> Relays<C>;
    /// Demonstrations: more strategies, asked through `get_with`.
    pub fn also(self, others: &[Strategy]) -> Relays<C>;
    /// Store only the relays the cells of `region` need, at least `min_relays`.
    pub fn home(self, region: Cell, min_relays: usize) -> Relays<C>;
    /// Replace the default directory authorities.
    pub fn authorities(self, authorities: Vec<Authority>) -> Relays<C>;
    /// Private or special relays: beside the strategy's (`Append`) or instead (`Replace`).
    pub fn extra_relays(self, relays: Vec<RelayUrl>, mode: Extra) -> Relays<C>;
    /// Qualify relays and set dead ones aside (default: on).
    pub fn qualify(self, on: bool) -> Relays<C>;

    // Running.
    /// `async`: refreshes the lists on their schedule and qualifies relays; never returns.
    pub async fn run(self);
    /// The relays of `cell` for the strategy in use, and for the migration target if any.
    pub async fn get(&self, cell: &Cell) -> Result<CellRelays, Error>;
    /// The same for one of the strategies given to `also`.
    pub async fn get_with(&self, strategy: Strategy, cell: &Cell) -> Result<CellRelays, Error>;
    /// What your own connections saw of a relay; `Dead` and refusals put it aside.
    pub fn report(&self, relay: &RelayUrl, seen: Seen);
    /// Download every list again and check every relay in use, now.
    /// (`blocking`: `refresh(force: bool)`, only the due work unless `force`.)
    pub async fn refresh(&self) -> Result<(), Error>;
}

pub enum Strategy {
    Apps(Apps),          // feature `apps`: Apps::Both, Apps::Ios, Apps::Android
    AirHop,              // feature `airhop`
    A0,                  // feature `a0`
    A1(Salts),           // feature `a1`: Salts::Fixed(bytes), Salts::ByEpoch(map)…
}
impl Strategy { pub fn name(&self) -> &'static str; }

pub struct CellRelays { pub cell: Cell, pub strategy: Strategy, pub relays: Vec<Relay> }
impl CellRelays {
    pub fn read(&self) -> impl Iterator<Item = &Relay>;
    pub fn write(&self) -> impl Iterator<Item = &Relay>;
}
pub struct Relay {
    pub url: RelayUrl,
    pub place: Option<(f64, f64)>,   // (lat, lon); None for extra relays
    pub rank: usize,                  // 1-based
    pub role: Role,
}
pub enum Role { ReadWrite, ReadOnly, WriteOnly }
impl Role { pub fn reads(self) -> bool; pub fn writes(self) -> bool; }
pub enum Seen { Dead, AuthRefused, EventRefused, Fine }
pub enum Extra { Append, Replace }
/// A source of lists. Signed lists: open.
pub struct Authority { pub url: String, pub platform: Platform }
impl Authority { pub fn defaults() -> Vec<Authority>; }   // the iOS list and georelays' list
/// A relay address without its scheme (`wss://` is implied).
pub struct RelayUrl(/* … */);
impl RelayUrl {
    pub fn parse(raw: &str) -> Result<RelayUrl, Error>;
    pub fn as_str(&self) -> &str;       // without `wss://`
    pub fn url(&self) -> String;        // `wss://…`, also `Display`
    pub fn nip11_url(&self) -> String;  // `https://…`
}
/// A valid geohash.
pub struct Cell(/* … */);
impl Cell { pub fn parse(geohash: &str) -> Result<Cell, Error>; pub fn as_str(&self) -> &str; }
pub struct CommError(pub String);
pub enum Error { InvalidGeohash, InvalidRelayUrl, NoRelayList, StrategyNotConfigured, Unreadable, … }
```

### What it does for you

- Fetches the lists from the authorities, checks their signatures, keeps them and refreshes
  them on its own schedule.
  <!-- Status: done: download, keeping, refresh after 24 h, retry after 1 h. To do: list signatures. -->
- Qualifies relays (`A6`: NIP-11 documents, probes) and sets aside those that are dead, refuse
  authentication or refuse events; the next relay takes their place (`A0`).
  <!-- Status: done: NIP-11 and reachability, every 6 h, relays in use only. To do: A6 probes (they need to sign a test event). -->
- Runs the strategies that need to talk to relays themselves: monitors (`A2`), republishing
  (`A4`), comparing a federation (`A5`), checking relays (`A6`).
  <!-- Status: to do. -->
- Checks the signature of every event it reads.
  <!-- Status: open: signatures, to be seen with B2. -->

### Constrained devices

<!-- Status: open: regional storage. -->

Today's app lists hold about 400 relays (15 KB). A list from a census is larger: of 3,371
addresses known on 2026-10-30, 940 answered and qualified, about 35 KB. Where that is too much,
`.home(region, 10)` keeps only the relays the cells of that region need, at least 10. Addresses
are stored without their scheme: `wss://` for relays and `https://` for NIP-11 are implied.

The subset must give every cell of the region the same relays as the whole list. This holds for
strategies by distance (`AS`, `AirHop`, `A0`, `A5`); `A1` draws relays by hash over the whole
list, so it needs the whole list.
<!-- Status: open: A1 with a home region, to be seen with B2.
     Done: the region keeps the relays within d_k + 2r of its centre (k = max(min, 18)); tested on
     "u0": same choice as the whole list for all its 4-character cells (AS, AirHop, A0), files more
     than 3 times smaller; A1 keeps the whole list. -->

### Which strategy

| Your goal | Strategy | In short |
|---|---|---|
| Be seen by BitChat users, as the apps are today | `Strategy::As(Apps::Both)` | the union of what iOS and Android pick (5 + 5) |
| Also reach AirHop users | `Strategy::AirHop` | the 10 relays AirHop picks |
| Your own users among themselves, simply | `Strategy::A0` | one list, the 5 nearest relays to read, 8 to write, dead relays replaced |
| Your own users, hard for an adversary to take over | `Strategy::A1(salts)` | relays drawn by a hash instead of by distance; your users must share the salt |

A private relay of yours is not a strategy's business: give it with `extra_relays`.

## Path 2: with geohash-relays-net

geohash-relays-net gives `RelayComm` objects; pass one to `Relays::new` instead of yours.
<!-- Status: to do: geohash-relays-net does not implement RelayComm yet. -->

| Type | Talks through | Feature |
|---|---|---|
| `Arti` | Tor inside your process | `arti` |
| `Socks5` | a Tor daemon | — |
| `Direct` | no Tor | — |
| `Loopback` | a local relay, for tests | — |

Each gives one Tor circuit per cell (except `Direct` and `Loopback`) and keeps its files in a
directory you name.

geohash-relays-net also gives direct access to the API of these connections: Nostr
subscriptions and publications, with NIP-42 authentication, reconnection and the wait for each
`OK`, for clients that have no Nostr connections of their own.

TLS: geohash-relays-net uses rustls. Choose its cryptography (`ring` or `aws-lc-rs` feature)
to match your other dependencies, so that only one is built in.

## Rules

- Never log message contents, nicknames or keys.
- In tests, avoid publishing on public relays: use a local relay (`Loopback`).
- Something missing or wrong? Open an issue or a pull request on
  [geohash-relays](https://github.com/wvmscs/geohash-relays) (strategies, constants) or
  [geohash-relays-net](https://github.com/wvmscs/geohash-relays-net) (connections).
