// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The entry point (`relays`) gives what the strategies give, on the frozen
//! lists, through a host that only carries bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use geohash_relays::a0::SharedStrategy;
use geohash_relays::a1::{HrwStrategy, Salts};
use geohash_relays::geohash::{bbox, encode};
use geohash_relays::relays::{
    self, Authority, Cell, CellRelays, CommError, Extra, RelayUrl, Role, Seen, StoredFile,
    Strategy, blocking,
};
use geohash_relays::{
    AirhopStrategy, AppStrategy, Apps, Health, Platform, RelayDirectory, RelayHealth, Selected,
    Strategy as _,
};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const ANDROID: &str = include_str!("data/nostr_relays.csv");
const IOS_URL: &str = "https://lists.example/ios.csv";
const ANDROID_URL: &str = "https://lists.example/android.csv";
const NOW: i64 = 1_793_000_000;
const CELLS: &[&str] = &["u09tv", "u0", "9q8yy", "dr5r", "s0000", "r3gx2", "zzzz"];

/// The host of the tests: lists and NIP-11 documents by URL, files in
/// memory, relays that cannot be reached.
#[derive(Clone, Default)]
struct Host {
    pages: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    dead: Arc<Mutex<BTreeSet<String>>>,
    fetched: Arc<Mutex<Vec<String>>>,
    now: Arc<Mutex<i64>>,
}

impl Host {
    fn new() -> Host {
        let h = Host::default();
        h.page(IOS_URL, IOS.as_bytes());
        h.page(ANDROID_URL, ANDROID.as_bytes());
        *h.now.lock().unwrap() = NOW;
        h
    }

    fn page(&self, url: &str, body: &[u8]) {
        self.pages.lock().unwrap().insert(url.into(), body.to_vec());
    }

    fn fetched(&self) -> Vec<String> {
        self.fetched.lock().unwrap().clone()
    }

    fn get(&self, url: &str) -> Result<Vec<u8>, CommError> {
        self.fetched.lock().unwrap().push(url.into());
        self.pages
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| CommError("404".into()))
    }

    fn connect(&self, relay: &RelayUrl) -> Result<Nothing, CommError> {
        if self.dead.lock().unwrap().contains(&relay.url()) {
            Err(CommError("refused".into()))
        } else {
            Ok(Nothing)
        }
    }
}

struct Nothing;

struct MemFile {
    name: String,
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl StoredFile for MemFile {
    fn read(&mut self) -> Result<Vec<u8>, CommError> {
        Ok(self
            .files
            .lock()
            .unwrap()
            .get(&self.name)
            .cloned()
            .unwrap_or_default())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), CommError> {
        self.files
            .lock()
            .unwrap()
            .insert(self.name.clone(), bytes.to_vec());
        Ok(())
    }
}

impl relays::Link for Nothing {
    async fn send(&mut self, _frame: &str) -> Result<(), CommError> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>, CommError> {
        Ok(None)
    }
}

impl blocking::Link for Nothing {
    fn send(&mut self, _frame: &str) -> Result<(), CommError> {
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<String>, CommError> {
        Ok(None)
    }
}

impl relays::RelayComm for Host {
    type Link = Nothing;
    type File = MemFile;

    async fn connect_relay(&self, relay: &RelayUrl, _cell: &Cell) -> Result<Nothing, CommError> {
        self.connect(relay)
    }

    async fn fetch(&self, url: &str, _accept: &str) -> Result<Vec<u8>, CommError> {
        self.get(url)
    }

    fn file(&self, name: &str) -> Result<MemFile, CommError> {
        Ok(MemFile {
            name: name.into(),
            files: self.files.clone(),
        })
    }

    fn now(&self) -> i64 {
        *self.now.lock().unwrap()
    }

    async fn sleep(&self, _seconds: u64) {}
}

impl blocking::RelayComm for Host {
    type Link = Nothing;
    type File = MemFile;

    fn connect_relay(&self, relay: &RelayUrl, _cell: &Cell) -> Result<Nothing, CommError> {
        self.connect(relay)
    }

    fn fetch(&self, url: &str, _accept: &str) -> Result<Vec<u8>, CommError> {
        self.get(url)
    }

    fn file(&self, name: &str) -> Result<MemFile, CommError> {
        Ok(MemFile {
            name: name.into(),
            files: self.files.clone(),
        })
    }

