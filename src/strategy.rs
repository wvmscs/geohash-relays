// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! How a client picks the relays of a channel, behind one interface, so that
//! the apps' rule (`AS`) and the alternatives (`A0` to `A6`, later) can be
//! swapped and compared. See
//! `docs/geohash-relays-requirements-and-alternatives.md`.

use crate::directory::RelayDirectory;
use crate::error::Result;
use crate::select::{Selected, select_both};

/// The relays of a channel.
pub trait Strategy: Send + Sync {
    /// Short name: `"AS"`, `"A0"`…
    fn name(&self) -> &str;

    /// Servers to read the channel from, nearest or best first, with the
    /// detail of each app's choice.
    fn read(&self, geohash: &str) -> Result<Vec<Selected>>;

    /// Servers to write to. The same as [`read`](Self::read) unless the
    /// strategy writes wider than it reads (A0).
    fn write(&self, geohash: &str) -> Result<Vec<Selected>> {
        self.read(geohash)
    }
}

/// Which apps' choice the `AS` strategy follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apps {
    /// Union of the iOS and Android choices: reaches the users of both.
    Both,
    Ios,
    Android,
}

/// `AS`: what the apps do today. Each app picks the 5 relays nearest to the
/// channel centre from its own list, with its own rule
/// ([`select_both`]).
#[derive(Debug, Clone)]
pub struct AppStrategy {
    ios: Option<RelayDirectory>,
    android: Option<RelayDirectory>,
    apps: Apps,
}

impl AppStrategy {
    /// Either list may be missing; then that app chooses nothing.
    pub fn new(ios: Option<RelayDirectory>, android: Option<RelayDirectory>, apps: Apps) -> Self {
        AppStrategy { ios, android, apps }
    }

    pub fn apps(&self) -> Apps {
        self.apps
    }
}

impl Strategy for AppStrategy {
    fn name(&self) -> &str {
        "AS"
    }

    fn read(&self, geohash: &str) -> Result<Vec<Selected>> {
        let (ios, android) = match self.apps {
            Apps::Both => (self.ios.as_ref(), self.android.as_ref()),
            Apps::Ios => (self.ios.as_ref(), None),
            Apps::Android => (None, self.android.as_ref()),
        };
        select_both(ios, android, geohash)
    }
}
