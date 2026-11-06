// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The entry point's core, without I/O: what to fetch and check, what to
//! keep, and the choice for a cell. The async and blocking front ends run
//! its requests with the host's means to talk; a port to another language
//! ports this file and keeps the front end of its platform.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::Strategy;
use super::files::{HEALTH, LIST_PREFIX};
use super::types::{Authority, Cell, CellRelays, Extra, Relay, RelayUrl, Role, Seen};
use crate::directory::{Platform, RelayDirectory};
use crate::error::{Error, Result};
use crate::params::{
    A0_HEALTH_TTL_SECS, HOME_NEAREST, LIST_MAX_AGE_SECS, LIST_RETRY_SECS, QUALIFY_EVERY_SECS,
};
use crate::select::haversine_km;
use crate::strategy::{GeoRelay, Health, RelayHealth};

/// What the client chose when building the entry point.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub strategy: Strategy,
    pub target: Option<Strategy>,
    pub others: Vec<Strategy>,
    pub home: Option<(Cell, usize)>,
    pub authorities: Vec<Authority>,
    pub extra: Vec<RelayUrl>,
    pub extra_mode: Extra,
    pub qualify: bool,
}

#[derive(Debug, Clone)]
struct Kept {
    fetched_at: i64,
    dir: RelayDirectory,
}

#[derive(Debug, Clone)]
struct Watched {
    /// The cell it was chosen for: its connection is isolated with that
    /// cell's.
    cell: Cell,
    checked: Option<i64>,
}

/// A relay to check: its NIP-11 document, and whether it can be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Check {
    pub relay: RelayUrl,
    pub cell: Cell,
}

#[derive(Debug)]
pub(crate) struct Engine {
    pub cfg: Config,
    /// By authority URL.
    lists: BTreeMap<String, Kept>,
    /// Last failed download, by authority URL.
    failed: BTreeMap<String, i64>,
    health: BTreeMap<RelayUrl, (Seen, i64)>,
    watched: BTreeMap<RelayUrl, Watched>,
    restored: bool,
}

impl Engine {
    pub fn new(strategy: Strategy) -> Engine {
        Engine {
            cfg: Config {
                strategy,
                target: None,
                others: Vec::new(),
                home: None,
                authorities: Authority::defaults(),
                extra: Vec::new(),
                extra_mode: Extra::Append,
                qualify: true,
            },
            lists: BTreeMap::new(),
            failed: BTreeMap::new(),
            health: BTreeMap::new(),
            watched: BTreeMap::new(),
            restored: false,
        }
    }

    // ------------------------------------------------------------ kept files

    /// The file a list from `authority` is kept in.
    pub fn list_file(authority: &str) -> String {
        let digest = Sha256::digest(authority.as_bytes());
        let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        format!("{LIST_PREFIX}{hex}")
    }

    pub fn restored(&self) -> bool {
        self.restored
    }

    /// The files to read once, before anything else.
    pub fn files_to_restore(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .cfg
            .authorities
            .iter()
            .map(|a| Engine::list_file(&a.url))
            .collect();
        names.push(HEALTH.to_owned());
        names
    }

