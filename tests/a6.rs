// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A6: verdicts from NIP-11 documents and probes, the filter, the prober.

use std::collections::BTreeMap;

use geohash_relays::a0::SharedStrategy;
use geohash_relays::a6::{Finding, Nip11, Probe, Prober, Qualification, Verdict};
use geohash_relays::params::A6_PROBE_CELL;
use geohash_relays::sansio::{Action, EventMeta, Input, Machine};
use geohash_relays::{Error, Health, Platform, RelayDirectory, Strategy};

const AUTH: &str = include_str!("data/nip11/auth.json");
const OPEN: &str = include_str!("data/nip11/open.json");
const STRICT: &str = include_str!("data/nip11/strict.json");
const IOS: &str = include_str!("data/online_relays_gps.csv");

fn reqs(v: &[Finding]) -> Vec<&'static str> {
    v.iter().map(Finding::requirement).collect()
}

#[test]
fn declarations() {
    let open = Verdict::of(Nip11::parse(OPEN).as_ref(), None);
    assert!(open.usable() && open.warnings.is_empty(), "{open:?}");
    let auth = Verdict::of(Nip11::parse(AUTH).as_ref(), None);
    assert_eq!(reqs(&auth.refusals), ["R5"]);
    assert_eq!(reqs(&auth.warnings), ["R8", "R13"]);
    let strict = Verdict::of(Nip11::parse(STRICT).as_ref(), None);
    assert_eq!(reqs(&strict.refusals), ["R6", "R6", "R7"]);
    assert_eq!(strict.warnings, [Finding::Retention(600)]);
    let n = Nip11::parse(STRICT).unwrap();
    assert_eq!(n.retention_of(20000), Some(Some(600)));
    assert_eq!(n.retention_of(1), Some(Some(86400)));
    assert_eq!(Nip11::parse(OPEN).unwrap().retention_of(1), Some(None));
    assert!(Nip11::parse("not json").is_none());
}

#[test]
fn probes_prevail() {
    let works = Probe {
        accepted: Some(true),
        read_back: vec![(5, true), (3600, false)],
        ..Probe::default()
    };
    // Declares NIP-42, but the test wrote and read back: usable, and the
    // event was gone after an hour.
    let v = Verdict::of(Nip11::parse(AUTH).as_ref(), Some(&works));
    assert!(v.usable(), "{v:?}");
    assert!(v.warnings.contains(&Finding::Retention(3600)));
    let refused = Probe {
        accepted: Some(false),
        ..Probe::default()
    };
    assert_eq!(
        reqs(&Verdict::of(Nip11::parse(OPEN).as_ref(), Some(&refused)).refusals),
        ["R9"]
    );
    let redirected = Probe {
        redirected: true,
        ..Probe::default()
    };
    assert_eq!(reqs(&Verdict::of(None, Some(&redirected)).refusals), ["R2"]);
}

#[test]
fn a_local_filter_needs_write_wide() {
    let dir = RelayDirectory::parse(IOS, Platform::Ios);
    let first = SharedStrategy::new(IOS).read("u09tv").unwrap()[0]
        .relay
        .url
        .clone();
    let mut q = Qualification::default();
    q.verdicts.insert(
        first.clone(),
        Verdict::of(Nip11::parse(AUTH).as_ref(), None),
    );
    assert!(matches!(
        q.filter(dir.clone()),
        Err(Error::LocalQualification)
    ));
    // A0 takes it.
    let a0 = SharedStrategy::new(IOS).qualified(&q);
    assert!(
        a0.read("u09tv")
            .unwrap()
            .iter()
            .all(|s| s.relay.url != first)
    );
    // A shared one filters any list.
    q.shared = true;
    let filtered = q.filter(dir.clone()).unwrap();
    assert_eq!(filtered.len(), dir.len() - 1);
    assert_eq!(q.refused(), [(first.as_str(), vec!["R5"])]);
}

fn test_event(id: &str) -> EventMeta {
    EventMeta {
        id: id.into(),
        pubkey: "ab".repeat(32),
        kind: 20000,
        created_at: 1000,
        tags: vec![vec!["g".into(), A6_PROBE_CELL.into()]],
        raw: format!("{{\"id\":\"{id}\"}}"),
    }
}

#[test]
fn the_prober_fetches_writes_and_reads_back() {
    let relays = vec!["wss://a.example".to_owned(), "wss://b.example".to_owned()];
    let tests = BTreeMap::from([("wss://a.example".to_owned(), test_event("t1"))]);
    let mut p = Prober::new(relays, tests, &[3600]);
    let start = p.start(1000);
    assert_eq!(
        start
            .iter()
            .filter(|a| matches!(a, Action::FetchNip11 { .. }))
            .count(),
        2
    );
    assert!(start.iter().any(|a| matches!(a, Action::Publish { relay, id, .. } if relay == "wss://a.example" && id == "t1")));
    // Subscribed before publishing.
    let sub_at = start
        .iter()
        .position(|a| matches!(a, Action::Subscribe { .. }))
        .unwrap();
    let pub_at = start
        .iter()
        .position(|a| matches!(a, Action::Publish { .. }))
        .unwrap();
    assert!(sub_at < pub_at);
    p.handle(Input::Nip11 {
        relay: "wss://a.example".into(),
        json: Some(OPEN.into()),
    });
    p.handle(Input::Nip11 {
        relay: "wss://b.example".into(),
        json: Some(AUTH.into()),
    });
    // Sent back live, before any read-back: R9 met.
    p.handle(Input::Event {
        relay: "wss://a.example".into(),
        event: test_event("t1"),
    });
    p.handle(Input::Ok {
        relay: "wss://a.example".into(),
        id: "t1".into(),
        accepted: true,
        message: String::new(),
    });
    assert!(p.handle(Input::Tick { now: 1002 }).is_empty());
    let sub = p.handle(Input::Tick { now: 1005 });
    assert!(
        matches!(&sub[..], [Action::Subscribe { relay, filter }] if relay == "wss://a.example" && filter.authors == ["ab".repeat(32)])
    );
    p.handle(Input::Event {
        relay: "wss://a.example".into(),
        event: test_event("t1"),
    });
    p.handle(Input::Eose {
        relay: "wss://a.example".into(),
    });
    assert!(!p.done());
    p.handle(Input::Tick { now: 4605 });
    p.handle(Input::Eose {
        relay: "wss://a.example".into(),
    });
    assert!(p.done());
    let q = p.qualification();
    assert!(!q.shared);
    let a = &q.verdicts["wss://a.example"];
    assert!(a.usable() && a.probed, "{a:?}");
    assert!(a.warnings.contains(&Finding::Retention(3600)));
    assert_eq!(reqs(&q.verdicts["wss://b.example"].refusals), ["R5"]);
    let _ = Health::Dead;
}

#[test]
fn at_most_sixteen_at_once() {
    let relays: Vec<String> = (0..40).map(|i| format!("wss://r{i}.example")).collect();
    let mut p = Prober::new(relays, BTreeMap::new(), &[]);
    let start = p.start(0);
    assert_eq!(start.len(), 16);
    let next = p.handle(Input::Nip11 {
        relay: "wss://r0.example".into(),
        json: None,
    });
    assert!(matches!(&next[..], [Action::FetchNip11 { relay }] if relay == "wss://r16.example"));
    for i in 1..40 {
        p.handle(Input::Nip11 {
            relay: format!("wss://r{i}.example"),
            json: None,
        });
    }
    assert!(p.done());
}
