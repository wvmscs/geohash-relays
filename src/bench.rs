// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Comparison bench (lot 1 of `PLAN_ET1.md`): the measures of §4 of the
//! requirements document, for any [`Strategy`]:
//!
//! * [`rendezvous`]: do two peers (two lists, two rules, two strategies)
//!   find each other: each one reads where the other writes (H1);
//! * [`load`]: how the cells spread over the servers;
//! * [`stability`]: how much the choice moves when a list changes;
//! * [`failures`]: the rendezvous when some chosen relays are dead;
//! * [`capture_cost`]: the servers an attacker must place to take every
//!   place of a cell (§7, question 4).
//!
//! Deterministic: the draws come from [`Rng`], seeded by
//! [`BENCH_SEED`](crate::params::BENCH_SEED).

use std::collections::{BTreeMap, HashSet};

use crate::directory::{Platform, RelayDirectory, normalize_url};
use crate::error::Result;
use crate::params::GEOHASH_ALPHABET;
use crate::strategy::{RelayHealth, Role, Selected, Strategy};

/// SplitMix64: small, deterministic, good enough for draws.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Every cell of `precision` characters (32^precision of them: keep it
/// small).
pub fn all_cells(precision: usize) -> Vec<String> {
    let mut cells = vec![String::new()];
    for _ in 0..precision {
        cells = cells
            .iter()
            .flat_map(|c| {
                GEOHASH_ALPHABET
                    .iter()
                    .map(move |&a| format!("{c}{}", a as char))
            })
            .collect();
    }
    cells
}

/// `n` cells of `precision` characters, drawn with `rng`.
pub fn sample_cells(precision: usize, n: usize, rng: &mut Rng) -> Vec<String> {
    (0..n)
        .map(|_| {
            (0..precision)
                .map(|_| GEOHASH_ALPHABET[rng.below(32)] as char)
                .collect()
        })
        .collect()
}

fn servers(chosen: &[Selected], writing: bool) -> HashSet<String> {
    chosen
        .iter()
        .filter(|s| writing || s.role == Role::ReadWrite)
        .map(|s| normalize_url(&s.relay.url).unwrap_or_else(|| s.relay.url.clone()))
        .collect()
}

/// Cells where two peers met, out of those tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendezvous {
    pub cells: usize,
    pub met: usize,
}

impl Rendezvous {
    pub fn ratio(&self) -> f64 {
        if self.cells == 0 {
            return 1.0;
        }
        self.met as f64 / self.cells as f64
    }
}

/// A peer reads what the other writes, both ways, `dead` relays aside.
fn meet(a: &dyn Strategy, b: &dyn Strategy, cell: &str, dead: &HashSet<String>) -> Result<bool> {
    let live = |set: HashSet<String>| -> HashSet<String> {
        set.into_iter().filter(|u| !dead.contains(u)).collect()
    };
    let (wa, ra) = (
        live(servers(&a.write(cell)?, true)),
        live(servers(&a.read(cell)?, false)),
    );
    let (wb, rb) = (
        live(servers(&b.write(cell)?, true)),
        live(servers(&b.read(cell)?, false)),
    );
    Ok(!wa.is_disjoint(&rb) && !wb.is_disjoint(&ra))
}

/// In how many `cells` a peer following `a` and a peer following `b` see
/// each other's messages.
pub fn rendezvous(a: &dyn Strategy, b: &dyn Strategy, cells: &[String]) -> Result<Rendezvous> {
    let mut r = Rendezvous {
        cells: cells.len(),
        met: 0,
    };
    for c in cells {
        if meet(a, b, c, &HashSet::new())? {
            r.met += 1;
        }
    }
    Ok(r)
}

/// How the cells spread over the servers (read sets).
#[derive(Debug, Clone, PartialEq)]
pub struct Load {
    /// Servers used by at least one cell.
    pub servers: usize,
    /// Cells of the busiest server.
    pub max_cells: usize,
    pub busiest: String,
    /// Cells whose read set has fewer than this many distinct servers…
    pub threshold: usize,
    /// … this many of them.
    pub below_threshold: usize,
}

