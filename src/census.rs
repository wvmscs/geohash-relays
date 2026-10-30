// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Census of relays (lot 13, G15): the relays users name in their relay
//! lists (NIP-65, kind 10002), read from the relays already known. A round
//! reads every relay given; the relays its lists name and that were not
//! known make the next round, until none is new (the caller runs the rounds,
//! [`crate::params::CENSUS_MAX_ROUNDS`] at most).
//!
//! * [`relay_url`]: a relay address from free text, checked and normalised;
//! * [`relays_of`]: the relays of a NIP-65 list;
//! * [`Crawler`]: the machine of a round ([`crate::sansio`]): subscribes to
//!   the lists on each relay, [`crate::params::CENSUS_CONCURRENCY`] at a
//!   time, until `EOSE`, a failure or the time limit.
//!
//! What a relay is worth for the apps (NIP-11, probes) is [`crate::a6`]'s.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::IpAddr;

use crate::directory::normalize_url;
use crate::params::{
    CENSUS_CONCURRENCY, CENSUS_LIST_LIMIT, CENSUS_MAX_FUTURE_SECS, CENSUS_MAX_URL_BYTES,
    CENSUS_TIMEOUT_SECS, KIND_RELAY_LIST,
};
use crate::sansio::{Action, EventMeta, FilterSpec, Input, Machine};
use crate::strategy::Health;

/// A relay address from free text (a NIP-65 tag, a command line): a
/// `wss://` or `ws://` URL whose host is a public name or address, without
/// user, query nor fragment; normalised as the apps key servers
/// ([`normalize_url`]). `None` for anything else: local or private
/// addresses, `localhost`, malformed names.
pub fn relay_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty()
        || raw.len() > CENSUS_MAX_URL_BYTES
        || raw.contains(|c: char| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("wss://")
        .or_else(|| lower.strip_prefix("ws://"))?;
    if rest.contains(['@', '?', '#', '\\']) {
        return None;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let host = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6.split_once(']')?;
        if !(after.is_empty() || port_ok(after.strip_prefix(':')?)) {
            return None;
        }
        h
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) if port_ok(p) => h,
            Some(_) => return None,
            None => authority,
        }
    };
    if !public_host(host) {
        return None;
    }
    normalize_url(raw)
}

fn port_ok(p: &str) -> bool {
    p.parse::<u16>().is_ok_and(|p| p != 0)
}

/// A host a relay can be reached at from the Internet (or through Tor:
/// `.onion`): a public address, or a name of letters, digits and hyphens
/// with a top-level domain.
fn public_host(host: &str) -> bool {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                !(v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                    || v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
            }
            IpAddr::V6(v6) => {
                let first = v6.segments()[0];
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80)
            }
        };
    }
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && host.len() <= 253
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.chars().all(|c| c.is_ascii_alphabetic()))
        && !matches!(
            *labels.last().unwrap_or(&""),
            "local" | "localhost" | "internal" | "lan"
        )
}

/// The relays of a NIP-65 list (`r` tags, read or write alike), checked and
/// normalised, in order, once each. Empty for another kind.
pub fn relays_of(event: &EventMeta) -> Vec<String> {
    if event.kind != KIND_RELAY_LIST {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    event
        .tag_values("r")
        .filter_map(relay_url)
        .filter(|u| seen.insert(u.clone()))
        .collect()
}

/// A relay list, as recorded: the newest one of its author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayList {
    pub id: String,
    /// Its author: a list replaces the older ones of the same author.
    pub pubkey: String,
    pub created_at: i64,
    pub relays: Vec<String>,
    /// The relay it was read from.
    pub read_from: String,
}

/// How reading a relay ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ended its stored events (`EOSE`), with this many valid lists.
    Answered { lists: usize },
    /// It could not be used.
    Failed(Health),
    /// Nothing ended within [`CENSUS_TIMEOUT_SECS`].
    Silent,
}

/// The machine of a census round.
pub struct Crawler {
    queue: VecDeque<String>,
    /// Relays being read: since when, valid lists so far.
    reading: BTreeMap<String, (i64, usize)>,
    outcomes: BTreeMap<String, Outcome>,
    lists: BTreeMap<String, RelayList>,
    now: i64,
}

impl Crawler {
    /// A round over `relays` (normalised, once each).
    pub fn new(relays: impl IntoIterator<Item = String>) -> Self {
        let mut seen = BTreeSet::new();
        let queue = relays
            .into_iter()
            .map(|r| normalize_url(&r).unwrap_or(r))
            .filter(|r| seen.insert(r.clone()))
            .collect();
        Crawler {
            queue,
            reading: BTreeMap::new(),
            outcomes: BTreeMap::new(),
            lists: BTreeMap::new(),
            now: 0,
        }
    }

