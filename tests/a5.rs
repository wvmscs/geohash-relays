// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A5: the federation strategy, the sync filter, omissions, the comparer.

use std::collections::{BTreeMap, BTreeSet};

use geohash_relays::a5::{Comparer, Federation, omissions, sync_filter};
use geohash_relays::bench::{self, Rng};
use geohash_relays::sansio::{EventMeta, Input, Machine};
use geohash_relays::{Health, Strategy};

fn members() -> Vec<String> {
    [
        "wss://a.example",
        "wss://b.example/",
        "c.example",
        "wss://a.example",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[test]
fn reads_two_members_writes_one_any_two_peers_meet() {
    let f = Federation::new(&members());
    let r = f.read("u09tv").unwrap();
    let w = f.write("u09tv").unwrap();
    assert_eq!((r.len(), w.len()), (2, 1));
    assert_eq!(w[0].relay.url, r[0].relay.url);
    assert!(f.describe().starts_with("A5 (federation of 3"));
    // With synchronisation every member has everything: the rendezvous
    // holds when a peer reads any member; here, without counting on it,
    // peers on the same federation still pick the same members.
    let cells = bench::sample_cells(5, 100, &mut Rng::new(3));
    assert_eq!(bench::rendezvous(&f, &f, &cells).unwrap().met, 100);
}

#[test]
fn the_filter_of_a_region() {
    assert_eq!(sync_filter(&[]), r#"{"kinds":[20000,20001,1]}"#);
    assert_eq!(
        sync_filter(&["u09".into(), "u0d".into()]),
        r##"{"kinds":[20000,20001,1],"#g":["u09","u0d"]}"##
    );
}

#[test]
fn omissions_show_what_a_member_lacks() {
    let s = |ids: &[&str]| ids.iter().map(|i| i.to_string()).collect::<BTreeSet<_>>();
    let seen = BTreeMap::from([
        ("wss://a".to_owned(), s(&["e1", "e2", "e3"])),
        ("wss://b".to_owned(), s(&["e1", "e3"])),
    ]);
    let o = omissions(&seen);
    assert!(o["wss://a"].is_empty());
    assert_eq!(o["wss://b"], s(&["e2"]));
}

fn ev(id: &str) -> EventMeta {
    EventMeta {
        id: id.into(),
        pubkey: "p".into(),
        kind: 20000,
        created_at: 0,
        tags: vec![vec!["g".into(), "u09tv".into()]],
        raw: String::new(),
    }
}

#[test]
fn the_comparer_reads_every_member() {
    let mut c = Comparer::new(&members(), &["u09tv".into()]);
    assert_eq!(c.start(10_000).len(), 3);
    for (m, ids) in [
        ("wss://a.example", &["e1", "e2"][..]),
        ("wss://b.example", &["e1"][..]),
    ] {
        for id in ids {
            c.handle(Input::Event {
                relay: m.into(),
                event: ev(id),
            });
        }
        c.handle(Input::Eose { relay: m.into() });
    }
    assert!(!c.done());
    c.handle(Input::Failed {
        relay: "wss://c.example".into(),
        health: Health::Dead,
    });
    assert!(c.done());
    assert_eq!(
        c.omissions()["wss://b.example"].iter().collect::<Vec<_>>(),
        ["e2"]
    );
    assert!(c.failed().contains("wss://c.example"));
}
