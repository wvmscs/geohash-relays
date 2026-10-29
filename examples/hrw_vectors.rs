// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Prints the A1 score vectors frozen in `tests/a1.rs` (for the Swift and
//! Kotlin ports):
//!
//!   cargo run --example hrw_vectors

use geohash_relays::a1::score;

/// (salt in hex, cell, relay address).
pub const INPUTS: &[(&str, &str, &str)] = &[
    ("", "u09tv", "wss://relay.damus.io"),
    ("00", "u09tv", "wss://relay.damus.io"),
    ("00", "u09t", "relay.damus.io:443"),
    ("6b2f", "9q8yy", "wss://nos.lol"),
    ("6b2f", "9q", "wss://nos.lol"),
];

fn main() {
    for (salt, cell, url) in INPUTS {
        let s: Vec<u8> = (0..salt.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&salt[i..i + 2], 16).unwrap())
            .collect();
        println!(
            "(\"{salt}\", \"{cell}\", \"{url}\", 0x{:016x}),",
            score(&s, cell, 4, url)
        );
    }
}
