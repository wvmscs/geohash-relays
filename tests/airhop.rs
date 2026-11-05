// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! AirHop's choice (`as_::airhop`): 10 nearest, one entry per canonical
//! address, ties by URL.

use geohash_relays::directory::{Platform, RelayDirectory};
use geohash_relays::select::closest;
use geohash_relays::{AirhopStrategy, Strategy};

const ANDROID: &str = include_str!("data/nostr_relays.csv");

#[test]
fn ten_relays_one_per_address_ties_by_url() {
    let csv = "Relay URL,Latitude,Longitude\n\
               zeta.example,52.0,5.0\n\
               alpha.example:443,52.0,5.0\n\
               alpha.example,52.0,5.0\n\
               far.example,10.0,5.0\n";
    let s = AirhopStrategy::new(csv);
    let urls: Vec<String> = s
        .read("u17")
        .unwrap()
        .into_iter()
        .map(|x| x.relay.url)
        .collect();
    assert_eq!(
        urls,
        [
            "wss://alpha.example",
            "wss://zeta.example",
            "wss://far.example"
        ]
    );
}

#[test]
fn its_first_five_are_what_bitchat_ios_picks_on_the_same_list() {
    let s = AirhopStrategy::new(ANDROID);
    let ios_rule = RelayDirectory::parse(ANDROID, Platform::Ios);
    for cell in ["u09tv", "dr5ru", "9q8yy", "gcpvj", "u0"] {
        let read = s.read(cell).unwrap();
        assert_eq!(read.len(), 10);
        let first: Vec<String> = read[..5].iter().map(|x| x.relay.url.clone()).collect();
        let ios: Vec<String> = closest(&ios_rule, cell, 5)
            .unwrap()
            .into_iter()
            .map(|r| r.url)
            .collect();
        assert_eq!(first, ios, "{cell}");
    }
}

#[test]
fn a_list_read_by_android_gives_the_same_choice() {
    let android = RelayDirectory::parse(ANDROID, Platform::Android);
    let (a, b) = (
        AirhopStrategy::new(ANDROID),
        AirhopStrategy::from_directory(&android),
    );
    assert_eq!(a.list().len(), b.list().len());
    for cell in ["u09tv", "dr5ru", "9q", "u0"] {
        assert_eq!(a.read(cell).unwrap(), b.read(cell).unwrap(), "{cell}");
    }
}
