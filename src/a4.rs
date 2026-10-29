// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A4`: republisher peers, which connect the relay sets
//! (`docs/geohash-relays-requirements-and-alternatives.md`,
//! §5 A4, and §6 step 1: the only approach usable with no change to the
//! apps).
//!
//! The events are signed: anybody can publish them again, nobody can change
//! them. A republisher reads the union of the relays of each cell (the iOS
//! set and the Android set of `AS`) and publishes each event on the relays
//! of its cell that did not send it. To limit amplification: one pass per
//! event id, only to the relays that do not have it, a limit per minute.
//!
//! [`Republisher`] is a machine without I/O ([`crate::sansio`]); the host
//! checks signatures before an event becomes an input.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use crate::directory::normalize_url;
use crate::params::{
    A4_BRIDGE_MAX_AGE_SECS, A4_HISTORY_LIMIT, A4_KINDS, A4_MAX_AGE_SECS, A4_PER_MINUTE,
    A4_SEEN_CAPACITY, A4_SETTLE_SECS,
};
use crate::sansio::{Action, EventMeta, FilterSpec, Input, Machine};
use crate::strategy::Strategy;

/// The relays of the cells a republisher connects: for each cell, the union
/// of the sets to connect (normalised URLs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    pub cells: BTreeMap<String, BTreeSet<String>>,
}

impl Plan {
    /// The plan of `cells` from the strategies whose relay sets must meet
    /// (`AS` iOS and `AS` Android, typically): each cell gets the union of
    /// what they write to and read from.
    pub fn of(cells: &[String], sets: &[&dyn Strategy]) -> crate::Result<Plan> {
        let mut plan = Plan::default();
        for c in cells {
            let entry = plan.cells.entry(c.clone()).or_default();
            for s in sets {
                for sel in s.write(c)?.into_iter().chain(s.read(c)?) {
                    entry.insert(normalize_url(&sel.relay.url).unwrap_or(sel.relay.url));
                }
            }
        }
        Ok(plan)
    }

    /// Every relay, with the cells it serves.
    pub fn relays(&self) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (cell, relays) in &self.cells {
            for r in relays {
                out.entry(r.clone()).or_default().push(cell.clone());
            }
        }
        out
    }
}

/// What a republisher did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Distinct events seen.
    pub seen: u64,
    /// Publications sent (one per event and relay).
    pub republished: u64,
    /// Events left aside: kind, age, cell.
    pub skipped: u64,
    /// Publications not sent: the limit per minute was reached.
    pub limited: u64,
    /// Publications a relay refused (`OK false`).
    pub refused: u64,
}

struct Pending {
    event: EventMeta,
    cell: String,
    /// Relays known to have it (they sent it, or accepted it).
    has: HashSet<String>,
    due: i64,
}

/// The republisher machine (A4).
pub struct Republisher {
    plan: Plan,
    pending: BTreeMap<String, Pending>,
    done: HashSet<String>,
    order: VecDeque<String>,
    /// Token bucket: publications left, and when it was last refilled.
    tokens: f64,
    refilled: i64,
    per_minute: u32,
    counters: Counters,
}

impl Republisher {
    pub fn new(plan: Plan) -> Self {
        Republisher::with_limit(plan, A4_PER_MINUTE)
    }

    /// With another limit per minute (tests, a busy area).
    pub fn with_limit(plan: Plan, per_minute: u32) -> Self {
        Republisher {
            plan,
            pending: BTreeMap::new(),
            done: HashSet::new(),
            order: VecDeque::new(),
            tokens: f64::from(per_minute),
            refilled: 0,
            per_minute,
            counters: Counters::default(),
        }
    }

    pub fn counters(&self) -> Counters {
        self.counters
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// The cell of an event: its `g` tag, or the `r` tag of the mesh
    /// bridge, if the plan has it.
    fn cell_of(&self, e: &EventMeta) -> Option<String> {
        e.tag_values("g")
            .chain(e.tag_values("r"))
            .find(|c| self.plan.cells.contains_key(*c))
            .map(str::to_owned)
    }

    fn admissible(&self, e: &EventMeta, now: i64) -> bool {
        let max_age = if e.tag("r").is_some() && e.tag("g").is_none() {
            A4_BRIDGE_MAX_AGE_SECS
        } else {
            A4_MAX_AGE_SECS
        };
        A4_KINDS.contains(&e.kind) && now - e.created_at <= max_age
    }

    fn remember(&mut self, id: String) {
        if self.done.insert(id.clone()) {
            self.order.push_back(id);
            while self.order.len() > A4_SEEN_CAPACITY {
                if let Some(old) = self.order.pop_front() {
                    self.done.remove(&old);
                }
            }
        }
    }

    fn refill(&mut self, now: i64) {
        let elapsed = (now - self.refilled).max(0) as f64;
        let rate = f64::from(self.per_minute) / 60.0;
        self.tokens = (self.tokens + elapsed * rate).min(f64::from(self.per_minute));
        self.refilled = now;
    }

    fn on_event(&mut self, relay: String, e: EventMeta, now: i64) {
        let relay = normalize_url(&relay).unwrap_or(relay);
        if let Some(p) = self.pending.get_mut(&e.id) {
            p.has.insert(relay);
            return;
        }
        if self.done.contains(&e.id) {
            return;
        }
        self.counters.seen += 1;
        let Some(cell) = self.cell_of(&e).filter(|_| self.admissible(&e, now)) else {
            self.counters.skipped += 1;
            self.remember(e.id);
            return;
        };
        self.pending.insert(
            e.id.clone(),
            Pending {
                has: HashSet::from([relay]),
                cell,
                event: e,
                due: now + A4_SETTLE_SECS,
            },
        );
    }

    fn on_tick(&mut self, now: i64) -> Vec<Action> {
        self.refill(now);
        let due: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, p)| p.due <= now)
            .map(|(id, _)| id.clone())
            .collect();
        let mut out = Vec::new();
        for id in due {
            let Some(p) = self.pending.remove(&id) else {
                continue;
            };
            let targets: Vec<String> = self.plan.cells[&p.cell]
                .iter()
                .filter(|r| !p.has.contains(*r))
                .cloned()
                .collect();
            for relay in targets {
                if self.tokens < 1.0 {
                    self.counters.limited += 1;
                    continue;
                }
                self.tokens -= 1.0;
                self.counters.republished += 1;
                out.push(Action::Publish {
                    relay,
                    id: p.event.id.clone(),
                    raw: p.event.raw.clone(),
                });
            }
            self.remember(id);
        }
        out
    }
}

