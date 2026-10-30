// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A server named under several paths counts once for the alternatives
//! (lot 13): the same information many times brings nothing, and would give
//! one server several places of a cell. AS stays what the apps do.

use geohash_relays::a0::SharedStrategy;
use geohash_relays::a1::HrwStrategy;
use geohash_relays::a2::{Candidate, Directory};
use geohash_relays::a3::{Announced, PresenceBook, announce_tags};
use geohash_relays::a5::Federation;
use geohash_relays::sansio::EventMeta;
use geohash_relays::{
    AppStrategy, Platform, RelayDirectory, Strategy, normalize_url, one_per_server, server,
};
use std::sync::Arc;

/// Six addresses of one server, at the cell's centre, and three others far
/// away.
const CSV: &str = "Relay URL,Latitude,Longitude\n\
    busy.example.org/a,48.85,2.35\n\
    busy.example.org/b,48.85,2.35\n\
    busy.example.org/c,48.85,2.35\n\
    busy.example.org/d,48.85,2.35\n\
    busy.example.org/e,48.85,2.35\n\
    busy.example.org/f,48.85,2.35\n\
    one.example.org,10,10\n\
    two.example.org,20,20\n\
    three.example.org:7777/x,30,30\n";
const CELL: &str = "u09tv";

fn servers(s: &dyn Strategy) -> Vec<String> {
    s.read(CELL)
        .unwrap()
        .iter()
        .map(|r| {
            let url = normalize_url(&r.relay.url).unwrap();
            server(&url).to_owned()
        })
        .collect()
}

fn once_each(list: &[String]) -> bool {
    let mut sorted = list.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted.len() == list.len()
}

#[test]
fn the_server_of_an_address() {
    assert_eq!(server("wss://a.example.org/inbox"), "wss://a.example.org");
    assert_eq!(
        server("wss://a.example.org:7777/x/y"),
        "wss://a.example.org:7777"
    );
    assert_eq!(server("wss://a.example.org"), "wss://a.example.org");
    // The port makes another server.
    let kept = one_per_server(
        [
            "wss://a.example.org/z",
            "wss://a.example.org",
            "wss://a.example.org-b",
            "wss://a.example.org:7777/y",
            "wss://b.example.org/y",
            "wss://b.example.org/x",
        ]
        .map(String::from),
    );
    assert_eq!(
        kept,
        [
            "wss://a.example.org",
            "wss://a.example.org-b",
            "wss://a.example.org:7777/y",
            "wss://b.example.org/x"
        ]
    );
}

#[test]
fn a_list_keeps_one_address_per_server_whatever_its_order() {
    let list = RelayDirectory::parse(CSV, Platform::Ios).one_per_server();
    let urls: Vec<&str> = list.relays().iter().map(|r| r.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "wss://busy.example.org/a",
            "wss://one.example.org",
            "wss://two.example.org",
            "wss://three.example.org:7777/x"
        ]
    );
    let reversed: String = CSV.lines().rev().collect::<Vec<_>>().join("\n");
    let other = RelayDirectory::parse(&reversed, Platform::Android).one_per_server();
    assert!(
        other
            .relays()
            .iter()
            .any(|r| r.url == "wss://busy.example.org/a")
    );
    assert_eq!(other.len(), 4);
}

#[test]
fn as_stays_what_the_apps_do() {
    let ios = AppStrategy::ios(RelayDirectory::parse(CSV, Platform::Ios));
    assert!(!once_each(&servers(&ios)), "five places for one server");
}

#[test]
fn the_alternatives_count_a_server_once() {
    let a0 = SharedStrategy::new(CSV);
    let a0_servers = servers(&a0);
    assert_eq!(a0_servers.len(), 4, "{a0_servers:?}");
    assert!(once_each(&a0_servers), "{a0_servers:?}");
    let a1 = HrwStrategy::new(&RelayDirectory::parse(CSV, Platform::Ios));
    let a1_servers = servers(&a1);
    assert_eq!(a1_servers.len(), 4, "{a1_servers:?}");
    assert!(once_each(&a1_servers), "{a1_servers:?}");
    let a5 = Federation::new(
        &[
            "wss://busy.example.org/a",
            "wss://busy.example.org/b",
            "wss://one.example.org",
        ]
        .map(String::from),
    );
    assert!(once_each(&servers(&a5)));
    assert_eq!(servers(&a5).len(), 2);
}

#[test]
fn a_monitors_directory_lists_a_server_once() {
    let candidate = |relay: &str| Candidate {
        relay: relay.into(),
        monitors: 2,
        geohash: Some(CELL.into()),
    };
    let directory = Directory {
        candidates: vec![
            candidate("wss://a.example.org/y"),
            candidate("wss://a.example.org/x"),
            candidate("wss://b.example.org"),
        ],
        ..Directory::default()
    };
    let list = directory.to_list();
    let urls: Vec<&str> = list.relays().iter().map(|r| r.url.as_str()).collect();
    assert_eq!(urls, ["wss://a.example.org/x", "wss://b.example.org"]);
}

fn presence(key: &str, relays: &[&str], at: i64) -> EventMeta {
    let mut tags = vec![vec!["g".to_owned(), CELL.to_owned()]];
    for r in relays {
        tags.push(vec!["relay".to_owned(), (*r).to_owned()]);
    }
    EventMeta {
        id: format!("00{key}{at}"),
        pubkey: key.into(),
        kind: 20001,
        created_at: at,
        tags,
        raw: String::new(),
    }
}

#[test]
fn announcements_count_a_server_once() {
    // Announcing: one address per server.
    let tags = announce_tags(
        &[
            "wss://a.example.org/x",
            "wss://a.example.org",
            "wss://b.example.org",
        ]
        .map(String::from),
    );
    let named: Vec<&str> = tags.iter().map(|t| t[1].as_str()).collect();
    assert_eq!(named, ["wss://a.example.org", "wss://b.example.org"]);

    // One key naming two paths is one key, not the two needed.
    let mut book = PresenceBook::default();
    book.record(
        CELL,
        &presence(
            "k1",
            &["wss://a.example.org/x", "wss://a.example.org/y"],
            100,
        ),
    );
    assert!(book.announced(CELL, 100).is_empty());
    // Two keys on two paths of a server: announced once, the smallest address.
    book.record(CELL, &presence("k2", &["wss://a.example.org/w"], 100));
    assert_eq!(
        book.announced(CELL, 100),
        [("wss://a.example.org/w".to_owned(), 2)]
    );
    // Not added when the base already has the server, under any path.
    let base = Arc::new(HrwStrategy::new(&RelayDirectory::parse(
        "Relay URL,Latitude,Longitude\na.example.org/base,1,1\n",
        Platform::Ios,
    ))) as Arc<dyn Strategy>;
    let a3 = Announced::new(base, book, 100);
    assert_eq!(a3.read(CELL).unwrap().len(), 1);
}
