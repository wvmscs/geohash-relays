// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The comparison bench on the frozen lists (`PLAN_ET1.md`, lot 1 and each
//! strategy's lot):
//!
//!   cargo run --release --example compare
//!
//! Each strategy has two peers that should meet: for `AS`, an iOS peer and
//! an Android peer (two lists, two rules); for the strategies with one
//! shared list, two peers whose copies are a month apart (the list moves).

use geohash_relays::a0::SharedStrategy;
use geohash_relays::a1::{HrwStrategy, Salts};
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

type Peer = Box<dyn Fn(&RelayHealth) -> Box<dyn Strategy>>;
type FromList = Box<dyn Fn(&str, Platform) -> Option<Box<dyn Strategy>>>;

/// A strategy in the bench.
struct Contender {
    name: &'static str,
    /// The two peers that should meet, knowing these failed relays.
    a: Peer,
    b: Peer,
    /// A peer on a given list (stability, capture); `None`: the strategy
    /// does not read that kind of list.
    on_list: FromList,
}

fn contenders() -> Vec<Contender> {
    vec![
        Contender {
            name: "AS",
            a: Box::new(|_| Box::new(AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios)))),
            b: Box::new(|_| {
                Box::new(AppStrategy::android(RelayDirectory::parse(
                    ANDROID,
                    Platform::Android,
                )))
            }),
            on_list: Box::new(|csv, p| {
                Some(Box::new(AppStrategy::new(
                    Some(RelayDirectory::parse(csv, p)),
                    None,
                    geohash_relays::Apps::Both,
                )))
            }),
        },
        Contender {
            name: "A0",
            a: Box::new(|h| Box::new(SharedStrategy::new(IOS).with_health(h.clone()))),
            b: Box::new(|h| Box::new(SharedStrategy::new(IOS_OLD).with_health(h.clone()))),
            on_list: Box::new(|csv, p| {
                (p == Platform::Ios)
                    .then(|| Box::new(SharedStrategy::new(csv)) as Box<dyn Strategy>)
            }),
        },
        Contender {
            name: "A1 (iOS / Android lists)",
            a: Box::new(|_| Box::new(salted(&RelayDirectory::parse(IOS, Platform::Ios)))),
            b: Box::new(|_| Box::new(salted(&RelayDirectory::parse(ANDROID, Platform::Android)))),
            on_list: Box::new(|csv, p| {
                Some(Box::new(salted(&RelayDirectory::parse(csv, p))) as Box<dyn Strategy>)
            }),
        },
    ]
}

/// A1 with a salt the attacker did not know in advance.
fn salted(d: &RelayDirectory) -> HrwStrategy {
    HrwStrategy::new(d).with_salts(Salts::Fixed(vec![0x6b, 0x2f]), 0)
}

fn main() {
    let p2 = bench::all_cells(2);
    let p5 = bench::sample_cells(5, 500, &mut Rng::new(BENCH_SEED));
    let healthy = RelayHealth::new();

    println!(
        "| Strategy | Rendezvous p2 | p5 | Servers (p2) | Busiest (cells) | Cells < 5 servers | Stability iOS list | Stability Android list | 1 dead | 2 dead | 3 dead |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for c in contenders() {
        let (a, b) = ((c.a)(&healthy), (c.b)(&healthy));
        let r2 = bench::rendezvous(a.as_ref(), b.as_ref(), &p2).unwrap();
        let r5 = bench::rendezvous(a.as_ref(), b.as_ref(), &p5).unwrap();
        let l = bench::load(b.as_ref(), &p2, 5).unwrap();
        let stab =
            |old: &str, new: &str, p: Platform| match ((c.on_list)(old, p), (c.on_list)(new, p)) {
                (Some(o), Some(n)) => format!(
                    "{:.2}",
                    bench::stability(o.as_ref(), n.as_ref(), &p5).unwrap()
                ),
                _ => "—".into(),
            };
        let mut dead = Vec::new();
        for d in 1..=3 {
            let mut rng = Rng::new(BENCH_SEED + d as u64);
            let r = bench::failures(&|h| (c.a)(h), &|h| (c.b)(h), &p5, d, &mut rng).unwrap();
            dead.push(format!("{:.0} %", 100.0 * r.ratio()));
        }
        println!(
            "| {} | {:.1} % | {:.1} % | {} | {} ({}) | {} | {} | {} | {} |",
            c.name,
            100.0 * r2.ratio(),
            100.0 * r5.ratio(),
            l.servers,
            l.busiest.trim_start_matches("wss://"),
            l.max_cells,
            l.below_threshold,
            stab(IOS_OLD, IOS, Platform::Ios),
            stab(ANDROID_OLD, ANDROID, Platform::Android),
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
    for c in contenders() {
        let mut row = Vec::new();
        for (_, cell) in CITIES {
            let mut both = Vec::new();
            for (csv, platform) in [(IOS, Platform::Ios), (ANDROID, Platform::Android)] {
                if (c.on_list)(csv, platform).is_none() {
                    both.push("—".to_owned());
                    continue;
                }
                let dir = RelayDirectory::parse(csv, platform);
                let point = bench::nearest_point(&dir, cell).unwrap().unwrap();
                // Names that win iOS's alphabetical tie-break.
                let cost = bench::capture_cost(
                    &|d| {
                        // The strategy on the list with the attacker's lines.
                        let text: String = d
                            .relays()
                            .iter()
                            .map(|r| format!("{},{},{}\n", r.url, r.lat, r.lon))
                            .collect();
                        (c.on_list)(&text, platform).unwrap()
                    },
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
        println!("| {} | {} |", c.name, row.join(" | "));
    }
}