impl Machine for Republisher {
    fn start(&mut self, now: i64) -> Vec<Action> {
        self.refilled = now;
        self.plan
            .relays()
            .into_iter()
            .map(|(relay, cells)| Action::Subscribe {
                relay,
                filter: FilterSpec {
                    kinds: A4_KINDS.to_vec(),
                    tags: vec![('g', cells)],
                    since: Some(now - A4_MAX_AGE_SECS),
                    limit: Some(A4_HISTORY_LIMIT),
                    ..FilterSpec::default()
                },
            })
            .collect()
    }

    fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Event { relay, event } => {
                // The time of the last tick: an event's own date says nothing.
                let now = self.refilled;
                self.on_event(relay, event, now);
                Vec::new()
            }
            Input::Ok {
                accepted: false, ..
            } => {
                self.counters.refused += 1;
                Vec::new()
            }
            Input::Tick { now } => self.on_tick(now),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            cells: BTreeMap::from([(
                "u09tv".to_owned(),
                BTreeSet::from([
                    "wss://ios.example".to_owned(),
                    "wss://android.example".to_owned(),
                ]),
            )]),
        }
    }

    fn ev(id: &str, kind: u32, at: i64, cell: &str) -> EventMeta {
        EventMeta {
            id: id.into(),
            pubkey: "p".into(),
            kind,
            created_at: at,
            tags: vec![vec!["g".into(), cell.into()]],
            raw: format!("{{\"id\":\"{id}\"}}"),
        }
    }

    fn published(actions: &[Action]) -> Vec<(String, String)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Publish { relay, id, .. } => Some((relay.clone(), id.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn subscribes_to_the_union() {
        let mut r = Republisher::new(plan());
        let a = r.start(1000);
        assert_eq!(a.len(), 2);
        assert!(a.iter().all(|a| matches!(a, Action::Subscribe { filter, .. }
            if filter.tags == [('g', vec!["u09tv".to_owned()])] && filter.since == Some(1000 - 3600))));
    }

    #[test]
    fn republishes_once_where_it_is_missing() {
        let mut r = Republisher::new(plan());
        r.start(1000);
        let input = |relay: &str, e: EventMeta| Input::Event {
            relay: relay.into(),
            event: e,
        };
        r.handle(input("wss://ios.example", ev("e1", 20000, 1000, "u09tv")));
        // Not yet: the other relays may still send it.
        assert!(published(&r.handle(Input::Tick { now: 1001 })).is_empty());
        let out = published(&r.handle(Input::Tick { now: 1003 }));
        assert_eq!(out, [("wss://android.example".to_owned(), "e1".to_owned())]);
        // Seen again (our own publication comes back): nothing more.
        r.handle(input(
            "wss://android.example",
            ev("e1", 20000, 1000, "u09tv"),
        ));
        assert!(published(&r.handle(Input::Tick { now: 1010 })).is_empty());
        // Sent by both before it is due: nothing to do.
        r.handle(input("wss://ios.example", ev("e2", 20001, 1010, "u09tv")));
        r.handle(input(
            "wss://android.example",
            ev("e2", 20001, 1010, "u09tv"),
        ));
        assert!(published(&r.handle(Input::Tick { now: 1020 })).is_empty());
        assert_eq!(r.counters().republished, 1);
        assert_eq!(r.counters().seen, 2);
    }

    #[test]
    fn leaves_aside_old_foreign_and_other_kinds() {
        let mut r = Republisher::new(plan());
        r.start(10_000);
        for e in [
            ev("old", 20000, 10_000 - 3601, "u09tv"),
            ev("dm", 1059, 10_000, "u09tv"),
            ev("far", 20000, 10_000, "9q8yy"),
        ] {
            r.handle(Input::Event {
                relay: "wss://ios.example".into(),
                event: e,
            });
        }
        assert!(published(&r.handle(Input::Tick { now: 10_010 })).is_empty());
        assert_eq!(r.counters().skipped, 3);
    }

    #[test]
    fn the_limit_per_minute_holds() {
        let mut r = Republisher::with_limit(plan(), 2);
        r.start(0);
        for i in 0..5 {
            r.handle(Input::Event {
                relay: "wss://ios.example".into(),
                event: ev(&format!("e{i}"), 20000, 0, "u09tv"),
            });
        }
        let out = published(&r.handle(Input::Tick { now: 3 }));
        assert_eq!(out.len(), 2);
        assert_eq!(r.counters().limited, 3);
        // A minute later, the bucket is full again.
        r.handle(Input::Event {
            relay: "wss://ios.example".into(),
            event: ev("e9", 20000, 60, "u09tv"),
        });
        assert_eq!(published(&r.handle(Input::Tick { now: 64 })).len(), 1);
    }
}
