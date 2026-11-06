# Frozen relay lists

| File | Source | Commit | Date |
|---|---|---|---|
| `online_relays_gps.csv` | bitchat `relays/online_relays_gps.csv` (iOS) | `fb8fe39713` | 2026-07-26 |
| `online_relays_gps-2026-06-21.csv` | the same, previous version | `f688e529f6` | 2026-06-21 |
| `nostr_relays.csv` | georelays `nostr_relays.csv` (Android) | `85f994a193` | 2026-10-25 |
| `nostr_relays-2026-10-22.csv` | the same, 3 days earlier | `282e64abd9` | 2026-10-22 |

Never downloaded by the tests. The older versions measure how stable a strategy is when a list
changes (bench, lot 1).

Study of the clients (`docs/current-state.md`, `cargo run --release --example clients`):

| File | Source | Commit | Date |
|---|---|---|---|
| `nostr_relays-2026-10-29.csv` | georelays `nostr_relays.csv` (Android), the census's copy | `e7c6e88edf` | 2026-10-29 |
| `airhop-nostr_relays-v1.0.10.csv` | AirHop `assets/data/nostr_relays.csv`, vendored from georelays | release `v1.0.10` | 2026-11-02 |
| `census-usable-2026-10-30.txt` | geohash-relays-net `relay-census`: the addresses that answered and that A6 does not refuse (`wss://`) | census of 2026-10-30 | 2026-10-30 |
| `sonar-relays_gps-2026-06-15.csv` | Sonar `core/sonar-core/src/relays_gps.csv`, compiled into its core | `fdf091c` (file of 2026-06-15) | 2026-06-15 |
| `sonar-online_relays_gps-2026-11-01.csv` | Sonar `relays/online_relays_gps.csv`, refreshed weekly, not read by the core | `fdf091c` | 2026-11-01 |
