// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! `AS`: what the apps do today, with **two implementations**, each fed by
//! its own list: iOS (the reviewed `relays/online_relays_gps.csv`, iOS rule)
//! and Android (georelays' `nostr_relays.csv`, Android rule). Each app picks
//! the 5 relays nearest to the channel centre ([`select::closest`]);
//! [`Apps::Both`] is the union of the two choices ([`select::select_both`]).

pub mod directory;
pub mod select;

use crate::error::Result;
use crate::strategy::{Selected, Strategy};
use directory::RelayDirectory;
use select::select_both;

/// Which apps' choice the `AS` strategy follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apps {
    /// Union of the iOS and Android choices: reaches the users of both.
    Both,
    Ios,
    Android,
}

/// `AS`: each app picks the 5 relays nearest to the channel centre from its
/// own list, with its own rule.
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

    /// The iOS implementation, on the iOS list.
    pub fn ios(list: RelayDirectory) -> Self {
        AppStrategy::new(Some(list), None, Apps::Ios)
    }

    /// The Android implementation, on the Android list.
    pub fn android(list: RelayDirectory) -> Self {
        AppStrategy::new(None, Some(list), Apps::Android)
    }

    pub fn apps(&self) -> Apps {
        self.apps
    }
}

impl Strategy for AppStrategy {
    fn name(&self) -> &str {
        "AS"
    }

    fn describe(&self) -> String {
        let apps = match self.apps {
            Apps::Both => "iOS + Android",
            Apps::Ios => "iOS",
            Apps::Android => "Android",
        };
        format!("AS ({apps}, {} nearest)", crate::params::GEO_RELAY_COUNT)
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