    /// The subscription: the newest relay lists.
    pub fn filter() -> FilterSpec {
        FilterSpec {
            kinds: vec![KIND_RELAY_LIST],
            limit: Some(CENSUS_LIST_LIMIT),
            ..FilterSpec::default()
        }
    }

    /// Every relay has answered, failed or been left.
    pub fn done(&self) -> bool {
        self.queue.is_empty() && self.reading.is_empty()
    }

    /// How each relay read so far ended.
    pub fn outcomes(&self) -> &BTreeMap<String, Outcome> {
        &self.outcomes
    }

    /// The newest valid list of each author seen.
    pub fn lists(&self) -> impl Iterator<Item = &RelayList> {
        self.lists.values()
    }

    /// Every relay the lists name.
    pub fn named(&self) -> BTreeSet<String> {
        self.lists
            .values()
            .flat_map(|l| l.relays.iter().cloned())
            .collect()
    }

    fn next(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        while self.reading.len() < CENSUS_CONCURRENCY
            && let Some(relay) = self.queue.pop_front()
        {
            self.reading.insert(relay.clone(), (self.now, 0));
            out.push(Action::Subscribe {
                relay,
                filter: Crawler::filter(),
            });
        }
        out
    }

    fn finish(&mut self, relay: &str, outcome: impl FnOnce(usize) -> Outcome) -> Vec<Action> {
        let Some((_, lists)) = self.reading.remove(relay) else {
            return Vec::new();
        };
        self.outcomes.insert(relay.to_owned(), outcome(lists));
        let mut out = vec![Action::Unsubscribe {
            relay: relay.to_owned(),
        }];
        out.extend(self.next());
        out
    }

    /// A list read from `relay`: valid when it is a NIP-65 list, not dated in
    /// the future, naming at least one relay; it replaces an older one of
    /// the same author.
    fn list(&mut self, relay: &str, event: EventMeta) {
        let Some((_, count)) = self.reading.get_mut(relay) else {
            return;
        };
        if event.kind != KIND_RELAY_LIST || event.created_at > self.now + CENSUS_MAX_FUTURE_SECS {
            return;
        }
        let relays = relays_of(&event);
        if relays.is_empty() {
            return;
        }
        *count += 1;
        let newer = self
            .lists
            .get(&event.pubkey)
            .is_none_or(|l| (event.created_at, &event.id) > (l.created_at, &l.id));
        if newer {
            self.lists.insert(
                event.pubkey.clone(),
                RelayList {
                    id: event.id,
                    pubkey: event.pubkey,
                    created_at: event.created_at,
                    relays,
                    read_from: relay.to_owned(),
                },
            );
        }
    }
}

impl Machine for Crawler {
    fn start(&mut self, now: i64) -> Vec<Action> {
        self.now = now;
        self.next()
    }

    fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Event { relay, event } => {
                self.list(&relay, event);
                Vec::new()
            }
            Input::Eose { relay } => self.finish(&relay, |lists| Outcome::Answered { lists }),
            Input::Failed { relay, health } => self.finish(&relay, |_| Outcome::Failed(health)),
            Input::Tick { now } => {
                self.now = now;
                let late: Vec<String> = self
                    .reading
                    .iter()
                    .filter(|(_, (since, _))| now - since > CENSUS_TIMEOUT_SECS)
                    .map(|(r, _)| r.clone())
                    .collect();
                late.iter()
                    .flat_map(|r| self.finish(r, |_| Outcome::Silent))
                    .collect()
            }
            Input::Ok { .. } | Input::Nip11 { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(id: &str, pubkey: &str, created_at: i64, relays: &[&str]) -> EventMeta {
        EventMeta {
            id: id.into(),
            pubkey: pubkey.into(),
            kind: KIND_RELAY_LIST,
            created_at,
            tags: relays
                .iter()
                .map(|r| vec!["r".into(), (*r).into(), "write".into()])
                .collect(),
            raw: String::new(),
        }
    }

    #[test]
    fn relay_addresses() {
        assert_eq!(
            relay_url(" WSS://Relay.Example.org/ ").as_deref(),
            Some("wss://relay.example.org")
        );
        assert_eq!(
            relay_url("wss://relay.example.org:443").as_deref(),
            Some("wss://relay.example.org")
        );
        assert_eq!(
            relay_url("wss://relay.example.org:7777/nostr").as_deref(),
            Some("wss://relay.example.org:7777/nostr")
        );
        assert_eq!(
            relay_url("ws://relay.example.org").as_deref(),
            Some("ws://relay.example.org")
        );
        assert_eq!(
            relay_url("wss://abcdefghijklmnop.onion").as_deref(),
            Some("wss://abcdefghijklmnop.onion")
        );
        assert_eq!(relay_url("wss://8.8.8.8").as_deref(), Some("wss://8.8.8.8"));
        for bad in [
            "relay.example.org",
            "https://relay.example.org",
            "wss://localhost",
            "wss://relay",
            "wss://127.0.0.1",
            "wss://192.168.1.2:7777",
            "wss://10.0.0.1",
            "wss://100.64.1.1",
            "wss://[::1]",
            "wss://[fd00::1]",
            "wss://relay.local",
            "wss://user@relay.example.org",
            "wss://relay.example.org/?x=1",
            "wss://relay example.org",
            "wss://relay.example.org:0",
            "wss://relay.example.org:99999",
            "wss://-relay.example.org",
            "wss://relay.example.123",
            "",
        ] {
            assert_eq!(relay_url(bad), None, "{bad}");
        }
        assert_eq!(relay_url(&format!("wss://{}.org", "a".repeat(300))), None);
    }

    #[test]
    fn lists_name_relays_once() {
        let e = list(
            "1",
            "p",
            10,
            &[
                "wss://a.example.org",
                "wss://A.example.org/",
                "nonsense",
                "wss://b.example.org",
            ],
        );
        assert_eq!(
            relays_of(&e),
            ["wss://a.example.org", "wss://b.example.org"]
        );
        let other = EventMeta { kind: 1, ..e };
        assert!(relays_of(&other).is_empty());
    }

    fn subscribed(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Subscribe { relay, filter } => {
                    assert_eq!(filter, &Crawler::filter());
                    Some(relay.clone())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_round() {
        let relays: Vec<String> = (0..CENSUS_CONCURRENCY + 2)
            .map(|i| format!("wss://r{i}.example.org"))
            .collect();
        let mut c = Crawler::new(relays.clone());
        let first = c.start(1_000);
        assert_eq!(subscribed(&first).len(), CENSUS_CONCURRENCY);
        let r0 = relays[0].clone();
        // Two lists of one author: the newest is kept; one from the future,
        // one of another kind, one without relays: not valid.
        for e in [
            list("a", "alice", 900, &["wss://old.example.org"]),
            list(
                "b",
                "alice",
                950,
                &["wss://new.example.org", "wss://r1.example.org"],
            ),
            list(
                "c",
                "bob",
                1_000 + CENSUS_MAX_FUTURE_SECS + 1,
                &["wss://future.example.org"],
            ),
            list("d", "carol", 900, &["localhost"]),
            EventMeta {
                kind: 1,
                ..list("e", "dave", 900, &["wss://note.example.org"])
            },
        ] {
            assert!(
                c.handle(Input::Event {
                    relay: r0.clone(),
                    event: e
                })
                .is_empty()
            );
        }
        let after = c.handle(Input::Eose { relay: r0.clone() });
        assert_eq!(after[0], Action::Unsubscribe { relay: r0.clone() });
        assert_eq!(subscribed(&after), [relays[CENSUS_CONCURRENCY].clone()]);
        assert_eq!(c.outcomes()[&r0], Outcome::Answered { lists: 2 });
        assert_eq!(
            c.named(),
            BTreeSet::from([
                "wss://new.example.org".into(),
                "wss://r1.example.org".into()
            ])
        );
        assert_eq!(c.lists().next().unwrap().read_from, r0);
        // An event from a relay not being read is ignored.
        c.handle(Input::Event {
            relay: "wss://other.example.org".into(),
            event: list("f", "erin", 900, &["wss://x.example.org"]),
        });
        assert_eq!(c.lists().count(), 1);
        // A failure, then the others left after the time limit.
        let r1 = relays[1].clone();
        c.handle(Input::Failed {
            relay: r1.clone(),
            health: Health::Redirect,
        });
        assert_eq!(c.outcomes()[&r1], Outcome::Failed(Health::Redirect));
        assert!(!c.done());
        let late = c.handle(Input::Tick {
            now: 1_000 + CENSUS_TIMEOUT_SECS + 1,
        });
        assert!(late.iter().any(|a| matches!(a, Action::Unsubscribe { .. })));
        // All were subscribed at 1000: every one left is late.
        assert!(c.done());
        assert_eq!(c.outcomes().len(), relays.len());
        assert_eq!(
            c.outcomes()
                .values()
                .filter(|o| **o == Outcome::Silent)
                .count(),
            relays.len() - 2
        );
    }

    #[test]
    fn relays_given_twice_are_read_once() {
        let mut c = Crawler::new([
            "wss://a.example.org".into(),
            "wss://A.example.org:443/".into(),
        ]);
        assert_eq!(subscribed(&c.start(0)).len(), 1);
    }
}
