# Evaluation of the relay strategies

How the strategies of this crate compare, on the frozen relay lists of the apps and on the
real network (29/10/2026). The strategies are those of
[geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md) (§5); `AS` is
what the apps do today. Reproduce the bench with `cargo run --release --example compare`.

## Bench (frozen lists)

Each strategy has two peers that must meet in a cell: for `AS`, an iOS peer (iOS list, iOS
rule) and an Android peer (Android list, Android rule); for A0, two peers whose copies of the
one list are a month apart; for A1, a peer on the iOS list and a peer on the Android list.
p2: the 1,024 cells of precision 2; p5: 500 cells of precision 5 drawn at random.

| | AS (today) | A0 | A1 |
|---|---|---|---|
| Rendezvous, p2 / p5 | 93.3 % / 91.8 % | **100 % / 100 %** | 98.4 % / 97.0 % |
| Cells with fewer than 5 servers (p2) | 322 | 0 | 0 |
| Servers used; busiest server (cells, p2) | 119; 361 | 136; 326 | **332; 30** |
| Stability when the list changes (iOS, a month; Android, 3 days) | 0.47; 0.43 | 0.47; — | **0.62; 0.78** |
| Rendezvous with 1, 2, 3 chosen relays dead | 87, 80, 75 % | **100, 100, 100 %** | 96, 93, 90 % |
| Servers an attacker needs at the city point to take a cell (iOS) | 5 | 5 | **> 60** (5 without salt) |

The other strategies are not a choice of relays from a list, so the bench measures them by
their property:

| | Property, tested |
|---|---|
| A2 (NIP-66 monitors) | A relay is a candidate when *m* trusted monitors report it; a finding counts when most of them make it; a lone hostile monitor has no effect; results that would remove more than half of a list are not used. |
| A3 (relays in presences) | In every p2 cell where iOS and Android peers miss each other today, they meet once two keys on each side announce their relays; 50 keys without proof of work change nothing. |
| A4 (republishers) | An event on the iOS set reaches the Android set once per republisher, never goes back, no loop between two republishers. |
| A5 (federation) | Peers of the same federation always pick the same members; a member's omissions are found by comparison. |
| A6 (qualification) | NIP-11 declarations and probes give a verdict per requirement; a probe prevails; a filter only this peer applies is refused unless the strategy writes wide (A0). |

## Relays a strategy leaves out (census, 30/10/2026)

What each strategy asks of a candidate relay (`NEEDS`, beside each strategy), counted on a
census of the real network: geohash-relays-net's `relay-census`, six rounds of NIP-65 relay
lists through Tor from three relays, 3,371 relays known, 1,138 answered. The requirements
(A6) are the same for all; only the need of a place differs: a strategy that ranks by distance
leaves out the relays without one (`.onion`, address not located).

| Strategy | Needs a place | Usable | Did not answer | Refused (A6) | `ws://` | No place |
|---|---|---|---|---|---|---|
| AS, A0, A4 | yes | 886 | 2,233 | 164 | 34 | **54** |
| A1, A2, A3, A5 | no | **940** | 2,233 | 164 | 34 | 0 |

Ports other than 443 and paths are kept by the common rule (`normalize_url`): 886 usable
addresses are only **666 servers**; 152 servers appear under several paths (220 addresses
more, up to 11 for one server: per-user `/inbox` and generated paths). A strategy that ranks
by distance would give one server several of a cell's 5 places, as Android does with `host`
and `host:443`, only worse: how to count a server is an open question (`PLAN_ET1.md`, lot 13).
Reproduce with `relay-census --refresh`, then `relay-census --eligibility`.

## Real network (read only)

- **NIP-11 (A6, §7 question 3)**, through Tor, 467 relays of the two lists: 323 to 352 give a
  document (two runs; the slow ones time out at 20 s). By their declarations alone, 26 to 28
  are unusable: 21–22 restrict writes (R6), 8–9 require NIP-42 (R5), 3 require payment (R6),
  1 requires more than 8 bits of proof of work (R7). 81–93 allow 20 subscriptions or fewer
  per connection (R8), 7 accept messages under 64 KiB (R13).
- **NIP-66 monitors (A2, §7 question 6)**: active monitors publish kind 10166 on
  `relay.nostr.watch`, `monitorlizard.nostr1.com`, `relaypag.es`, and on large relays; some
  check writes (`c: write`), most only `open`, `read`, `nip11`. A2 needs monitors that test
  writes through Tor; few do today.
- **Traffic (A4, §7 question 5)**: in the last hour, cell `u0` had 7 messages and 203
  presences on `nostr.sathoarder.com` (about 3.5 events per minute); `9q`, `dr`, and
  `relay.damus.io` for the three, had none. A small sample, but the limit of 60 republications
  per minute of A4 is far above it.
- **The list itself**: with A1, the busiest server of the bench is a temporary tunnel
  (`*.trycloudflare.com`): the lists carry entries that A6 or A2 must remove.

Not measured yet (they write, or need the user's machines): retention of ephemeral events and
access through Tor by probing (§7 questions 1 and 2, `bitchat-console --qualify-probe`); a
real strfry federation (A5).

## Proposal

From the most immediate to the most structural, as §6 of the requirements:

1. **Now, no change to the apps: A4.** Republishers (`bitchat-console --republish`) connect
   the iOS and Android sets of the cells they follow.
2. **For both apps: A0 + A6.** One list and the iOS rule, write 8 and read 5, failed relays
   replaced, optional shared relays; NIP-11 qualification; wait for `OK`, answer `AUTH`, read
   `CLOSED`. Every peer pair meets in the bench, even with dead relays and lists a month apart.
   A0 does not resist capture.
3. **Target: A1 + A2, then A3.** Salted HRW on the candidates the monitors agree on: capture
   out of reach, load spread over 332 servers instead of 119, choices more stable when the list
   moves; then the relays announced in presences, upward compatible with the current apps.

A5 is infrastructure the operators may add; it needs no change to the apps.

The Swift and Kotlin ports (lot 11 of `PLAN_ET1.md`) cover the strategies retained after
review: proposed A0, A6, A1 (with its frozen score vectors), and the A3 tag.