/// The load of `s` over `cells`; `threshold`: count the cells with fewer
/// distinct servers (§4: 322 of 1,024 cells have only 4 with Android).
pub fn load(s: &dyn Strategy, cells: &[String], threshold: usize) -> Result<Load> {
    let mut per: BTreeMap<String, usize> = BTreeMap::new();
    let mut below = 0;
    for c in cells {
        let set = servers(&s.read(c)?, false);
        if set.len() < threshold {
            below += 1;
        }
        for u in set {
            *per.entry(u).or_default() += 1;
        }
    }
    let (busiest, max_cells) = per
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(u, n)| (u.clone(), *n))
        .unwrap_or_default();
    Ok(Load {
        servers: per.len(),
        max_cells,
        busiest,
        threshold,
        below_threshold: below,
    })
}

/// Mean Jaccard index of the read sets of `old` and `new` over `cells`: 1
/// when nothing moves.
pub fn stability(old: &dyn Strategy, new: &dyn Strategy, cells: &[String]) -> Result<f64> {
    let mut sum = 0.0;
    for c in cells {
        let (a, b) = (servers(&old.read(c)?, false), servers(&new.read(c)?, false));
        let union = a.union(&b).count();
        sum += if union == 0 {
            1.0
        } else {
            a.intersection(&b).count() as f64 / union as f64
        };
    }
    Ok(if cells.is_empty() {
        1.0
    } else {
        sum / cells.len() as f64
    })
}

/// The rendezvous when `dead` of the relays the two peers write to are down
/// (drawn per cell). The strategies are rebuilt with what they know of the
/// dead relays (`make_a`, `make_b`: a strategy that replaces failed relays,
/// A0, uses it; `AS` ignores it).
pub fn failures(
    make_a: &dyn Fn(&RelayHealth) -> Box<dyn Strategy>,
    make_b: &dyn Fn(&RelayHealth) -> Box<dyn Strategy>,
    cells: &[String],
    dead: usize,
    rng: &mut Rng,
) -> Result<Rendezvous> {
    let healthy = RelayHealth::new();
    let (a0, b0) = (make_a(&healthy), make_b(&healthy));
    let mut r = Rendezvous {
        cells: cells.len(),
        met: 0,
    };
    for c in cells {
        let mut chosen: Vec<String> = servers(&a0.write(c)?, true)
            .union(&servers(&b0.write(c)?, true))
            .cloned()
            .collect();
        chosen.sort();
        let mut down = HashSet::new();
        while down.len() < dead.min(chosen.len()) {
            down.insert(chosen[rng.below(chosen.len())].clone());
        }
        let health: RelayHealth = down
            .iter()
            .map(|u| (u.clone(), crate::strategy::Health::Dead))
            .collect();
        if meet(make_a(&health).as_ref(), make_b(&health).as_ref(), c, &down)? {
            r.met += 1;
        }
    }
    Ok(r)
}

/// The coordinates of the listed relay nearest to the centre of `cell`: where
/// IP geolocation puts the servers of that place (often a city centre).
pub fn nearest_point(dir: &RelayDirectory, cell: &str) -> Result<Option<(f64, f64)>> {
    Ok(crate::select::closest(dir, cell, 1)?
        .first()
        .map(|r| (r.lat, r.lon)))
}

/// How many servers an attacker must add to `csv`, all at `point` (where
/// geolocation puts its rented servers), for the strategy `make` builds to
/// read `cell` only from them. Attacker server `i` is named `name(i)`; the
/// lines are appended (Android keeps list order on ties). `None` beyond
/// `cap`.
pub fn capture_cost(
    make: &dyn Fn(&RelayDirectory) -> Box<dyn Strategy>,
    csv: &str,
    platform: Platform,
    cell: &str,
    point: (f64, f64),
    name: &dyn Fn(usize) -> String,
    cap: usize,
) -> Result<Option<usize>> {
    let mut text = csv.trim_end().to_owned();
    let mut ours = HashSet::new();
    for n in 1..=cap {
        let host = name(n);
        text.push_str(&format!("\n{host},{},{}", point.0, point.1));
        ours.insert(normalize_url(&host).unwrap_or(host));
        let dir = RelayDirectory::parse(&text, platform);
        let read = servers(&make(&dir).read(cell)?, false);
        if !read.is_empty() && read.is_subset(&ours) {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_are_deterministic() {
        let (mut a, mut b) = (Rng::new(1), Rng::new(1));
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(all_cells(1).len(), 32);
        assert_eq!(all_cells(2).len(), 1024);
        assert_eq!(all_cells(2)[33], "11");
        let s = sample_cells(5, 3, &mut Rng::new(2));
        assert!(
            s.iter()
                .all(|c| c.len() == 5 && crate::geohash::bbox(c).is_ok())
        );
    }
}
