# AGENTS.md

Guidance for coding agents (and humans) working in this crate.

## What this is

`geohash-relays`: the relay choice of BitChat location channels (geohash), isolated from any
client so that strategies can be compared and swapped:

- `as_` (`AS`): what the iOS and Android apps do today (each app's list and rule);
- `a0` to `a6`: the alternatives of
  `../bitchat-envelope/Documents/geohash-relays-requirements-and-alternatives.md`.

It serves `bitchat-console` first (decision D2 of `../bitchat-console/SPECIFICATIONS.md`),
`bitchat-geohash` later, and is meant to be proposed to the BitChat apps (iOS, Android), hence
the public-domain licence (Unlicense, like the iOS app). Progress is tracked in
`../bitchat-console/PLAN_ET1.md` (lot 1, `AS`) and in `PLAN_ET1.md` (the alternatives A0 to
A6).

## Constraints

- Keep it small, pure and dependency-light: no network, no async, no I/O beyond parsing the
  text given by the caller (relay lists, NIP-11 documents, monitor reports). The network is in
  `../geohash-relays-net` (GPL-3.0-or-later), which depends on this crate, never the reverse.
- Strategies that talk to relays are machines without I/O (`sansio`: inputs in, actions
  out, the time given), run by `geohash-relays-net` (decision G13).
- Every constant lives in `params`, with its source (decision G14): none elsewhere.
- Dependencies (direct and transitive) must have a licence compatible with
  `Unlicense OR 0BSD`, permissive only: `cargo deny check` (`deny.toml`, also in CI).
- Every rule must cite the app source it mirrors (file and function), so that a PR to the
  apps can be reviewed against it.
- Reproducible tests: frozen relay lists, never downloaded.
- Code, comments and rustdoc in English.
- Own git repository (`origin` = `git@github.com:wvmscs/geohash-relays.git`), own
  `[workspace]`. `main` and the tag `v0.1.0` stay as they are (tagged versions never change,
  no new tags); the strategies are developed on the `strategies` branch, and pull requests
  are rebased on it.

## Commands

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check advisories licenses sources
reuse lint      # every file has its copyright and licence (also a GitHub Action)
```

<!-- REUSE-IgnoreStart -->
A new file starts with
`SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)` and
`SPDX-License-Identifier: Unlicense OR 0BSD` in a comment, or is covered by `REUSE.toml`.
<!-- REUSE-IgnoreEnd -->
