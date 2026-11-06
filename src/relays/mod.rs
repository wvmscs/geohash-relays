// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! One entry point for a client: give it a means to talk ([`RelayComm`]),
//! ask it for the relays of a cell ([`Relays::get`]).
//!
//! It holds the logic the strategies leave to the host: fetching the lists
//! and keeping them, a home region for constrained devices, the relays found
//! dead or refusing, private relays, a migration from one strategy to
//! another. The host only carries bytes. Everything is decided in a core
//! without I/O ([`engine`]); [`Relays`] (feature `async`) and
//! [`blocking::Relays`] (feature `blocking`) only run its requests.
//!
//! Each strategy is a feature (`apps`, `airhop`, `a0`, `a1`): a
//! [`Strategy`] value exists only when its feature is on.

#[cfg(all(
    any(feature = "async", feature = "blocking"),
    not(any(feature = "apps", feature = "airhop", feature = "a0", feature = "a1"))
))]
compile_error!("the entry point needs at least one strategy: feature apps, airhop, a0 or a1");

#[cfg(any(feature = "async", feature = "blocking"))]
mod engine;
pub mod gateway;
mod types;

#[cfg(feature = "async")]
mod nonblocking;

#[cfg(feature = "blocking")]
pub mod blocking;

#[cfg(feature = "async")]
pub use nonblocking::{Link, RelayComm, Relays};
pub use types::{
    Authority, Cell, CellRelays, CommError, Extra, Relay, RelayUrl, Role, Seen, StoredFile,
};

/// A strategy the entry point can run.
#[derive(Debug, Clone, PartialEq)]
pub enum Strategy {
    /// What the BitChat apps do today (`AS`), on their own lists.
    #[cfg(feature = "apps")]
    Apps(crate::Apps),
    /// What AirHop does: the 10 nearest, on the shared list.
    #[cfg(feature = "airhop")]
    AirHop,
    /// One list, read 5, write 8, dead relays replaced.
    #[cfg(feature = "a0")]
    A0,
    /// Salted rendezvous hashing, on the shared list. Needs the whole list:
    /// a home region keeps everything while it is configured.
    #[cfg(feature = "a1")]
    A1(crate::a1::Salts),
}

impl Strategy {
    pub fn name(&self) -> &'static str {
        match *self {
            #[cfg(feature = "apps")]
            Strategy::Apps(_) => "AS",
            #[cfg(feature = "airhop")]
            Strategy::AirHop => "AirHop",
            #[cfg(feature = "a0")]
            Strategy::A0 => "A0",
            #[cfg(feature = "a1")]
            Strategy::A1(_) => "A1",
        }
    }

    /// Whether the choice reads the whole list (no home region possible).
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn needs_whole_list(&self) -> bool {
        match self {
            #[cfg(feature = "a1")]
            Strategy::A1(_) => true,
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }
}

/// Names of the files the entry point keeps through the host.
pub mod files {
    /// What it knows of relays (dead, refusing), with dates.
    pub const HEALTH: &str = "health";
    /// The prefix of a kept list; the rest names its authority.
    pub const LIST_PREFIX: &str = "list-";
}
