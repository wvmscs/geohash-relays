// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! What a gateway reads and where it publishes, within fixed limits
//! (`docs/gateways.md`, section 7).
//!
//! A gateway is a peer of the Bluetooth mesh that also has Internet. Its
//! peers deposit events for the cells they choose; it reads cells for them.
//! Without limits, the relays it needs grow with the places its peers
//! choose. This module keeps them bounded:
//!
//! - it reads a fixed set of cells (the served cells), which it announces;
//! - it reads another cell only on a peer's request, for a fixed time, and
//!   refuses the request when the links would pass the limit;
//! - it publishes to any cell: through a link it already reads, or through
//!   a small pool of links, the least recently used one closed first;
//! - it reads each relay through one link, with all its cells in one filter,
//!   or through one link per relay and cell ([`Isolation`]).
//!
//! No I/O: the host opens and closes the links that [`Gateway`] names. The
//! relays of a cell come from the caller ([`CellRelays`], for example from
//! [`Relays::get`](super::Relays::get)).

use std::collections::{BTreeMap, BTreeSet};

use super::types::{Cell, CellRelays, RelayUrl};
use crate::error::Result;
use crate::geohash::{encode, neighbours};
use crate::params::{
    CHANNEL_LEVELS, GATEWAY_LEASE_SECS, GATEWAY_LEASES_PER_PEER, GATEWAY_MAX_LEASES,
    GATEWAY_MAX_READ_LINKS, GATEWAY_NEIGHBOURS_FROM, GATEWAY_PUBLISH_POOL,
};

/// The cells of one place: those that contain the point at each channel
/// level (`params::CHANNEL_LEVELS`), and their neighbours from level
/// `GATEWAY_NEIGHBOURS_FROM`. The default served cells of a gateway.
pub fn place_cells(lat: f64, lon: f64) -> Result<Vec<Cell>> {
    let mut out: Vec<Cell> = Vec::new();
    for &level in CHANNEL_LEVELS {
        let Some(cell) = encode(lat, lon, level) else {
            continue;
        };
        let mut around = vec![cell.clone()];
        if level >= GATEWAY_NEIGHBOURS_FROM {
            around.extend(neighbours(&cell)?);
        }
        for c in around {
            let c = Cell::parse(&c)?;
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    Ok(out)
}

/// How the links to the relays are shared between cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// One link per relay, all its cells in one filter: fewer links, but
    /// the relay can link these cells (the gateway's own place).
    PerRelay,
    /// One link per relay and cell (one Tor circuit per cell when the host
    /// uses Tor): the relay cannot link the cells, more links.
    PerCell,
}

/// The limits of a gateway. [`Default`] gives the values of `params`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Links for reading (served cells and leases).
    pub max_read_links: usize,
    /// Links kept open for publishing to cells the gateway does not read.
    pub publish_pool: usize,
    /// How long a requested cell is read, in seconds, unless asked again.
    pub lease_secs: i64,
    /// Requested cells one peer can hold at the same time.
    pub leases_per_peer: usize,
    /// Requested cells in total.
    pub max_leases: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_read_links: GATEWAY_MAX_READ_LINKS,
            publish_pool: GATEWAY_PUBLISH_POOL,
            lease_secs: GATEWAY_LEASE_SECS,
            leases_per_peer: GATEWAY_LEASES_PER_PEER,
            max_leases: GATEWAY_MAX_LEASES,
        }
    }
}

/// One link the host keeps open: a relay, and the cell it is isolated
/// with ([`Isolation::PerCell`]) or `None` ([`Isolation::PerRelay`]).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkKey {
    pub relay: RelayUrl,
    pub cell: Option<Cell>,
}

/// What to read on one link: the cells of its filter (`#g`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub link: LinkKey,
    pub cells: Vec<Cell>,
}

/// The answer to a peer's request for a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granted {
    /// The gateway reads this cell anyway.
    Served,
    /// Read until this Unix time, unless the peer asks again.
    Until(i64),
}

/// Why a request is refused. The gateway tells the peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The peer already holds `leases_per_peer` cells.
    PeerLimit,
    /// The gateway already reads `max_leases` requested cells.
    LeaseLimit,
    /// The cell needs links beyond `max_read_links`.
    LinkLimit,
}

