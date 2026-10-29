// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The comparison bench on the frozen lists (lot 1 of `PLAN_ET1.md`):
//!
//!   cargo run --release --example compare
//!
//! An iOS peer (iOS list, its strategy) against an Android peer (Android
//! list, its strategy), for each strategy; then the load, the stability
//! between two versions of each list, the failures and the capture cost.

use geohash_relays::bench::{self, Rng};
use geohash_relays::params::BENCH_SEED;
use geohash_relays::{AppStrategy, Platform, RelayDirectory, RelayHealth, Strategy};

const IOS: &str = include_str!("../tests/data/online_relays_gps.csv");
const IOS_OLD: &str = include_str!("../tests/data/online_relays_gps-2026-06-21.csv");
const ANDROID: &str = include_str!("../tests/data/nostr_relays.csv");
const ANDROID_OLD: &str = include_str!("../tests/data/nostr_relays-2026-10-22.csv");

/// Cities of §7, question 4: Paris, New York, San Francisco, London.
const CITIES: &[(&str, &str)] = &[
    ("Paris", "u09tv"),
    ("New York", "dr5ru"),
    ("San Francisco", "9q8yy"),
    ("London", "gcpvj"),
];

type Make = Box<dyn Fn(&RelayDirectory, &RelayHealth) -> Box<dyn Strategy>>;

/// The strategies compared: name, and how a peer builds it from its list.
fn strategies() -> Vec<(&'static str, Make)> {
    vec![(
        "AS",
        Box::new(|d: &RelayDirectory, _: &RelayHealth| {
            Box::new(AppStrategy::new(
                Some(d.clone()),
                None,
                geohash_relays::Apps::Both,
            )) as Box<dyn Strategy>
        }),
    )]
}

fn main() {
    let ios = RelayDirectory::parse(IOS, Platform::Ios);
    let ios_old = RelayDirectory::parse(IOS_OLD, Platform::Ios);
    let android = RelayDirectory::parse(ANDROID, Platform::Android);
    let android_old = RelayDirectory::parse(ANDROID_OLD, Platform::Android);
    let p2 = bench::all_cells(2);
    let p5 = bench::sample_cells(5, 500, &mut Rng::new(BENCH_SEED));
    let healthy = RelayHealth::new();

    println!(
        "| Strategy | Rendezvous iOS/Android p2 | p5 | Servers (p2) | Busiest (cells) | Cells < 5 servers | Stability iOS | Stability Android | 1 dead | 2 dead | 3 dead |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for (name, make) in strategies() {
        let (si, sa) = (make(&ios, &healthy), make(&android, &healthy));
        let r2 = bench::rendezvous(si.as_ref(), sa.as_ref(), &p2).unwrap();
        let r5 = bench::rendezvous(si.as_ref(), sa.as_ref(), &p5).unwrap();
        let l = bench::load(sa.as_ref(), &p2, 5).unwrap();
        let st_i = bench::stability(make(&ios_old, &healthy).as_ref(), si.as_ref(), &p5).unwrap();
        let st_a =
            bench::stability(make(&android_old, &healthy).as_ref(), sa.as_ref(), &p5).unwrap();
        let mut dead = Vec::new();
        for d in 1..=3 {
            let mut rng = Rng::new(BENCH_SEED + d as u64);
            let r = bench::failures(&|h| make(&ios, h), &|h| make(&android, h), &p5, d, &mut rng)
                .unwrap();
            dead.push(format!("{:.0} %", 100.0 * r.ratio()));
        }
        println!(
            "| {name} | {:.1} % | {:.1} % | {} | {} ({}) | {} | {:.2} | {:.2} | {} |",
            100.0 * r2.ratio(),
            100.0 * r5.ratio(),
            l.servers,
            l.busiest.trim_start_matches("wss://"),
            l.max_cells,
            l.below_threshold,
            st_i,
            st_a,
            dead.join(" | "),
        );
    }

    println!();
    println!(
        "Capture cost (servers at the city point to take every read place), iOS list / Android list:"
    );
    println!();
    println!(
        "| Strategy | {} |",
        CITIES.iter().map(|c| c.0).collect::<Vec<_>>().join(" | ")
    );
    println!("|---|{}", "---|".repeat(CITIES.len()));
    for (name, make) in strategies() {
        let mut row = Vec::new();
        for (_, cell) in CITIES {
            let mut both = Vec::new();
            for (csv, dir, platform) in [
                (IOS, &ios, Platform::Ios),
                (ANDROID, &android, Platform::Android),
            ] {
                let point = bench::nearest_point(dir, cell).unwrap().unwrap();
                // Names that win iOS's alphabetical tie-break.
                let cost = bench::capture_cost(
                    &|d| make(d, &healthy),
                    csv,
                    platform,
                    cell,
                    point,
                    &|i| format!("0{i:04}.attacker.example"),
                    60,
                )
                .unwrap();
                both.push(cost.map_or("> 60".into(), |n| n.to_string()));
            }
            row.push(both.join(" / "));
        }
        println!("| {name} | {} |", row.join(" | "));
    }
}
