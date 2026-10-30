// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `A5`: federation of synchronised relays (NIP-77, negentropy)
//! (`docs/geohash-relays-requirements-and-alternatives.md`,
//! §5 A5; decision G11 of `PLAN_ET1.md`).
//!
//! Relays of a region synchronise the kinds 20000, 20001 and 1 that have a
//! `g` tag (`docs/federation.md`: `strfry router` for live events, `strfry
//! sync` for stored ones). A client then needs only **one** relay of the
//! federation, not the same one as its neighbours: [`Federation`] reads from
//! [`A5_READ`] members and writes to [`A5_WRITE`], chosen per cell by
//! rendezvous hashing (unsalted: the members are known and trusted).
//!
//! Events are signed: a member can only omit events, not change them.
//! [`Comparer`] reads the same window from every member and
//! [`omissions`] says what each one lacks.

use std::collections::{BTreeMap, BTreeSet};

use crate::a1::score;
use crate::directory::{normalize_url, one_per_server};
use crate::error::Result;
use crate::params::{A1_PREFIX, A5_KINDS, A5_READ, A5_WINDOW_SECS, A5_WRITE};
use crate::sansio::{Action, FilterSpec, Input, Machine};
use crate::strategy::{GeoRelay, Origin, Role, Selected, Strategy};

/// What this strategy asks of a candidate relay: A5's relays are the federation's members.
pub const NEEDS: crate::strategy::Needs = crate::strategy::Needs { place: false };

/// The NIP-01 filter of what a federation synchronises for these cells
/// (JSON, for `strfry sync --filter` or `strfry router`). Without cells, the
/// kinds only: kind 1 notes without `g` come too.
pub fn sync_filter(cells: &[String]) -> String {
    let kinds = A5_KINDS
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if cells.is_empty() {
        return format!("{{\"kinds\":[{kinds}]}}");
    }
    let g = cells
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"kinds\":[{kinds}],\"#g\":[{g}]}}")
}

/// For each member, the ids that some other member has and it has not.
pub fn omissions(seen: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, BTreeSet<String>> {
    let all: BTreeSet<&String> = seen.values().flatten().collect();
    seen.iter()
        .map(|(relay, ids)| {
            let missing = all
                .iter()
                .filter(|id| !ids.contains(**id))
                .map(|id| (*id).clone())
                .collect();
            (relay.clone(), missing)
        })
        .collect()
}

/// `A5`: the members of a federation, the same for every cell.
#[derive(Debug, Clone)]
pub struct Federation {
    members: Vec<String>,
}

impl Federation {
    pub fn new(members: &[String]) -> Self {
        Federation {
            members: one_per_server(members.iter().filter_map(|u| normalize_url(u))),
        }
    }

    fn ranked(&self, cell: &str) -> Vec<String> {
        let mut m: Vec<(u64, &String)> = self
            .members
            .iter()
            .map(|u| (score(&[], cell, A1_PREFIX, u), u))
            .collect();
        m.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        m.into_iter().map(|(_, u)| u.clone()).collect()
    }

    fn pick(&self, cell: &str, n: usize, role: Role) -> Result<Vec<Selected>> {
        crate::geohash::bbox(cell)?;
        Ok(self
            .ranked(cell)
            .into_iter()
            .take(n)
            .enumerate()
            .map(|(i, u)| Selected::new(GeoRelay::unplaced(u), i + 1, role, Origin::List))
            .collect())
    }
}

impl Strategy for Federation {
    fn name(&self) -> &str {
        "A5"
    }

    fn describe(&self) -> String {
        format!(
            "A5 (federation of {}, read {}, write {})",
            self.members.len(),
            A5_READ,
            A5_WRITE
        )
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        self.pick(geohash, A5_READ, Role::ReadWrite)
    }

    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        // The member it writes to is the first it reads: a peer sees its
        // own messages without waiting for the synchronisation.
        self.pick(geohash, A5_WRITE, Role::ReadWrite)
    }
}

/// The machine that compares the members: the ids each one has for these
/// cells over the last [`A5_WINDOW_SECS`].
pub struct Comparer {
    members: Vec<String>,
    cells: Vec<String>,
    seen: BTreeMap<String, BTreeSet<String>>,
    eose: BTreeSet<String>,
    failed: BTreeSet<String>,
}

impl Comparer {
    pub fn new(members: &[String], cells: &[String]) -> Self {
        let members: Vec<String> = Federation::new(members).members;
        Comparer {
            seen: members
                .iter()
                .map(|m| (m.clone(), BTreeSet::new()))
                .collect(),
            members,
            cells: cells.to_vec(),
            eose: BTreeSet::new(),
            failed: BTreeSet::new(),
        }
    }

    /// Every member answered (or failed).
    pub fn done(&self) -> bool {
        self.members
            .iter()
            .all(|m| self.eose.contains(m) || self.failed.contains(m))
    }

    /// What each member that answered lacks.
    pub fn omissions(&self) -> BTreeMap<String, BTreeSet<String>> {
        let answered: BTreeMap<String, BTreeSet<String>> = self
            .seen
            .iter()
            .filter(|(m, _)| self.eose.contains(*m))
            .map(|(m, s)| (m.clone(), s.clone()))
            .collect();
        omissions(&answered)
    }

    /// Members that could not be read.
    pub fn failed(&self) -> &BTreeSet<String> {
        &self.failed
    }
}

impl Machine for Comparer {
    fn start(&mut self, now: i64) -> Vec<Action> {
        self.members
            .iter()
            .map(|m| Action::Subscribe {
                relay: m.clone(),
                filter: FilterSpec {
                    kinds: A5_KINDS.to_vec(),
                    tags: vec![('g', self.cells.clone())],
                    since: Some(now - A5_WINDOW_SECS),
                    ..FilterSpec::default()
                },
            })
            .collect()
    }

    fn handle(&mut self, input: Input) -> Vec<Action> {
        let norm = |r: String| normalize_url(&r).unwrap_or(r);
        match input {
            Input::Event { relay, event } => {
                let relay = norm(relay);
                if !self.eose.contains(&relay)
                    && let Some(s) = self.seen.get_mut(&relay)
                {
                    s.insert(event.id);
                }
            }
            Input::Eose { relay } => {
                self.eose.insert(norm(relay));
            }
            Input::Failed { relay, .. } => {
                self.failed.insert(norm(relay));
            }
            _ => {}
        }
        Vec::new()
    }
}
