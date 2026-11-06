// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Error type of the crate.

/// Errors returned by this crate. Variants come with the lots that need
/// them (see `bitchat-console/PLAN_ET1.md`, lot 1).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid geohash {0:?}")]
    InvalidGeohash(String),
    /// A6: a filter of the candidates that only this peer applies breaks
    /// the rendezvous unless the strategy writes wide (A0).
    #[error("a local qualification needs a strategy that writes wide (A0), or a shared filter")]
    LocalQualification,
    #[error("invalid relay address {0:?}")]
    InvalidRelayUrl(String),
    /// No relay list could be fetched, and none was kept.
    #[error("no relay list")]
    NoRelayList,
    /// `get_with` for a strategy not given to `Relays`.
    #[error("strategy {0} not configured")]
    StrategyNotConfigured(String),
    /// A list or a kept file that cannot be read.
    #[error("unreadable {0}")]
    Unreadable(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_input() {
        assert_eq!(
            Error::InvalidGeohash("u0a".into()).to_string(),
            "invalid geohash \"u0a\""
        );
    }
}
