// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The `AS` strategy on the frozen lists.

use geohash_relays::geohash::with_neighbours;
use geohash_relays::{AppStrategy, Apps, Platform, RelayDirectory, Strategy, select_both};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const ANDROID: &str = include_str!("data/nostr_relays.csv");

fn strategy(apps: Apps) -> AppStrategy {
    AppStrategy::new(
        Some(RelayDirectory::parse(IOS, Platform::Ios)),
        Some(RelayDirectory::parse(ANDROID, Platform::Android)),
        apps,
    )
}

#[test]
fn as_follows_the_apps() {
    let both = strategy(Apps::Both);
    assert_eq!(both.name(), "AS");
    let ios = RelayDirectory::parse(IOS, Platform::Ios);
    let android = RelayDirectory::parse(ANDROID, Platform::Android);
    assert_eq!(
        both.read("u09tv").unwrap(),
        select_both(Some(&ios), Some(&android), "u09tv").unwrap()
    );
    // Writes where it reads.
    assert_eq!(both.write("u09tv").unwrap(), both.read("u09tv").unwrap());

    let only_ios = strategy(Apps::Ios).read("u09tv").unwrap();
    assert_eq!(only_ios.len(), 5);
    assert!(
        only_ios
            .iter()
            .all(|s| s.ios.is_some() && s.android.is_empty())
    );
    let only_android = strategy(Apps::Android).read("u09tv").unwrap();
    assert!(
        only_android
            .iter()
            .all(|s| s.ios.is_none() && !s.android.is_empty())
    );
    assert!(strategy(Apps::Both).read("u0a").is_err());
}

#[test]
fn nine_cells_and_their_servers() {
    // What bitchat-console follows: a cell, its 8 neighbours, the union of
    // their relays.
    let s = strategy(Apps::Both);
    let cells = with_neighbours("u09tv").unwrap();
    let mut servers: Vec<String> = cells
        .iter()
        .flat_map(|c| s.read(c).unwrap())
        .map(|sel| sel.relay.url)
        .collect();
    servers.sort();
    servers.dedup();
    assert!(servers.len() >= 8, "{servers:?}");
    assert!(servers.iter().all(|u| u.starts_with("wss://")));
}

#[test]
fn default_relays_and_lists() {
    assert_eq!(Platform::Ios.default_relays().len(), 4);
    assert!(
        Platform::Android
            .default_relays()
            .contains(&"wss://nostr21.com")
    );
    assert!(Platform::Ios.list_url().ends_with("online_relays_gps.csv"));
    assert!(Platform::Android.list_url().ends_with("nostr_relays.csv"));
}

#[test]
fn ranks_roles_and_origins() {
    use geohash_relays::{Origin, Role};
    let s = strategy(Apps::Both).read("u09tv").unwrap();
    for (i, sel) in s.iter().enumerate() {
        assert_eq!(sel.rank, i + 1);
        assert_eq!(sel.role, Role::ReadWrite);
        let expected = match (sel.ios.is_some(), sel.android.is_empty()) {
            (true, true) => Origin::Ios,
            (false, _) => Origin::Android,
            (true, false) => Origin::Both,
        };
        assert_eq!(sel.origin, expected);
    }
    assert!(s.iter().any(|x| x.origin == Origin::Both));
    let ios = AppStrategy::ios(RelayDirectory::parse(IOS, Platform::Ios));
    assert!(
        ios.read("u09tv")
            .unwrap()
            .iter()
            .all(|x| x.origin == Origin::Ios)
    );
    assert_eq!(ios.describe(), "AS (iOS, 5 nearest)");
}
