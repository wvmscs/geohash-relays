// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A3: announcements in presences, the threshold, Sybil, convergence.

use std::sync::Arc;

use geohash_relays::a3::{Announced, PresenceBook, announce_tags};
use geohash_relays::bench;
use geohash_relays::params::A3_ANNOUNCE_TTL_SECS;
use geohash_relays::sansio::EventMeta;
use geohash_relays::{AppStrategy, Origin, Platform, RelayDirectory, Strategy};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const ANDROID: &str = include_str!("data/nostr_relays.csv");

fn presence(key: &str, cell: &str, relays: &[&str], at: i64, pow: bool) -> EventMeta {
    let mut tags = vec![vec!["g".to_owned(), cell.to_owned()]];
    tags.extend(announce_tags(
        &relays.iter().map(|r| (*r).to_owned()).collect::<Vec<_>>(),
    ));
    EventMeta {
        id: format!("{}{key}{at}", if pow { "00" } else { "ff" }),
        pubkey: key.into(),
        kind: 20001,
        created_at: at,
        tags,
        raw: String::new(),
    }
}

#[test]
fn tags_are_bounded_and_normalised() {
    let r: Vec<String> = ["a.example", "wss://A.example/", "b", "c", "d", "e", "f"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let t = announce_tags(&r);
    assert_eq!(t.len(), 5);
    assert_eq!(t[0], ["relay", "wss://a.example"]);
    assert_eq!(t[1], ["relay", "wss://b"]);
}

#[test]
fn two_keys_with_proof_of_work_are_needed() {
    let mut book = PresenceBook::default();
    assert!(book.record("u09tv", &presence("k1", "u09tv", &["wss://x"], 100, true)));
    assert!(
        book.announced("u09tv", 100).is_empty(),
        "one key is not enough"
    );
    // Sybil: many keys without proof of work count for nothing.
    for i in 0..50 {
        assert!(!book.record(
            "u09tv",
            &presence(&format!("s{i}"), "u09tv", &["wss://evil"], 100, false)
        ));
    }
    assert!(book.record(
        "u09tv",
        &presence("k2", "u09tv", &["wss://x", "wss://y"], 101, true)
    ));
    assert_eq!(book.announced("u09tv", 110), [("wss://x".to_owned(), 2)]);
    // Another cell's presence is not this cell's.
    assert!(!book.record("u09tv", &presence("k3", "u09tw", &["wss://y"], 101, true)));
    // Too old: forgotten.
    assert!(
        book.announced("u09tv", 101 + A3_ANNOUNCE_TTL_SECS + 1)
            .is_empty()
    );
    book.prune(101 + A3_ANNOUNCE_TTL_SECS + 1);
    assert_eq!(book, PresenceBook::default());
}

#[test]
fn an_ios_peer_and_an_android_peer_converge() {
    // Cells where today's apps miss each other: each peer announces the
    // relays it writes to; with two keys on each side, both read the
    // other's relays.
    let ios =
        Arc::new(AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios))) as Arc<dyn Strategy>;
    let android = Arc::new(AppStrategy::android(RelayDirectory::parse(
        ANDROID,
        Platform::Android,
    ))) as Arc<dyn Strategy>;
    let missed: Vec<String> = bench::all_cells(2)
        .into_iter()
        .filter(|c| {
            bench::rendezvous(ios.as_ref(), android.as_ref(), std::slice::from_ref(c))
                .unwrap()
                .met
                == 0
        })
        .collect();
    assert!(!missed.is_empty());
    let mut book = PresenceBook::default();
    let now = 1000;
    for cell in &missed {
        for (side, s) in [("i", &ios), ("a", &android)] {
            let relays: Vec<String> = s
                .write(cell)
                .unwrap()
                .into_iter()
                .map(|x| x.relay.url)
                .collect();
            let refs: Vec<&str> = relays.iter().map(String::as_str).collect();
            for n in 0..2 {
                book.record(
                    cell,
                    &presence(&format!("{side}{n}{cell}"), cell, &refs, now, true),
                );
            }
        }
    }
    let (ai, aa) = (
        Announced::new(ios.clone(), book.clone(), now),
        Announced::new(android.clone(), book, now),
    );
    let r = bench::rendezvous(&ai, &aa, &missed).unwrap();
    assert_eq!(r.met, r.cells, "{r:?}");
    assert!(
        ai.read(&missed[0])
            .unwrap()
            .iter()
            .any(|s| s.origin == Origin::Announced)
    );
    assert!(
        ai.describe()
            .starts_with("A3 (announced by 2 keys, PoW 8) over AS")
    );
}
