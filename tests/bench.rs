// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The bench on the frozen lists: the `AS` reference figures (lot 1).

use geohash_relays::bench::{self, Rng};
use geohash_relays::params::BENCH_SEED;
use geohash_relays::{AppStrategy, Platform, RelayDirectory, Strategy};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const IOS_OLD: &str = include_str!("data/online_relays_gps-2026-06-21.csv");
const ANDROID: &str = include_str!("data/nostr_relays.csv");

fn ios() -> AppStrategy {
    AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios))
}

fn android() -> AppStrategy {
    AppStrategy::android(RelayDirectory::parse(ANDROID, Platform::Android))
}

#[test]
fn the_apps_miss_each_other_in_some_cells() {
    let cells = bench::all_cells(2);
    let r = bench::rendezvous(&ios(), &android(), &cells).unwrap();
    assert_eq!(r.cells, 1024);
    // H1 is false: iOS and Android peers miss each other in some cells.
    assert!(r.met < r.cells && r.ratio() > 0.85, "{r:?}");
    // A strategy always meets itself.
    assert_eq!(bench::rendezvous(&ios(), &ios(), &cells).unwrap().met, 1024);
}

#[test]
fn android_puts_322_cells_on_4_servers() {
    // §4 of the requirements: "With the Android rule at precision 2, 322
    // cells of 1,024 have only 4 different servers".
    let l = bench::load(&android(), &bench::all_cells(2), 5).unwrap();
    assert_eq!(l.below_threshold, 322);
    assert!(l.max_cells > 300, "{l:?}");
}

#[test]
fn stability_and_failures() {
    let cells = bench::sample_cells(5, 200, &mut Rng::new(BENCH_SEED));
    let old = AppStrategy::ios(RelayDirectory::parse(IOS_OLD, Platform::Ios));
    let s = bench::stability(&old, &ios(), &cells).unwrap();
    assert!(s > 0.0 && s < 1.0, "{s}");
    assert_eq!(bench::stability(&ios(), &ios(), &cells).unwrap(), 1.0);
    let make_i = |_: &geohash_relays::RelayHealth| Box::new(ios()) as Box<dyn Strategy>;
    let make_a = |_: &geohash_relays::RelayHealth| Box::new(android()) as Box<dyn Strategy>;
    let none = bench::failures(&make_i, &make_a, &cells, 0, &mut Rng::new(1)).unwrap();
    let three = bench::failures(&make_i, &make_a, &cells, 3, &mut Rng::new(1)).unwrap();
    assert!(three.met < none.met, "{none:?} {three:?}");
}

#[test]
fn five_servers_capture_a_cell_on_ios() {
    // iOS breaks distance ties by host name: five well-named servers at the
    // city point take the five places. Android keeps list order on ties:
    // appended servers at the same point never win.
    let ios_dir = RelayDirectory::parse(IOS, Platform::Ios);
    let point = bench::nearest_point(&ios_dir, "u09tv").unwrap().unwrap();
    let name = |i: usize| format!("0{i:04}.attacker.example");
    let make = |d: &RelayDirectory| Box::new(AppStrategy::ios(d.clone())) as Box<dyn Strategy>;
    let cost = bench::capture_cost(&make, IOS, Platform::Ios, "u09tv", point, &name, 20).unwrap();
    assert_eq!(cost, Some(5));
    let android_dir = RelayDirectory::parse(ANDROID, Platform::Android);
    let point = bench::nearest_point(&android_dir, "u09tv")
        .unwrap()
        .unwrap();
    let make = |d: &RelayDirectory| Box::new(AppStrategy::android(d.clone())) as Box<dyn Strategy>;
    let cost =
        bench::capture_cost(&make, ANDROID, Platform::Android, "u09tv", point, &name, 20).unwrap();
    assert_eq!(cost, None);
}
