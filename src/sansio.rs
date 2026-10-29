// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! The common form of the strategies that talk to relays (decision G13 of
//! `PLAN_ET1.md`): **machines without I/O**. A machine takes [`Input`]s
//! (an event seen on a relay, an `OK`, a failure, the time) and returns
//! [`Action`]s (publish this event on that relay, subscribe, fetch a NIP-11
//! document). `geohash-relays-net` runs the actions with the communication
//! functions of its feature; a Swift or Kotlin port would only replace that
//! runner.
//!
//! Events are carried as [`EventMeta`]: the fields a rule reads, and the
//! event's JSON as received, to publish it again unchanged. Signatures are
//! checked by the host before an event becomes an input.

use crate::strategy::Health;

/// A signed Nostr event, as far as the rules need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMeta {
    pub id: String,
    pub pubkey: String,
    pub kind: u32,
    pub created_at: i64,
    pub tags: Vec<Vec<String>>,
    /// The event's JSON object, as received (already verified).
    pub raw: String,
}

impl EventMeta {
    /// The first value of the first tag named `name`.
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some(name))
            .and_then(|t| t.get(1))
            .map(String::as_str)
    }

    /// The values of every tag named `name`.
    pub fn tag_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.tags
            .iter()
            .filter(move |t| t.first().map(String::as_str) == Some(name))
            .filter_map(|t| t.get(1).map(String::as_str))
    }

    /// Proof of work of the id (NIP-13): leading zero bits.
    pub fn pow_bits(&self) -> u32 {
        let mut bits = 0;
        for c in self.id.chars() {
            let Some(v) = c.to_digit(16) else { return bits };
            if v == 0 {
                bits += 4;
            } else {
                return bits + v.leading_zeros() - 28;
            }
        }
        bits
    }
}

/// A subscription filter, as far as the rules need one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterSpec {
    pub kinds: Vec<u32>,
    /// Single-letter tag filters: (`'g'`, values).
    pub tags: Vec<(char, Vec<String>)>,
    pub authors: Vec<String>,
    pub since: Option<i64>,
    pub limit: Option<u32>,
}

/// What a machine is told.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// A verified event, received from `relay`.
    Event { relay: String, event: EventMeta },
    /// The relay's answer to a publication.
    Ok {
        relay: String,
        id: String,
        accepted: bool,
        message: String,
    },
    /// End of stored events of the subscription on `relay`.
    Eose { relay: String },
    /// The relay cannot be used, for this reason.
    Failed { relay: String, health: Health },
    /// The relay's NIP-11 document (raw JSON), or `None` when it has none.
    Nip11 { relay: String, json: Option<String> },
    /// The time now, in Unix seconds. Sent regularly.
    Tick { now: i64 },
}

/// What a machine asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Subscribe on `relay` (one subscription per relay and machine;
    /// replaces the previous one).
    Subscribe { relay: String, filter: FilterSpec },
    /// Close the machine's subscription on `relay`.
    Unsubscribe { relay: String },
    /// Publish this event (its JSON, unchanged) on `relay`.
    Publish {
        relay: String,
        id: String,
        raw: String,
    },
    /// Fetch the NIP-11 document of `relay`; the answer comes as
    /// [`Input::Nip11`].
    FetchNip11 { relay: String },
}

/// A strategy that talks to relays.
pub trait Machine: Send {
    /// What to do first.
    fn start(&mut self, now: i64) -> Vec<Action>;
    /// What to do after this input.
    fn handle(&mut self, input: Input) -> Vec<Action>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> EventMeta {
        EventMeta {
            id: id.into(),
            pubkey: String::new(),
            kind: 20000,
            created_at: 0,
            tags: vec![
                vec!["g".into(), "u09tv".into()],
                vec!["relay".into(), "wss://a".into()],
                vec!["relay".into(), "wss://b".into()],
            ],
            raw: String::new(),
        }
    }

    #[test]
    fn pow_and_tags() {
        assert_eq!(ev("00ff").pow_bits(), 8);
        assert_eq!(ev("0f").pow_bits(), 4);
        assert_eq!(ev("1f").pow_bits(), 3);
        assert_eq!(ev("8f").pow_bits(), 0);
        assert_eq!(ev("x").tag("g"), Some("u09tv"));
        assert_eq!(
            ev("x").tag_values("relay").collect::<Vec<_>>(),
            ["wss://a", "wss://b"]
        );
    }
}