    /// A kept file, as read. An unreadable one is ignored: the list is
    /// fetched again.
    pub fn restore(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error::Unreadable(name.to_owned()))?;
        if name == HEALTH {
            for line in text.lines() {
                let mut f = line.split('\t');
                let (Some(url), Some(seen), Some(since)) = (f.next(), f.next(), f.next()) else {
                    continue;
                };
                let (Ok(url), Some(seen), Ok(since)) =
                    (RelayUrl::parse(url), seen_of(seen), since.parse::<i64>())
                else {
                    continue;
                };
                self.health.insert(url, (seen, since));
            }
            return Ok(());
        }
        let Some(authority) = self
            .cfg
            .authorities
            .iter()
            .find(|a| Engine::list_file(&a.url) == name)
            .cloned()
        else {
            return Ok(());
        };
        let (at, csv) = text
            .split_once('\n')
            .ok_or_else(|| Error::Unreadable(name.to_owned()))?;
        let at: i64 = at.parse().map_err(|_| Error::Unreadable(name.to_owned()))?;
        let dir = RelayDirectory::parse(csv, authority.platform);
        if !dir.is_empty() {
            self.lists.insert(
                authority.url,
                Kept {
                    fetched_at: at,
                    dir,
                },
            );
        }
        Ok(())
    }

    pub fn mark_restored(&mut self) {
        self.restored = true;
    }

    pub fn health_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        for (url, (seen, since)) in &self.health {
            out.push_str(&format!(
                "{}\t{}\t{since}\n",
                url.as_str(),
                seen_name(*seen)
            ));
        }
        out.into_bytes()
    }

    // ------------------------------------------------------------ lists

    pub fn has_list(&self) -> bool {
        !self.lists.is_empty()
    }

    /// The lists to download now: missing or older than a day, not failed
    /// within the hour; all of them when `force`.
    pub fn due_lists(&self, now: i64, force: bool) -> Vec<Authority> {
        self.cfg
            .authorities
            .iter()
            .filter(|a| {
                if force {
                    return true;
                }
                let stale = self
                    .lists
                    .get(&a.url)
                    .is_none_or(|k| now - k.fetched_at >= LIST_MAX_AGE_SECS);
                let resting = self
                    .failed
                    .get(&a.url)
                    .is_some_and(|t| now - t < LIST_RETRY_SECS);
                stale && !resting
            })
            .cloned()
            .collect()
    }

    /// A downloaded list: read it, keep it; the bytes to store. An empty or
    /// unreadable list does not replace the one kept.
    pub fn ingest_list(&mut self, authority: &Authority, body: &[u8], now: i64) -> Result<Vec<u8>> {
        let unreadable = || Error::Unreadable(format!("list {}", authority.url));
        let text = std::str::from_utf8(body).map_err(|_| unreadable())?;
        let text = match &self.cfg.home {
            Some((region, min)) if !self.needs_whole_list() => keep_region(text, region, *min)?,
            _ => text
                .lines()
                .map(strip_scheme)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        let dir = RelayDirectory::parse(&text, authority.platform);
        if dir.is_empty() {
            return Err(unreadable());
        }
        self.lists.insert(
            authority.url.clone(),
            Kept {
                fetched_at: now,
                dir,
            },
        );
        self.failed.remove(&authority.url);
        Ok(format!("{now}\n{text}\n").into_bytes())
    }

    pub fn list_failed(&mut self, authority: &Authority, now: i64) {
        self.failed.insert(authority.url.clone(), now);
    }

    fn needs_whole_list(&self) -> bool {
        std::iter::once(&self.cfg.strategy)
            .chain(&self.cfg.target)
            .chain(&self.cfg.others)
            .any(Strategy::needs_whole_list)
    }

    /// The first kept list of each platform, in the order of the
    /// authorities.
    fn dirs(&self) -> (Option<&RelayDirectory>, Option<&RelayDirectory>) {
        let of = |p: Platform| {
            self.cfg
                .authorities
                .iter()
                .filter(|a| a.platform == p)
                .find_map(|a| self.lists.get(&a.url))
                .map(|k| &k.dir)
        };
        (of(Platform::Ios), of(Platform::Android))
    }

    // ------------------------------------------------------------ health

    pub fn report(&mut self, relay: &RelayUrl, seen: Seen, now: i64) {
        match seen {
            Seen::Fine => {
                self.health.remove(relay);
            }
            _ => {
                self.health.insert(relay.clone(), (seen, now));
            }
        }
    }

    /// What A0 avoids now: dead relays for 10 min, refusals until the next
    /// check.
    #[cfg_attr(not(feature = "a0"), allow(dead_code))]
    fn relay_health(&self, now: i64) -> RelayHealth {
        self.health
            .iter()
            .filter_map(|(url, (seen, since))| {
                let (health, ttl) = match seen {
                    Seen::Dead => (Health::Dead, A0_HEALTH_TTL_SECS),
                    Seen::AuthRefused => (Health::AuthRequired, QUALIFY_EVERY_SECS),
                    Seen::EventRefused => (Health::WriteRefused, QUALIFY_EVERY_SECS),
                    Seen::Fine => return None,
                };
                (now - since < ttl).then(|| (url.url(), health))
            })
            .collect()
    }

    /// The relays to check now (each once per [`QUALIFY_EVERY_SECS`]).
    pub fn due_checks(&mut self, now: i64, force: bool) -> Vec<Check> {
        if !self.cfg.qualify {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (relay, w) in &mut self.watched {
            if force || w.checked.is_none_or(|t| now - t >= QUALIFY_EVERY_SECS) {
                w.checked = Some(now);
                out.push(Check {
                    relay: relay.clone(),
                    cell: w.cell.clone(),
                });
            }
        }
        out
    }

    /// The result of a check: the NIP-11 document (if any), and whether a
    /// connection opened.
    pub fn checked(&mut self, relay: &RelayUrl, nip11: Option<&[u8]>, reachable: bool, now: i64) {
        if !reachable {
            self.report(relay, Seen::Dead, now);
            return;
        }
        if self
            .health
            .get(relay)
            .is_some_and(|(s, _)| *s == Seen::Dead)
        {
            self.health.remove(relay);
        }
        let declared = nip11
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(crate::a6::Nip11::parse);
        let verdict = crate::a6::Verdict::of(declared.as_ref(), None);
        let refusal = verdict.refusals.iter().find_map(|f| match f {
            crate::a6::Finding::AuthRequired => Some(Seen::AuthRefused),
            crate::a6::Finding::Payment
            | crate::a6::Finding::RestrictedWrites
            | crate::a6::Finding::Pow(_) => Some(Seen::EventRefused),
            _ => None,
        });
        if let Some(seen) = refusal {
            self.report(relay, seen, now);
        }
    }

    // ------------------------------------------------------------ choice

    /// The relays of `cell`: for the strategy in use (and the migration
    /// target), or for `strategy` when given.
    pub fn get(
        &mut self,
        strategy: Option<&Strategy>,
        cell: &Cell,
        now: i64,
    ) -> Result<CellRelays> {
        let main = strategy.unwrap_or(&self.cfg.strategy).clone();
        let configured = std::iter::once(&self.cfg.strategy)
            .chain(&self.cfg.target)
            .chain(&self.cfg.others)
            .any(|s| *s == main);
        if !configured {
            return Err(Error::StrategyNotConfigured(main.name().to_owned()));
        }
        let mut chosen = Vec::new();
        if !(self.cfg.extra_mode == Extra::Replace && !self.cfg.extra.is_empty()) {
            self.choose(&main, cell, now, &mut chosen)?;
            if strategy.is_none()
                && let Some(target) = self.cfg.target.clone()
            {
                self.choose(&target, cell, now, &mut chosen)?;
            }
        }
        for url in &self.cfg.extra {
            mark(&mut chosen, url.clone(), None, true, true);
        }
        let relays: Vec<Relay> = chosen
            .into_iter()
            .enumerate()
            .map(|(i, (url, place, r, w))| Relay {
                url,
                place,
                rank: i + 1,
                role: match (r, w) {
                    (true, true) => Role::ReadWrite,
                    (true, false) => Role::ReadOnly,
                    _ => Role::WriteOnly,
                },
            })
            .collect();
        if self.cfg.qualify {
            for r in &relays {
                self.watched.entry(r.url.clone()).or_insert(Watched {
                    cell: cell.clone(),
                    checked: None,
                });
            }
        }
        Ok(CellRelays {
            cell: cell.clone(),
            strategy: main,
            relays,
        })
    }

    fn choose(&self, s: &Strategy, cell: &Cell, now: i64, out: &mut Vec<Chosen>) -> Result<()> {
        let (ios, android) = self.dirs();
        let shared = android.or(ios);
        let strategy: Box<dyn crate::Strategy> = match s {
            #[cfg(feature = "apps")]
            Strategy::Apps(apps) => {
                if ios.is_none() && android.is_none() {
                    return Err(Error::NoRelayList);
                }
                Box::new(crate::AppStrategy::new(
                    ios.cloned(),
                    android.cloned(),
                    *apps,
                ))
            }
            #[cfg(feature = "airhop")]
            Strategy::AirHop => Box::new(crate::AirhopStrategy::from_directory(
                shared.ok_or(Error::NoRelayList)?,
            )),
            #[cfg(feature = "a0")]
            Strategy::A0 => Box::new(
                crate::a0::SharedStrategy::from_directory(
                    shared.ok_or(Error::NoRelayList)?.clone(),
                )
                .with_health(self.relay_health(now)),
            ),
            #[cfg(feature = "a1")]
            Strategy::A1(salts) => Box::new(
                crate::a1::HrwStrategy::new(shared.ok_or(Error::NoRelayList)?)
                    .with_salts(salts.clone(), now),
            ),
        };
        let _ = (shared, now);
        for s in strategy.read(cell.as_str())? {
            add(out, &s.relay, true, false);
        }
        for s in strategy.write(cell.as_str())? {
            add(out, &s.relay, false, true);
        }
        Ok(())
    }
}

/// A relay of a cell while the choice is built: address, place, read,
/// written.
type Chosen = (RelayUrl, Option<(f64, f64)>, bool, bool);

fn add(out: &mut Vec<Chosen>, relay: &GeoRelay, read: bool, write: bool) {
    let Ok(url) = RelayUrl::parse(&relay.url) else {
        return;
    };
    let place = relay.km.is_finite().then_some((relay.lat, relay.lon));
    mark(out, url, place, read, write);
}

fn mark(out: &mut Vec<Chosen>, url: RelayUrl, place: Option<(f64, f64)>, read: bool, write: bool) {
    match out.iter_mut().find(|c| c.0 == url) {
        Some(c) => {
            c.2 |= read;
            c.3 |= write;
        }
        None => out.push((url, place, read, write)),
    }
}

fn seen_name(seen: Seen) -> &'static str {
    match seen {
        Seen::Dead => "dead",
        Seen::AuthRefused => "auth",
        Seen::EventRefused => "refused",
        Seen::Fine => "fine",
    }
}