    fn now(&self) -> i64 {
        *self.now.lock().unwrap()
    }
}

/// The test host's futures are always ready.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

fn authorities() -> Vec<Authority> {
    vec![
        Authority {
            url: IOS_URL.into(),
            platform: Platform::Ios,
        },
        Authority {
            url: ANDROID_URL.into(),
            platform: Platform::Android,
        },
    ]
}

fn entry(host: &Host, strategy: Strategy) -> relays::Relays<Host> {
    relays::Relays::new(host.clone(), strategy)
        .authorities(authorities())
        .qualify(false)
}

fn cell(g: &str) -> Cell {
    Cell::parse(g).unwrap()
}

fn get(r: &relays::Relays<Host>, g: &str) -> CellRelays {
    block_on(r.get(&cell(g))).unwrap()
}

fn urls<'a>(it: impl Iterator<Item = &'a relays::Relay>) -> Vec<String> {
    it.map(|r| r.url.url()).collect()
}

fn selected(v: Vec<Selected>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v {
        if !out.contains(&s.relay.url) {
            out.push(s.relay.url);
        }
    }
    out
}

fn ios() -> RelayDirectory {
    RelayDirectory::parse(IOS, Platform::Ios)
}

fn android() -> RelayDirectory {
    RelayDirectory::parse(ANDROID, Platform::Android)
}

/// The entry point gives, for each cell, what the strategy itself gives.
fn same_as(strategy: Strategy, reference: &dyn geohash_relays::Strategy) {
    let host = Host::new();
    let r = entry(&host, strategy);
    for g in CELLS {
        let got = get(&r, g);
        assert_eq!(
            urls(got.read()),
            selected(reference.read(g).unwrap()),
            "read {g}"
        );
        assert_eq!(
            urls(got.write()),
            selected(reference.write(g).unwrap()),
            "write {g}"
        );
    }
}

#[test]
fn apps_as_the_strategy() {
    same_as(
        Strategy::Apps(Apps::Both),
        &AppStrategy::new(Some(ios()), Some(android()), Apps::Both),
    );
}

#[test]
fn airhop_as_the_strategy() {
    same_as(
        Strategy::AirHop,
        &AirhopStrategy::from_directory(&android()),
    );
}

#[test]
fn a0_as_the_strategy() {
    same_as(Strategy::A0, &SharedStrategy::from_directory(android()));
}

#[test]
fn a1_as_the_strategy() {
    let salts = Salts::Fixed(b"salt".to_vec());
    same_as(
        Strategy::A1(salts.clone()),
        &HrwStrategy::new(&android()).with_salts(salts, NOW),
    );
}

#[test]
fn a0_reads_5_writes_8_and_one_list_holds_both() {
    let host = Host::new();
    let got = get(&entry(&host, Strategy::A0), "u09tv");
    assert_eq!(got.read().count(), 5);
    assert_eq!(got.write().count(), 8);
    assert_eq!(got.relays.len(), 8);
    assert!(got.relays[..5].iter().all(|r| r.role == Role::ReadWrite));
    assert!(got.relays[5..].iter().all(|r| r.role == Role::WriteOnly));
    assert_eq!(
        got.relays.iter().map(|r| r.rank).collect::<Vec<_>>(),
        (1..=8).collect::<Vec<_>>()
    );
}

#[test]
fn a_dead_relay_reported_is_replaced_as_a0_does() {
    let host = Host::new();
    let r = entry(&host, Strategy::A0);
    let first = get(&r, "u09tv").relays[0].url.clone();
    r.report(&first, Seen::Dead);
    let health: RelayHealth = [(first.url(), Health::Dead)].into();
    let reference = SharedStrategy::from_directory(android()).with_health(health);
    let got = get(&r, "u09tv");
    assert_eq!(urls(got.read()), selected(reference.read("u09tv").unwrap()));
    assert!(!got.relays.iter().any(|x| x.url == first));
    // Ten minutes later it is tried again.
    *host.now.lock().unwrap() += 600;
    assert_eq!(get(&r, "u09tv").relays[0].url, first);
    // Fine forgets it at once.
    *host.now.lock().unwrap() -= 600;
    r.report(&first, Seen::Fine);
    assert_eq!(get(&r, "u09tv").relays[0].url, first);
}

#[test]
fn checks_set_aside_dead_and_refusing_relays() {
    let host = Host::new();
    let r = relays::Relays::new(host.clone(), Strategy::A0).authorities(authorities());
    let first = get(&r, "u09tv").relays.clone();
    let (dead, auth) = (first[0].url.clone(), first[1].url.clone());
    host.dead.lock().unwrap().insert(dead.url());
    host.page(
        &auth.nip11_url(),
        br#"{"limitation":{"auth_required":true}}"#,
    );
    block_on(r.refresh()).unwrap();
    let got = get(&r, "u09tv");
    assert!(!got.relays.iter().any(|x| x.url == dead || x.url == auth));
    assert_eq!(got.read().count(), 5);
    // The checks went through the host: NIP-11 over HTTPS.
    assert!(host.fetched().contains(&auth.nip11_url()));
    assert!(auth.nip11_url().starts_with("https://"));
}

#[test]
fn lists_are_kept_and_not_fetched_again_within_a_day() {
    let host = Host::new();
    let before = get(&entry(&host, Strategy::A0), "u09tv");
    assert_eq!(host.fetched().len(), 2);
    // Another start: the kept lists serve, nothing is fetched.
    let again = entry(&host, Strategy::A0);
    assert_eq!(get(&again, "u09tv"), before);
    assert_eq!(host.fetched().len(), 2);
    // The sources gone, a day later: the kept lists still serve.
    host.pages.lock().unwrap().clear();
    *host.now.lock().unwrap() += 86_400;
    let later = entry(&host, Strategy::A0);
    block_on(later.refresh()).unwrap();
    assert_eq!(get(&later, "u09tv").relays, before.relays);
}

#[test]
fn without_any_list_there_is_no_answer() {
    let host = Host::new();
    host.pages.lock().unwrap().clear();
    let r = entry(&host, Strategy::A0);
    assert!(block_on(r.get(&cell("u09tv"))).is_err());
}

#[test]
fn kept_lists_carry_no_scheme() {
    let host = Host::new();
    get(&entry(&host, Strategy::A0), "u09tv");
    let files = host.files.lock().unwrap();
    assert!(!files.is_empty());
    for (name, bytes) in files.iter() {
        assert!(!String::from_utf8_lossy(bytes).contains("wss://"), "{name}");
    }
}

/// Every cell of precision `p` inside `region`.
fn cells_of(region: &str, p: usize) -> Vec<String> {
    let [[s, w], [n, e]] = bbox(region).unwrap();
    let mut out = BTreeSet::new();
    let steps = 24;
    for i in 0..=steps {
        for j in 0..=steps {
            let lat = s + (n - s) * (i as f64 + 0.5) / (steps as f64 + 1.0);
            let lon = w + (e - w) * (j as f64 + 0.5) / (steps as f64 + 1.0);
            out.insert(encode(lat, lon, p).unwrap());
        }
    }
    out.into_iter().collect()
}

#[test]
fn a_home_region_keeps_fewer_relays_and_the_same_choice() {
    for strategy in [Strategy::A0, Strategy::AirHop, Strategy::Apps(Apps::Both)] {
        let (whole, small) = (Host::new(), Host::new());
        let all = entry(&whole, strategy.clone());
        let home = entry(&small, strategy.clone()).home(cell("u0"), 10);
        for g in cells_of("u0", 4) {
            assert_eq!(get(&home, &g), get(&all, &g), "{} {g}", strategy.name());
        }
        let size = |h: &Host| {
            h.files
                .lock()
                .unwrap()
                .values()
                .map(Vec::len)
                .sum::<usize>()
        };
        assert!(size(&small) * 3 < size(&whole), "{}", strategy.name());
    }
}

#[test]
fn a1_keeps_the_whole_list_even_with_a_home() {
    let salts = Salts::Fixed(b"salt".to_vec());
    let (whole, small) = (Host::new(), Host::new());
    let all = entry(&whole, Strategy::A1(salts.clone()));
    let home = entry(&small, Strategy::A1(salts)).home(cell("u0"), 10);
    assert_eq!(get(&home, "u09tv"), get(&all, "u09tv"));
    let size = |h: &Host| {
        h.files
            .lock()
            .unwrap()
            .values()
            .map(Vec::len)
            .sum::<usize>()
    };
    assert_eq!(size(&small), size(&whole));
}

#[test]
fn a_migration_reads_and_writes_both() {
    let host = Host::new();
    let salts = Salts::Fixed(b"salt".to_vec());
    let r = entry(&host, Strategy::A0).migrating_to(Strategy::A1(salts.clone()));
    let got = get(&r, "u09tv");
    let a0 = selected(
        SharedStrategy::from_directory(android())
            .write("u09tv")
            .unwrap(),
    );
    let a1 = selected(
        HrwStrategy::new(&android())
            .with_salts(salts, NOW)
            .read("u09tv")
            .unwrap(),
    );
    let written = urls(got.write());
    assert!(a0.iter().chain(&a1).all(|u| written.contains(u)));
    assert!(a1.iter().all(|u| urls(got.read()).contains(u)));
}

#[test]
fn other_strategies_only_when_given() {
    let host = Host::new();
    let r = entry(&host, Strategy::A0).also(&[Strategy::AirHop]);
    let airhop = block_on(r.get_with(&Strategy::AirHop, &cell("u09tv"))).unwrap();
    assert_eq!(airhop.relays.len(), 10);
    assert_eq!(airhop.strategy, Strategy::AirHop);
    let a1 = Strategy::A1(Salts::None);
    assert!(block_on(r.get_with(&a1, &cell("u09tv"))).is_err());
}

#[test]
fn extra_relays_beside_or_instead() {
    let host = Host::new();
    let mine = RelayUrl::parse("relay.mine.example").unwrap();
    let beside = entry(&host, Strategy::A0).extra_relays(vec![mine.clone()], Extra::Append);
    let got = get(&beside, "u09tv");
    assert_eq!(got.relays.len(), 9);
    assert_eq!(got.relays[8].url, mine);
    assert_eq!(got.relays[8].role, Role::ReadWrite);
    assert_eq!(got.relays[8].place, None);
    let instead = entry(&host, Strategy::A0).extra_relays(vec![mine.clone()], Extra::Replace);
    assert_eq!(get(&instead, "u09tv").relays.len(), 1);
}

#[test]
fn blocking_gives_the_same() {
    let (a, b) = (Host::new(), Host::new());
    let nb = entry(&a, Strategy::A0);
    let bl = blocking::Relays::new(b.clone(), Strategy::A0)
        .authorities(authorities())
        .qualify(false);
    bl.refresh(false).unwrap();
    for g in CELLS {
        assert_eq!(bl.get(&cell(g)).unwrap(), get(&nb, g));
    }
}
