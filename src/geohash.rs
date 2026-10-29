// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: Unlicense OR 0BSD

//! Geohash cells: validation, bounds, centre, encoding and the 8 neighbours.
//!
//! Self-contained (no dependency), so that the apps can port it as is.

use crate::error::{Error, Result};
use crate::params::GEOHASH_ALPHABET as ALPHABET;
pub use crate::params::MAX_PRECISION;

/// Bounds of a cell: `[[south, west], [north, east]]`, in degrees.
pub type Bbox = [[f64; 2]; 2];

fn value(c: u8) -> Option<u8> {
    ALPHABET.iter().position(|&a| a == c).map(|p| p as u8)
}

/// The bounds of a cell. Lowercase only, 1 to [`MAX_PRECISION`] characters,
/// as the apps write the `g` tag.
pub fn bbox(geohash: &str) -> Result<Bbox> {
    let bad = || Error::InvalidGeohash(geohash.to_owned());
    if geohash.is_empty() || geohash.len() > MAX_PRECISION {
        return Err(bad());
    }
    let (mut lat, mut lon) = ((-90.0_f64, 90.0_f64), (-180.0_f64, 180.0_f64));
    // Bits alternate, longitude first.
    let mut even = true;
    for c in geohash.bytes() {
        let v = value(c).ok_or_else(bad)?;
        for bit in (0..5).rev() {
            let on = (v >> bit) & 1 == 1;
            let range = if even { &mut lon } else { &mut lat };
            let mid = (range.0 + range.1) / 2.0;
            if on {
                range.0 = mid;
            } else {
                range.1 = mid;
            }
            even = !even;
        }
    }
    Ok([[lat.0, lon.0], [lat.1, lon.1]])
}

/// The centre `(lat, lon)` of a cell: where the apps measure the distance
/// to the relays from.
pub fn center(geohash: &str) -> Result<(f64, f64)> {
    let [[s, w], [n, e]] = bbox(geohash)?;
    Ok(((s + n) / 2.0, (w + e) / 2.0))
}

/// The cell of `precision` characters that holds a point. `None` for an
/// invalid point or precision.
pub fn encode(lat: f64, lon: f64, precision: usize) -> Option<String> {
    if !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
        || precision == 0
        || precision > MAX_PRECISION
    {
        return None;
    }
    let (mut la, mut lo) = ((-90.0_f64, 90.0_f64), (-180.0_f64, 180.0_f64));
    let mut even = true;
    let mut out = String::with_capacity(precision);
    for _ in 0..precision {
        let mut v = 0u8;
        for _ in 0..5 {
            let (range, x) = if even { (&mut lo, lon) } else { (&mut la, lat) };
            let mid = (range.0 + range.1) / 2.0;
            v <<= 1;
            if x >= mid {
                v |= 1;
                range.0 = mid;
            } else {
                range.1 = mid;
            }
            even = !even;
        }
        out.push(ALPHABET[v as usize] as char);
    }
    Some(out)
}

/// The cells around `geohash` at the same precision, clockwise from the
/// north: N, NE, E, SE, S, SW, W, NW. Longitude wraps around the
/// antimeridian; there is nothing beyond the poles, so a cell on the top or
/// bottom row has only 5 neighbours.
///
/// Mirrors iOS `Geohash.neighbors(of:)` (`bitchat/Protocols/Geohash.swift`):
/// centre plus or minus one cell, encoded at the same precision.
pub fn neighbours(geohash: &str) -> Result<Vec<String>> {
    let [[s, w], [n, e]] = bbox(geohash)?;
    let (h, wd) = (n - s, e - w);
    let (clat, clon) = ((s + n) / 2.0, (w + e) / 2.0);
    let mut out = Vec::with_capacity(8);
    for (dlat, dlon) in [
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (-1.0, 1.0),
        (-1.0, 0.0),
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
    ] {
        let lat = clat + dlat * h;
        if !(-90.0..=90.0).contains(&lat) {
            continue;
        }
        let mut lon = clon + dlon * wd;
        if lon > 180.0 {
            lon -= 360.0;
        } else if lon < -180.0 {
            lon += 360.0;
        }
        if let Some(cell) = encode(lat, lon, geohash.len()) {
            out.push(cell);
        }
    }
    Ok(out)
}

/// A cell and its neighbours: the area a client follows around a channel
/// (iOS location notes read the cell and its 8 neighbours).
pub fn with_neighbours(geohash: &str) -> Result<Vec<String>> {
    let mut out = vec![geohash.to_owned()];
    out.extend(neighbours(geohash)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_and_encoding_agree() {
        // Paris, 5e arrondissement.
        let [[s, w], [n, e]] = bbox("u09tv").unwrap();
        assert_eq!(
            [s, w, n, e],
            [48.8232421875, 2.3291015625, 48.8671875, 2.373046875]
        );
        assert_eq!(encode(48.85, 2.35, 5).as_deref(), Some("u09tv"));
        assert_eq!(encode(48.8566, 2.3522, 9).as_deref(), Some("u09tvw0f6"));
        for g in [
            "u",
            "u0",
            "u09",
            "9q8yy",
            "gcpvj0",
            "s0000000",
            "zzzzzzzzzzzz",
        ] {
            let (lat, lon) = center(g).unwrap();
            assert_eq!(encode(lat, lon, g.len()).as_deref(), Some(g));
        }
    }

    #[test]
    fn invalid_cells() {
        for g in ["", "u0a", "U09", "u09tv!", "0123456789bcd"] {
            assert!(bbox(g).is_err(), "{g}");
        }
        assert_eq!(encode(91.0, 0.0, 5), None);
        assert_eq!(encode(0.0, 0.0, 0), None);
    }

    #[test]
    fn the_eight_neighbours() {
        // Expected values: the `geohash` crate 0.13 (`neighbors`), except
        // beyond the poles, where it wraps the latitude and the app does not.
        assert_eq!(
            neighbours("u09tv").unwrap(),
            [
                "u09wj", "u09wn", "u09ty", "u09tw", "u09tt", "u09ts", "u09tu", "u09wh"
            ]
        );
        // Across the antimeridian.
        assert_eq!(
            neighbours("xbpb").unwrap(),
            [
                "xbpc", "8001", "8000", "2pbp", "rzzz", "rzzx", "xbp8", "xbp9"
            ]
        );
        // Top row: no N, NE, NW.
        assert_eq!(neighbours("b").unwrap(), ["c", "9", "8", "x", "z"]);
        assert_eq!(
            neighbours("zzz").unwrap(),
            ["bpb", "bp8", "zzx", "zzw", "zzy"]
        );
    }
}