fn seen_of(name: &str) -> Option<Seen> {
    match name {
        "dead" => Some(Seen::Dead),
        "auth" => Some(Seen::AuthRefused),
        "refused" => Some(Seen::EventRefused),
        _ => None,
    }
}

/// A list line without its implied `wss://`.
fn strip_scheme(line: &str) -> &str {
    match line.get(..6) {
        Some(s) if s.eq_ignore_ascii_case("wss://") => &line[6..],
        _ => line,
    }
}

/// The place of a list line (`url,lat,lon`), if it has one.
fn place_of(line: &str) -> Option<(f64, f64)> {
    let mut f = line.split(',').map(str::trim);
    f.next()?;
    let lat: f64 = f.next()?.parse().ok()?;
    let lon: f64 = f.next()?.parse().ok()?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// The lines of a list a region needs, in their order, without their
/// scheme.
///
/// Any point of the region is within `r` of its centre. The `k` relays
/// nearest the centre are within `d_k` of it, so within `d_k + r` of any
/// point: every point has its `k` nearest within that distance, and they
/// are all within `d_k + 2r` of the centre. Keeping those keeps, for every
/// cell of the region, the same `k` nearest as the whole list.
fn keep_region(text: &str, region: &Cell, min: usize) -> Result<String> {
    let [[s, w], [n, e]] = crate::geohash::bbox(region.as_str())?;
    let (clat, clon) = ((s + n) / 2.0, (w + e) / 2.0);
    let r = [
        (s, w),
        (s, e),
        (n, w),
        (n, e),
        (s, clon),
        (n, clon),
        (clat, w),
        (clat, e),
    ]
    .iter()
    .map(|(lat, lon)| haversine_km(clat, clon, *lat, *lon))
    .fold(0.0_f64, f64::max);
    let lines: Vec<(&str, Option<f64>)> = text
        .lines()
        .map(|l| {
            (
                l,
                place_of(l).map(|(lat, lon)| haversine_km(clat, clon, lat, lon)),
            )
        })
        .collect();
    let mut dists: Vec<f64> = lines.iter().filter_map(|(_, d)| *d).collect();
    dists.sort_by(f64::total_cmp);
    let k = min.max(HOME_NEAREST);
    let limit = match dists.get(k.saturating_sub(1)) {
        Some(dk) => dk + 2.0 * r + 1e-6,
        None => f64::INFINITY,
    };
    let kept: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, (_, d))| match d {
            Some(d) => *d <= limit,
            // The header.
            None => *i == 0,
        })
        .map(|(_, (l, _))| strip_scheme(l))
        .collect();
    Ok(kept.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_are_implied() {
        assert_eq!(strip_scheme("wss://a.example,1,2"), "a.example,1,2");
        assert_eq!(strip_scheme("WSS://a.example,1,2"), "a.example,1,2");
        assert_eq!(strip_scheme("ws://a.example,1,2"), "ws://a.example,1,2");
    }

    #[test]
    fn list_files_name_their_authority() {
        let a = Engine::list_file("https://a.example/list.csv");
        assert!(a.starts_with(LIST_PREFIX));
        assert_eq!(a.len(), LIST_PREFIX.len() + 16);
        assert_ne!(a, Engine::list_file("https://b.example/list.csv"));
    }
}
