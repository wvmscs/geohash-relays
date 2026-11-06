// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Sonar's choice (`as_::sonar`): 12 nearest, lines as they are, ties in the
//! list's order; the pool on request.

use geohash_relays::directory::{Platform, RelayDirectory};
use geohash_relays::select::closest;
use geohash_relays::{Origin, SonarStrategy, Strategy};

const SONAR: &str = include_str!("data/sonar-relays_gps-2026-06-15.csv");

#[test]
fn twelve_relays_lines_as_they_are_ties_in_list_order() {
    let csv = "Relay URL,Latitude,Longitude\n\
               zeta.example,52.0,5.0\n\
               alpha.example:443,52.0,5.0\n\
               alpha.example,52.0,5.0\n";
    let urls: Vec<String> = SonarStrategy::new(csv)
        .read("u17")
        .unwrap()
        .into_iter()
        .map(|s| s.relay.url)
        .collect();
    // Android's reading and rule: `host` and `host:443` twice, file order.
    assert_eq!(
        urls,
        [
            "wss://zeta.example",
            "wss://alpha.example:443",
            "wss://alpha.example"
        ]
    );
}

#[test]
fn its_first_five_are_what_bitchat_android_picks_on_the_same_list() {
    let s = SonarStrategy::new(SONAR);
    let android = RelayDirectory::parse(SONAR, Platform::Android);
    for cell in ["u09tv", "dr5ru", "9q8yy", "sr2yk", "xn76"] {
        let read = s.read(cell).unwrap();
        assert_eq!(read.len(), 12);
        let first: Vec<&str> = read[..5].iter().map(|x| x.relay.url.as_str()).collect();
        let android: Vec<String> = closest(&android, cell, 5)
            .unwrap()
            .into_iter()
            .map(|r| r.url)
            .collect();
        assert_eq!(first, android, "{cell}");
    }
}

#[test]
fn the_pool_adds_the_default_relays_once() {
    let read = SonarStrategy::new(SONAR).with_pool().read("u09tv").unwrap();
    let pool: Vec<&str> = read
        .iter()
        .filter(|s| s.origin == Origin::Shared)
        .map(|s| s.relay.url.as_str())
        .collect();
    assert!(pool.len() <= 5 && pool.contains(&"wss://relay.damus.io"));
    assert_eq!(read.len(), 12 + pool.len());
}

#[test]
fn the_cells_opened_before_stay_in_the_pool() {
    let alone = SonarStrategy::new(SONAR).read("u09tv").unwrap();
    let paris = SonarStrategy::new(SONAR).read("u09").unwrap();
    let read = SonarStrategy::new(SONAR)
        .with_history(&["u09", "u09tv"])
        .read("u09tv")
        .unwrap();
    // The cell's own 12 first, then those of `u09` it lacks, once each.
    assert_eq!(read[..12], alone[..]);
    for s in &paris {
        assert!(
            read.iter().any(|r| r.relay.url == s.relay.url),
            "{}",
            s.relay.url
        );
    }
    let extra = read.iter().filter(|s| s.origin == Origin::Shared).count();
    assert_eq!(read.len(), 12 + extra);
    assert!(extra <= 12);
}

#[test]
fn the_levels_of_a_place_are_its_coarser_channels() {
    let levels = SonarStrategy::new(SONAR)
        .with_levels()
        .read("u09tvw")
        .unwrap();
    let history = SonarStrategy::new(SONAR)
        .with_history(&["u0", "u09t", "u09tv"])
        .read("u09tvw")
        .unwrap();
    assert_eq!(levels, history);
    // A region (precision 2) has no coarser channel.
    assert_eq!(
        SonarStrategy::new(SONAR).with_levels().read("u0").unwrap(),
        SonarStrategy::new(SONAR).read("u0").unwrap()
    );
}