/// Where to publish one event, and the links to open and close first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishPlan {
    /// Every link the event goes to.
    pub to: Vec<LinkKey>,
    /// Links of the pool to open (not open yet).
    pub open: Vec<LinkKey>,
    /// Links of the pool to close: the least recently used ones.
    pub close: Vec<LinkKey>,
}

#[derive(Debug, Clone)]
struct Lease {
    relays: CellRelays,
    /// Peer → end of its lease.
    holders: BTreeMap<String, i64>,
}

impl Lease {
    fn until(&self) -> i64 {
        self.holders.values().copied().max().unwrap_or(0)
    }
}

/// The reading and publishing of a gateway, within its limits.
#[derive(Debug, Clone)]
pub struct Gateway {
    served: Vec<CellRelays>,
    limits: Limits,
    isolation: Isolation,
    leases: BTreeMap<Cell, Lease>,
    /// Pool link → last use.
    pool: BTreeMap<LinkKey, i64>,
}

impl Gateway {
    /// A gateway that reads `served` (for example the relays of
    /// [`place_cells`]). The served cells are read even beyond
    /// `max_read_links`: the limit applies to requests.
    pub fn new(served: Vec<CellRelays>, limits: Limits, isolation: Isolation) -> Gateway {
        Gateway {
            served,
            limits,
            isolation,
            leases: BTreeMap::new(),
            pool: BTreeMap::new(),
        }
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The cells to announce to the mesh: served cells first, then the
    /// requested cells.
    pub fn cells(&self) -> Vec<Cell> {
        let mut out: Vec<Cell> = self.served.iter().map(|c| c.cell.clone()).collect();
        for cell in self.leases.keys() {
            if !out.contains(cell) {
                out.push(cell.clone());
            }
        }
        out
    }

    /// Whether the gateway reads `cell` now.
    pub fn reads(&self, cell: &Cell) -> bool {
        self.is_served(cell) || self.leases.contains_key(cell)
    }

    fn is_served(&self, cell: &Cell) -> bool {
        self.served.iter().any(|c| &c.cell == cell)
    }

    fn key(&self, relay: &RelayUrl, cell: &Cell) -> LinkKey {
        LinkKey {
            relay: relay.clone(),
            cell: match self.isolation {
                Isolation::PerRelay => None,
                Isolation::PerCell => Some(cell.clone()),
            },
        }
    }

    fn read_cells(&self) -> impl Iterator<Item = &CellRelays> {
        self.served
            .iter()
            .chain(self.leases.values().map(|l| &l.relays))
    }

    fn read_links_with(&self, extra: Option<&CellRelays>) -> BTreeSet<LinkKey> {
        self.read_cells()
            .chain(extra)
            .flat_map(|c| c.read().map(move |r| (r, &c.cell)))
            .map(|(r, cell)| self.key(&r.url, cell))
            .collect()
    }

    /// The links for reading, with the cells of each one's filter.
    pub fn subscriptions(&self) -> Vec<Subscription> {
        let mut by_link: BTreeMap<LinkKey, Vec<Cell>> = BTreeMap::new();
        for c in self.read_cells() {
            for r in c.read() {
                let cells = by_link.entry(self.key(&r.url, &c.cell)).or_default();
                if !cells.contains(&c.cell) {
                    cells.push(c.cell.clone());
                }
            }
        }
        by_link
            .into_iter()
            .map(|(link, cells)| Subscription { link, cells })
            .collect()
    }

    /// How many links reading needs now.
    pub fn read_links(&self) -> usize {
        self.read_links_with(None).len()
    }

    /// The links open now: reading and the publishing pool.
    pub fn links(&self) -> usize {
        let read = self.read_links_with(None);
        read.len() + self.pool.keys().filter(|k| !read.contains(k)).count()
    }

    /// A peer asks the gateway to read a cell. `relays` are the relays of
    /// that cell. Asking again extends the lease.
    pub fn request(
        &mut self,
        peer: &str,
        relays: CellRelays,
        now: i64,
    ) -> std::result::Result<Granted, Refused> {
        self.expire(now);
        let cell = relays.cell.clone();
        if self.is_served(&cell) {
            return Ok(Granted::Served);
        }
        let until = now + self.limits.lease_secs;
        if let Some(lease) = self.leases.get_mut(&cell)
            && lease.holders.contains_key(peer)
        {
            lease.holders.insert(peer.to_owned(), until);
            return Ok(Granted::Until(until));
        }
        let held = self
            .leases
            .values()
            .filter(|l| l.holders.contains_key(peer))
            .count();
        if held >= self.limits.leases_per_peer {
            return Err(Refused::PeerLimit);
        }
        if let Some(lease) = self.leases.get_mut(&cell) {
            lease.holders.insert(peer.to_owned(), until);
            return Ok(Granted::Until(until));
        }
        if self.leases.len() >= self.limits.max_leases {
            return Err(Refused::LeaseLimit);
        }
        if self.read_links_with(Some(&relays)).len() > self.limits.max_read_links {
            return Err(Refused::LinkLimit);
        }
        let mut holders = BTreeMap::new();
        holders.insert(peer.to_owned(), until);
        self.leases.insert(cell, Lease { relays, holders });
        Ok(Granted::Until(until))
    }

    /// Ends the leases that are over. Returns the cells no longer read.
    pub fn expire(&mut self, now: i64) -> Vec<Cell> {
        let mut ended = Vec::new();
        for lease in self.leases.values_mut() {
            lease.holders.retain(|_, until| *until > now);
        }
        self.leases.retain(|cell, lease| {
            let keep = !lease.holders.is_empty();
            if !keep {
                ended.push(cell.clone());
            }
            keep
        });
        ended
    }

    /// When the next lease ends, if any.
    pub fn next_expiry(&self) -> Option<i64> {
        self.leases.values().map(Lease::until).min()
    }

    /// Where to publish an event of the cell of `relays` (any cell). A
    /// relay the gateway reads takes it on its reading link; the others go
    /// through the pool.
    pub fn publish(&mut self, relays: &CellRelays, now: i64) -> PublishPlan {
        let read = self.read_links_with(None);
        let mut plan = PublishPlan::default();
        for r in relays.write() {
            let key = self.key(&r.url, &relays.cell);
            if !plan.to.contains(&key) {
                plan.to.push(key.clone());
            }
            if read.contains(&key) {
                continue;
            }
            if self.pool.insert(key.clone(), now).is_none() {
                plan.open.push(key);
            }
        }
        while self.pool.len() > self.limits.publish_pool {
            let Some(oldest) = self
                .pool
                .iter()
                .filter(|(k, _)| !plan.to.contains(k))
                .min_by_key(|(_, used)| **used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.pool.remove(&oldest);
            plan.close.push(oldest);
        }
        plan
    }

    /// Links of the pool to close after `publish` sent its event, when the
    /// event needed more links than the pool holds.
    pub fn trim_pool(&mut self) -> Vec<LinkKey> {
        let mut close = Vec::new();
        while self.pool.len() > self.limits.publish_pool {
            let Some(oldest) = self
                .pool
                .iter()
                .min_by_key(|(_, used)| **used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.pool.remove(&oldest);
            close.push(oldest);
        }
        close
    }
}

#[cfg(all(test, feature = "airhop"))]
mod tests {
    use super::*;
    use crate::relays::{Relay, Role, Strategy};

    fn cell_relays(cell: &str, relays: &[&str]) -> CellRelays {
        CellRelays {
            cell: Cell::parse(cell).unwrap(),
            strategy: Strategy::AirHop,
            relays: relays
                .iter()
                .enumerate()
                .map(|(i, r)| Relay {
                    url: RelayUrl::parse(r).unwrap(),
                    place: None,
                    rank: i + 1,
                    role: Role::ReadWrite,
                })
                .collect(),
        }
    }

    fn limits() -> Limits {
        Limits {
            max_read_links: 4,
            publish_pool: 2,
            lease_secs: 100,
            leases_per_peer: 1,
            max_leases: 2,
        }
    }

    #[test]
    fn place_cells_are_the_levels_and_their_neighbours() {
        let cells = place_cells(48.8566, 2.3522).unwrap();
        // 2 and 4 alone, 5 to 8 with 8 neighbours each.
        assert_eq!(cells.len(), 2 + 4 * 9);
        assert_eq!(cells[0].as_str(), "u0");
        assert!(cells.iter().any(|c| c.as_str() == "u09tv"));
    }

    #[test]
    fn one_link_per_relay_holds_all_its_cells() {
        let g = Gateway::new(
            vec![
                cell_relays("u09", &["a.example", "b.example"]),
                cell_relays("u09t", &["b.example"]),
            ],
            limits(),
            Isolation::PerRelay,
        );
        let subs = g.subscriptions();
        assert_eq!(subs.len(), 2);
        let b = subs
            .iter()
            .find(|s| s.link.relay.as_str() == "b.example")
            .unwrap();
        assert_eq!(b.cells.len(), 2);
        assert_eq!(g.read_links(), 2);
    }

    #[test]
    fn per_cell_isolation_opens_one_link_per_relay_and_cell() {
        let g = Gateway::new(
            vec![
                cell_relays("u09", &["a.example", "b.example"]),
                cell_relays("u09t", &["b.example"]),
            ],
            limits(),
            Isolation::PerCell,
        );
        assert_eq!(g.read_links(), 3);
        assert!(g.subscriptions().iter().all(|s| s.cells.len() == 1));
    }

    #[test]
    fn requests_are_bounded_and_expire() {
        let mut g = Gateway::new(
            vec![cell_relays("u09", &["a.example", "b.example"])],
            limits(),
            Isolation::PerRelay,
        );
        assert_eq!(
            g.request("p1", cell_relays("u09", &["a.example"]), 0),
            Ok(Granted::Served)
        );
        assert_eq!(
            g.request("p1", cell_relays("9q8", &["c.example"]), 0),
            Ok(Granted::Until(100))
        );
        // Asking again extends it, and does not count twice.
        assert_eq!(
            g.request("p1", cell_relays("9q8", &["c.example"]), 50),
            Ok(Granted::Until(150))
        );
        assert_eq!(
            g.request("p1", cell_relays("dr5", &["d.example"]), 50),
            Err(Refused::PeerLimit)
        );
        // Three new relays would pass 4 links.
        assert_eq!(
            g.request(
                "p2",
                cell_relays("dr5", &["d.example", "e.example", "f.example"]),
                50
            ),
            Err(Refused::LinkLimit)
        );
        assert_eq!(
            g.request("p2", cell_relays("dr5", &["d.example"]), 50),
            Ok(Granted::Until(150))
        );
        assert_eq!(
            g.request("p3", cell_relays("s00", &["a.example"]), 50),
            Err(Refused::LeaseLimit)
        );
        assert_eq!(g.read_links(), 4);
        assert_eq!(g.next_expiry(), Some(150));
        let ended = g.expire(150);
        assert_eq!(ended.len(), 2);
        assert_eq!(g.read_links(), 2);
        assert_eq!(g.cells().len(), 1);
    }

    #[test]
    fn publishing_uses_reading_links_then_a_bounded_pool() {
        let mut g = Gateway::new(
            vec![cell_relays("u09", &["a.example"])],
            limits(),
            Isolation::PerRelay,
        );
        // a is read: no new link. b and c go to the pool.
        let plan = g.publish(
            &cell_relays("u0", &["a.example", "b.example", "c.example"]),
            1,
        );
        assert_eq!(plan.to.len(), 3);
        assert_eq!(plan.open.len(), 2);
        assert!(plan.close.is_empty());
        assert_eq!(g.links(), 3);
        // d is new: the least recently used of the pool closes.
        let plan = g.publish(&cell_relays("9q", &["c.example", "d.example"]), 2);
        assert_eq!(plan.open.len(), 1);
        assert_eq!(plan.close.len(), 1);
        assert_eq!(plan.close[0].relay.as_str(), "b.example");
        assert_eq!(g.links(), 3);
        // An event that needs more links than the pool: closed after it.
        let plan = g.publish(
            &cell_relays("dr", &["e.example", "f.example", "h.example"]),
            3,
        );
        assert_eq!(plan.open.len(), 3);
        assert_eq!(g.trim_pool().len(), 1);
        assert_eq!(g.links(), 3);
    }
}
