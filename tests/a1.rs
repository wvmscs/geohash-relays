// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A1 on the frozen lists: frozen score vectors, spread, stability,
//! neighbours, epochs, capture with and without salt.

use std::collections::{BTreeMap, HashSet};

use geohash_relays::a1::{HrwStrategy, Salts, epoch_of, score};
use geohash_relays::bench::{self, Rng};
use geohash_relays::params::{A1_EPOCH_SECS, BENCH_SEED};
use geohash_relays::{AppStrategy, Platform, RelayDirectory, Role, Strategy};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const ANDROID: &str = include_str!("data/nostr_relays.csv");

/// Checked against an independent implementation (Python `hashlib`).
const VECTORS: &[(&str, &str, &str, u64)] = &[
    ("", "u09tv", "wss://relay.damus.io", 0xf00e9546de7c6407),
    ("00", "u09tv", "wss://relay.damus.io", 0xe4effd35b0a234ce),
    ("00", "u09t", "relay.damus.io:443", 0xe4effd35b0a234ce),
    ("6b2f", "9q8yy", "wss://nos.lol", 0x539b1c2d09271408),
    ("6b2f", "9q", "wss://nos.lol", 0xfddae940c1e1db50),
];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn ios() -> RelayDirectory {
    RelayDirectory::parse(IOS, Platform::Ios)
}

fn salted(dir: &RelayDirectory) -> HrwStrategy {
    HrwStrategy::new(dir).with_salts(Salts::Fixed(hex("6b2f")), 0)
}

fn urls(s: &dyn Strategy, cell: &str) -> HashSet<String> {
    s.read(cell)
        .unwrap()
        .into_iter()
        .map(|x| x.relay.url)
        .collect()
}

#[test]
fn frozen_vectors() {
    for (salt, cell, url, expected) in VECTORS {
        assert_eq!(
            score(&hex(salt), cell, 4, url),
            *expected,
            "{salt} {cell} {url}"
        );
    }
}

#[test]
fn spreads_the_cells() {
    let cells = bench::all_cells(2);
    let a1 = bench::load(&salted(&ios()), &cells, 5).unwrap();
    let as_ = bench::load(&AppStrategy::ios(ios()), &cells, 5).unwrap();
    assert_eq!(a1.below_threshold, 0);
    assert!(a1.servers > as_.servers, "{a1:?} {as_:?}");
    assert!(a1.max_cells * 3 < as_.max_cells, "{a1:?} {as_:?}");
}

#[test]
fn a_new_relay_only_moves_the_cells_that_choose_it() {
    let cells = bench::sample_cells(5, 300, &mut Rng::new(BENCH_SEED));
    let before = salted(&ios());
    let after = salted(&RelayDirectory::parse(
        &format!("{IOS}\nnew.relay.example,0,0\n"),
        Platform::Ios,
    ));
    for c in &cells {
        let (b, a) = (urls(&before, c), urls(&after, c));
        if !a.contains("wss://new.relay.example") {
            assert_eq!(a, b, "{c}");
        } else {
            assert_eq!(a.difference(&b).count(), 1, "{c}");
        }
    }
}

#[test]
fn neighbours_share_their_relays() {
    let s = salted(&ios());
    let cells = geohash_relays::geohash::with_neighbours("u09tv").unwrap();
    let same_prefix: Vec<&String> = cells.iter().filter(|c| c.starts_with("u09t")).collect();
    assert!(same_prefix.len() > 1);
    let first = urls(&s, same_prefix[0]);
    assert!(same_prefix.iter().all(|c| urls(&s, c) == first));
}

#[test]
fn two_lists_nearly_the_same_choice() {
    // HRW on the iOS list and on the Android list: the two apps meet far
    // more often than with distance.
    let cells = bench::sample_cells(5, 500, &mut Rng::new(BENCH_SEED));
    let android = RelayDirectory::parse(ANDROID, Platform::Android);
    let r = bench::rendezvous(&salted(&ios()), &salted(&android), &cells).unwrap();
    let today = bench::rendezvous(
        &AppStrategy::ios(ios()),
        &AppStrategy::android(android),
        &cells,
    )
    .unwrap();
    assert!(r.ratio() >= today.ratio(), "{r:?} {today:?}");
}

#[test]
fn writes_to_both_epochs_around_a_change() {
    let day = 20_000;
    let salts = Salts::ByEpoch(BTreeMap::from([(day, hex("01")), (day + 1, hex("02"))]));
    let late = day * A1_EPOCH_SECS + A1_EPOCH_SECS - 600;
    assert_eq!(epoch_of(late), day);
    let s = HrwStrategy::new(&ios()).with_salts(salts.clone(), late);
    let w = s.write("u09tv").unwrap();
    assert_eq!(s.read("u09tv").unwrap().len(), 5);
    assert!(w.len() > 5 && w.iter().skip(5).all(|x| x.role == Role::WriteOnly));
    let mid = day * A1_EPOCH_SECS + A1_EPOCH_SECS / 2;
    let s = HrwStrategy::new(&ios()).with_salts(salts.clone(), mid);
    assert_eq!(s.write("u09tv").unwrap().len(), 5);
    // An epoch without a salt is not served.
    let s = HrwStrategy::new(&ios()).with_salts(salts, (day + 5) * A1_EPOCH_SECS);
    assert!(s.read("u09tv").unwrap().is_empty());
}

#[test]
fn the_salt_defeats_ground_names() {
    // Without salt, an attacker grinds names offline that beat the best of
    // the current top 5 of a cell: five servers capture it. With a salt unknown at
    // grinding time, the same names are worth nothing.
    let cell = "u09tv";
    let dir = ios();
    let unsalted = HrwStrategy::new(&dir);
    let best = unsalted
        .read(cell)
        .unwrap()
        .iter()
        .map(|s| score(&[], cell, 4, &s.relay.url))
        .max()
        .unwrap();
    let ground: Vec<String> = (0..)
        .map(|i| format!("g{i}.attacker.example"))
        .filter(|n| score(&[], cell, 4, &format!("wss://{n}")) > best)
        .take(5)
        .collect();
    let point = (0.0, 0.0);
    let name = |i: usize| {
        ground
            .get(i - 1)
            .cloned()
            .unwrap_or_else(|| format!("x{i}.attacker.example"))
    };
    let make_plain = |d: &RelayDirectory| Box::new(HrwStrategy::new(d)) as Box<dyn Strategy>;
    let cost =
        bench::capture_cost(&make_plain, IOS, Platform::Ios, cell, point, &name, 30).unwrap();
    assert_eq!(cost, Some(5));
    let make_salted = |d: &RelayDirectory| Box::new(salted(d)) as Box<dyn Strategy>;
    let cost =
        bench::capture_cost(&make_salted, IOS, Platform::Ios, cell, point, &name, 30).unwrap();
    assert_eq!(cost, None);
}
