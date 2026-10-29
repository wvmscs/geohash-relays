# AGENTS.md

Guidance for coding agents (and humans) working in this crate.

## What this is

`geohash-relays`: the relay choice of BitChat location channels (geohash), isolated from any
client so that strategies can be compared and swapped:

- `AS`: what the iOS and Android apps do today (each app's list and rule);
- `A0` to `A6`: the alternatives of
  [docs/geohash-relays-requirements-and-alternatives.md](docs/geohash-relays-requirements-and-alternatives.md), later
  (branch `strategies`, work in progress).

It serves [bitchat-console](https://github.com/wvmscs/bitchat-console) first (its decision
D2), [bitchat-geohash](https://github.com/wvmscs/bitchat-geohash) too, and is meant to be
proposed to the BitChat apps (iOS, Android), hence the public-domain licence (Unlicense, like
the iOS app). Progress is tracked in bitchat-console's `PLAN_ET1.md` (lot 1).

Rules that only concern the author's local checkout, if any, are in `AGENTS.local.md`
(not committed).

## Constraints

- Keep it small, pure and dependency-light: no network, no async, no I/O beyond parsing the
  text of a relay list given by the caller.
- Every rule must cite the app source it mirrors (file and function), so that a PR to the
  apps can be reviewed against it.
- Reproducible tests: frozen relay lists, never downloaded.
- Code, comments and rustdoc in English.
- Own git repository (`origin` = `git@github.com:wvmscs/geohash-relays.git`, branch `main`),
  own `[workspace]`. Commits only when the user asks.

## Commands

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
reuse lint      # every file has its copyright and licence (also a GitHub Action)
```

<!-- REUSE-IgnoreStart -->
A new file starts with
`SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)` and
`SPDX-License-Identifier: Unlicense OR 0BSD` in a comment, or is covered by `REUSE.toml`.
<!-- REUSE-IgnoreEnd -->
