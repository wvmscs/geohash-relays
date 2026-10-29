// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! A2: NIP-66 reports (format of `nostr-protocol/nips` `66.md`, 2026-10-29),
//! the m-of-n directory, the guard, the watcher.

use std::collections::BTreeSet;

use geohash_relays::a1::{HrwStrategy, Salts};
use geohash_relays::a2::{Announcement, Directory, Report, Watcher};
use geohash_relays::a6::Finding;
use geohash_relays::sansio::{Action, EventMeta, Input, Machine};
use geohash_relays::{Platform, RelayDirectory, Strategy};

const NOW: i64 = 1_800_000_000;

fn report(monitor: &str, relay: &str, tags: &[(&str, &str)], age: i64) -> EventMeta {
    let mut t = vec![vec!["d".to_owned(), relay.to_owned()]];
    t.extend(
        tags.iter()
            .map(|(k, v)| vec![(*k).to_owned(), (*v).to_owned()]),
    );
    EventMeta {
        id: format!("{monitor}{relay}{age}"),
        pubkey: monitor.into(),
        kind: 30166,
        created_at: NOW - age,
        tags: t,
        raw: "{\"content\":\"\"}".into(),
    }
}

fn parsed(e: &EventMeta) -> Report {
    Report::parse(e, "").unwrap()
}

fn trusted(m: &[&str]) -> BTreeSet<String> {
    m.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn reads_a_report_like_the_nip() {
    // The example of NIP-66.
    let e = report(
        "m1",
        "wss://some.relay/",
        &[
            ("n", "clearnet"),
            ("N", "40"),
            ("R", "!payment"),
            ("R", "auth"),
            ("g", "ww8p1r4t8"),
            ("k", "!20000"),
        ],
        0,
    );
    let r = parsed(&e);
    assert_eq!(r.relay, "wss://some.relay");
    assert_eq!(r.requirements.get("payment"), Some(&false));
    assert_eq!(r.requirements.get("auth"), Some(&true));
    assert_eq!(r.geohash.as_deref(), Some("ww8p1r4t8"));
    assert!(r.refused_kinds.contains(&20000));
    let v = r.verdict();
    assert_eq!(v.refusals, [Finding::AuthRequired, Finding::Kind20000]);
    let mut a = e.clone();
    a.kind = 10166;
    a.tags = vec![
        vec!["frequency".into(), "3600".into()],
        vec!["c".into(), "write".into()],
    ];
    let ann = Announcement::parse(&a).unwrap();
    assert_eq!(ann.frequency_secs, Some(3600));
    assert!(ann.checks.contains("write"));
}

#[test]
fn m_of_n_and_majority() {
    let reports: Vec<Report> = [
        report("m1", "wss://a", &[("g", "u09tv")], 10),
        report("m2", "wss://a", &[("g", "u09tv")], 10),
        // Only one monitor: not a candidate.
        report("m1", "wss://b", &[], 10),
        // Two monitors, but one hostile monitor alone says NIP-42: kept.
        report("m1", "wss://c", &[("R", "auth")], 10),
        report("m2", "wss://c", &[], 10),
        report("m3", "wss://c", &[], 10),
        // The majority says NIP-42: refused.
        report("m1", "wss://d", &[("R", "auth")], 10),
        report("m2", "wss://d", &[("R", "auth")], 10),
        // Too old.
        report("m1", "wss://e", &[], 4 * 86_400),
        report("m2", "wss://e", &[], 4 * 86_400),
        // An untrusted monitor counts for nothing.
        report("x", "wss://f", &[], 10),
        report("m1", "wss://f", &[], 10),
    ]
    .iter()
    .map(parsed)
    .collect();
    let d = Directory::build(&reports, &trusted(&["m1", "m2", "m3"]), NOW, 2);
    let names: Vec<&str> = d.candidates.iter().map(|c| c.relay.as_str()).collect();
    assert_eq!(names, ["wss://a", "wss://c"]);
    assert_eq!(d.candidates[0].geohash.as_deref(), Some("u09tv"));
    assert!(d.qualification.shared);
    assert!(!d.qualification.allows("wss://d"));
    assert!(
        d.qualification.allows("wss://b"),
        "unknown to the threshold: kept"
    );
    // A1 on the directory.
    let list = d.to_list();
    assert_eq!(list.len(), 2);
    let s = HrwStrategy::new(&list).with_salts(Salts::Fixed(vec![1]), 0);
    assert_eq!(s.read("9q8yy").unwrap().len(), 2);
}

#[test]
fn the_guard_keeps_a_list_the_monitors_would_empty() {
    let list = RelayDirectory::parse("wss://a,1,1\nwss://b,1,1\nwss://c,1,1\n", Platform::Ios);
    let refuse = |r: &str| {
        [
            report("m1", r, &[("R", "payment")], 1),
            report("m2", r, &[("R", "payment")], 1),
        ]
    };
    let mut reports: Vec<Report> = refuse("wss://a").iter().map(parsed).collect();
    let d = Directory::build(&reports, &trusted(&["m1", "m2"]), NOW, 2);
    let (kept, used) = d.guard(list.clone()).unwrap();
    assert!(used);
    assert_eq!(kept.len(), 2);
    reports.extend(refuse("wss://b").iter().map(parsed));
    let d = Directory::build(&reports, &trusted(&["m1", "m2"]), NOW, 2);
    let (kept, used) = d.guard(list).unwrap();
    assert!(!used, "two thirds removed: not used");
    assert_eq!(kept.len(), 3);
}

#[test]
fn the_watcher_subscribes_and_collects() {
    let mut w = Watcher::new(
        vec!["wss://monitors.example".into()],
        trusted(&["m1", "m2"]),
    );
    let start = w.start(NOW);
    assert!(matches!(&start[..], [Action::Subscribe { filter, .. }]
        if filter.kinds == [30166, 10166] && filter.authors == ["m1", "m2"]));
    for m in ["m1", "m2", "x"] {
        w.handle(Input::Event {
            relay: "wss://monitors.example".into(),
            event: report(m, "wss://a", &[], 5),
        });
    }
    assert!(!w.caught_up());
    w.handle(Input::Eose {
        relay: "wss://monitors.example".into(),
    });
    assert!(w.caught_up());
    assert_eq!(w.reports().len(), 2);
    assert_eq!(w.default_directory().candidates.len(), 1);
}
