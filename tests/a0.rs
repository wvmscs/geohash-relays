// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A0 on the frozen iOS list: write wide, read narrow, replacement.

use geohash_relays::a0::{ListVersion, SharedStrategy};
use geohash_relays::bench::{self, Rng};
use geohash_relays::{Health, Origin, RelayHealth, Role, Strategy};

const IOS: &str = include_str!("data/online_relays_gps.csv");
const IOS_OLD: &str = include_str!("data/online_relays_gps-2026-06-21.csv");

fn urls(v: &[geohash_relays::Selected]) -> Vec<String> {
    v.iter().map(|s| s.relay.url.clone()).collect()
}

#[test]
fn writes_eight_reads_five() {
    let s = SharedStrategy::new(IOS);
    let (r, w) = (s.read("u09tv").unwrap(), s.write("u09tv").unwrap());
    assert_eq!(r.len(), 5);
    assert_eq!(w.len(), 8);
    assert!(
        r.iter()
            .all(|x| x.role == Role::ReadWrite && x.origin == Origin::List)
    );
    assert_eq!(w.iter().filter(|x| x.role == Role::WriteOnly).count(), 3);
    assert_eq!(urls(&w)[..5], urls(&r)[..]);
    // The first five are the iOS choice.
    let ios = geohash_relays::AppStrategy::ios(geohash_relays::RelayDirectory::parse(
        IOS,
        geohash_relays::Platform::Ios,
    ));
    assert_eq!(urls(&r), urls(&ios.read("u09tv").unwrap()));
}

#[test]
fn a_dead_relay_is_replaced_for_reading() {
    let s = SharedStrategy::new(IOS);
    let r = s.read("u09tv").unwrap();
    let dead = r[0].relay.url.clone();
    let s = s.with_health(RelayHealth::from([(dead.clone(), Health::AuthRequired)]));
    let r2 = s.read("u09tv").unwrap();
    assert_eq!(r2.len(), 5);
    assert!(!urls(&r2).contains(&dead));
    assert_eq!(r2[4].rank, 6);
}

#[test]
fn peers_that_lost_fewer_than_r_still_meet() {
    // Every pair of dead sets of 4 relays among the first 8, one per peer:
    // with write wide they always meet; reading and writing the same 5
    // (no write wide) they do not.
    let cells = bench::sample_cells(5, 40, &mut Rng::new(7));
    let mut rng = Rng::new(11);
    for cell in &cells {
        let order = urls(&SharedStrategy::new(IOS).write(cell).unwrap());
        for _ in 0..10 {
            let mut pick = |n: usize| -> RelayHealth {
                let mut h = RelayHealth::new();
                while h.len() < n {
                    h.insert(order[rng.below(8)].clone(), Health::Dead);
                }
                h
            };
            let (ha, hb) = (pick(4), pick(4));
            let a = SharedStrategy::new(IOS).with_health(ha.clone());
            let b = SharedStrategy::new(IOS).with_health(hb.clone());
            // A writes to its live first 8; B reads its first 5 live.
            let wa: Vec<String> = urls(&a.write(cell).unwrap());
            let rb = urls(&b.read(cell).unwrap());
            assert!(
                rb.iter().any(|u| wa.contains(u)),
                "{cell}: A lost {ha:?}, B lost {hb:?}"
            );
        }
    }
    // Today's design (AS: the same 5, no replacement): two peers that lost
    // different relays miss each other; A0 makes them meet.
    let cell = "u09tv";
    let order = urls(&SharedStrategy::new(IOS).write(cell).unwrap());
    let first5: Vec<&String> = order.iter().take(5).collect();
    let (da, db) = (&order[0..4], &order[4..5]);
    let a_today: Vec<&&String> = first5.iter().filter(|u| !da.contains(u)).collect();
    let b_today: Vec<&&String> = first5.iter().filter(|u| !db.contains(u)).collect();
    assert!(a_today.iter().all(|u| !b_today.contains(u)));
    let dead = |d: &[String]| d.iter().map(|u| (u.clone(), Health::Dead)).collect();
    let a = SharedStrategy::new(IOS).with_health(dead(da));
    let b = SharedStrategy::new(IOS).with_health(dead(db));
    let wa = urls(&a.write(cell).unwrap());
    assert!(urls(&b.read(cell).unwrap()).iter().any(|u| wa.contains(u)));
}

#[test]
fn shared_relays_and_versions() {
    let s = SharedStrategy::new(IOS).with_shared(1);
    let r = s.read("9q8yy").unwrap();
    assert_eq!(r.len(), 6);
    assert_eq!(r[5].origin, Origin::Shared);
    assert_eq!(r[5].relay.url, "wss://relay.damus.io");
    assert_eq!(
        SharedStrategy::new(IOS)
            .with_shared(9)
            .read("9q")
            .unwrap()
            .len(),
        7
    );
    let v = ListVersion::of(3, IOS);
    assert_eq!(v, ListVersion::of(3, &IOS.replace('\n', "\r\n")));
    assert_ne!(v.sha256, ListVersion::of(3, IOS_OLD).sha256);
    assert!(
        SharedStrategy::new(IOS)
            .with_version(v)
            .describe()
            .starts_with("A0 (iOS rule, w = 8, r = 5, 0 shared, list v3 ")
    );
}

#[test]
fn one_list_one_rule_beats_two() {
    // Two peers with the same list always meet; with list versions a
    // month apart they still meet more often than iOS and Android today.
    let cells = bench::all_cells(2);
    let (new, old) = (SharedStrategy::new(IOS), SharedStrategy::new(IOS_OLD));
    assert_eq!(bench::rendezvous(&new, &new, &cells).unwrap().met, 1024);
    let r = bench::rendezvous(&new, &old, &cells).unwrap();
    assert!(r.ratio() > 0.95, "{r:?}");
}
