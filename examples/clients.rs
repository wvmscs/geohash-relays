// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The clients of the location channels today (`docs/clients.md`): bitchat
//! iOS, bitchat Android and AirHop, each with its own list and rule. Do
//! their users meet in a cell?
//!
//!   cargo run --release --example clients
//!
//! The lists are the frozen ones of `tests/data` (see its README); "alive":
//! only the relays the census of 2026-10-30 found answering and not refused
//! by A6 carry anything.

use std::collections::HashSet;

use geohash_relays::bench::{self, Rng};
use geohash_relays::params::BENCH_SEED;
use geohash_relays::{
    AirhopStrategy, AppStrategy, Platform, RelayDirectory, Strategy, normalize_url,
};

const IOS: &str = include_str!("../tests/data/online_relays_gps.csv");
const ANDROID: &str = include_str!("../tests/data/nostr_relays-2026-10-29.csv");
const AIRHOP: &str = include_str!("../tests/data/airhop-nostr_relays-v1.0.10.csv");
const ALIVE: &str = include_str!("../tests/data/census-usable-2026-10-30.txt");

fn pct(r: bench::Rendezvous) -> String {
    format!("{:.1} %", 100.0 * r.ratio())
}

/// Mean number of chosen servers that are alive, per cell.
fn alive_per_cell(s: &dyn Strategy, cells: &[String], alive: &HashSet<String>) -> f64 {
    let mut n = 0usize;
    for c in cells {
        n += s
            .read(c)
            .unwrap()
            .iter()
            .filter(|x| alive.contains(&normalize_url(&x.relay.url).unwrap_or_default()))
            .count();
    }
    n as f64 / cells.len() as f64
}

/// Servers of a list, normalised.
fn servers(dir: &RelayDirectory) -> HashSet<String> {
    dir.relays()
        .iter()
        .filter_map(|r| normalize_url(&r.url))
        .collect()
}

fn main() {
    let alive: HashSet<String> = ALIVE.lines().filter_map(normalize_url).collect();
    let ios = AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios));
    let android = AppStrategy::android(RelayDirectory::parse(ANDROID, Platform::Android));
    let airhop = AirhopStrategy::new(AIRHOP);
    let clients: [(&str, &dyn Strategy); 3] = [
        ("bitchat iOS", &ios),
        ("bitchat Android", &android),
        ("AirHop", &airhop),
    ];

    println!("## Lists\n");
    println!("| | Entries | Servers | Alive (census) |\n|---|---|---|---|");
    for (name, csv, p) in [
        ("bitchat iOS (2026-07-26)", IOS, Platform::Ios),
        (
            "bitchat Android (georelays 2026-10-29)",
            ANDROID,
            Platform::Android,
        ),
        ("AirHop v1.0.10", AIRHOP, Platform::Ios),
    ] {
        let dir = RelayDirectory::parse(csv, p);
        let s = servers(&dir);
        let a = s.iter().filter(|u| alive.contains(*u)).count();
        println!("| {name} | {} | {} | {a} |", dir.len(), s.len());
    }
    let (si, sa, sh) = (
        servers(&RelayDirectory::parse(IOS, Platform::Ios)),
        servers(&RelayDirectory::parse(ANDROID, Platform::Android)),
        servers(&RelayDirectory::parse(AIRHOP, Platform::Ios)),
    );
    println!(
        "\nIn common: iOS and Android {}, iOS and AirHop {}, Android and AirHop {}.\n",
        si.intersection(&sa).count(),
        si.intersection(&sh).count(),
        sa.intersection(&sh).count()
    );

    let p2 = bench::all_cells(2);
    let p5 = bench::sample_cells(5, 500, &mut Rng::new(BENCH_SEED));
    println!("## Rendezvous (a message from one reaches the other, both ways)\n");
    println!(
        "| Pair | p2, all relays | p2, alive | p5, all relays | p5, alive |\n|---|---|---|---|---|"
    );
    for i in 0..clients.len() {
        for j in i + 1..clients.len() {
            let (na, a) = clients[i];
            let (nb, b) = clients[j];
            println!(
                "| {na} – {nb} | {} | {} | {} | {} |",
                pct(bench::rendezvous(a, b, &p2).unwrap()),
                pct(bench::rendezvous_among(a, b, &p2, &alive).unwrap()),
                pct(bench::rendezvous(a, b, &p5).unwrap()),
                pct(bench::rendezvous_among(a, b, &p5, &alive).unwrap()),
            );
        }
    }
    // The same client on both sides: only dead relays separate them.
    println!("\n## Two users of the same client, alive relays only\n");
    println!("| Client | p2 | p5 | Alive relays per cell (p2) |\n|---|---|---|---|");
    for (name, s) in clients {
        println!(
            "| {name} | {} | {} | {:.1} |",
            pct(bench::rendezvous_among(s, s, &p2, &alive).unwrap()),
            pct(bench::rendezvous_among(s, s, &p5, &alive).unwrap()),
            alive_per_cell(s, &p2, &alive)
        );
    }
}
