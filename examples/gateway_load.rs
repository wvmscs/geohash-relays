// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! How many relays a gateway needs (`docs/gateways.md`, section 5): the
//! cells of its own place, then distant cells.
//!
//!   cargo run --release --example gateway_load
//!
//! The lists are the frozen ones of `tests/data`; "alive": the relays the
//! census of 2026-10-30 found answering and not refused by A6.

use std::collections::HashSet;

use geohash_relays::bench::{Rng, sample_cells};
use geohash_relays::params::BENCH_SEED;
use geohash_relays::relays::gateway::place_cells;
use geohash_relays::{AppStrategy, Platform, RelayDirectory, Strategy, normalize_url};

const IOS: &str = include_str!("../tests/data/online_relays_gps.csv");
const ANDROID: &str = include_str!("../tests/data/nostr_relays-2026-10-29.csv");
const ALIVE: &str = include_str!("../tests/data/census-usable-2026-10-30.txt");

const PLACES: &[(&str, f64, f64)] = &[
    ("Paris", 48.8566, 2.3522),
    ("Correze (rural France)", 45.27, 1.77),
    ("New York", 40.71, -74.0),
    ("Chad", 12.1, 15.0),
];

/// The relays a strategy reads for these cells, normalised.
fn union<'a>(s: &dyn Strategy, cells: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    let mut out = HashSet::new();
    for c in cells {
        for x in s.read(c).unwrap() {
            out.insert(normalize_url(&x.relay.url).unwrap_or(x.relay.url));
        }
    }
    out
}

fn shown(set: &HashSet<String>, alive: &HashSet<String>) -> String {
    format!("{} ({})", set.len(), set.intersection(alive).count())
}

fn main() {
    let alive: HashSet<String> = ALIVE.lines().filter_map(normalize_url).collect();
    let ios = AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios));
    let android = AppStrategy::android(RelayDirectory::parse(ANDROID, Platform::Android));

    println!("Cells of the place (levels 2 to 8, neighbours from level 5): relays (alive)\n");
    println!(
        "| Gateway at | Cells | iOS list | Android list | Both lists |\n|---|---|---|---|---|"
    );
    for &(name, lat, lon) in PLACES {
        let cells = place_cells(lat, lon).unwrap();
        let names: Vec<&str> = cells.iter().map(|c| c.as_str()).collect();
        let i = union(&ios, names.iter().copied());
        let a = union(&android, names.iter().copied());
        let both: HashSet<String> = i.union(&a).cloned().collect();
        println!(
            "| {name} | {} | {} | {} | {} |",
            names.len(),
            shown(&i, &alive),
            shown(&a, &alive),
            shown(&both, &alive)
        );
    }

    println!("\nDistant cells (precision 2, 4, 5 or 6, drawn at random): relays\n");
    let marks = [1usize, 10, 50, 100, 500, 1_000];
    let mut rng = Rng::new(BENCH_SEED);
    let mut cells: Vec<String> = Vec::new();
    while cells.len() < 1_000 {
        let precision = [2, 4, 5, 6][rng.below(4)];
        cells.extend(sample_cells(precision, 1, &mut rng));
    }
    print!("| List |");
    for m in marks {
        print!(" {m} |");
    }
    println!("\n|---|---|---|---|---|---|---|");
    for (label, s) in [("iOS", &ios), ("Android", &android)] {
        print!("| {label} |");
        for m in marks {
            let set = union(s, cells[..m].iter().map(String::as_str));
            print!(" {} |", set.len());
        }
        println!();
    }
}
