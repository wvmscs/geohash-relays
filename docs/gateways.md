# Radio–Nostr gateways: who chooses the cell, and what a gateway must hold

> Written on 2026-11-06 from the source code of the apps and from a simulation on the frozen
> lists of `tests/data/`. It completes section 1 and approach A7 of
> [geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md).

**Question.** On the Bluetooth mesh, each user chooses a cell (geohash) for each message. The
cell decides the relays. A gateway that connects the mesh to Nostr must then reach the relays
of the cells that its users choose. Does the memory, the CPU time and the traffic of a gateway
grow without limit when its users choose different cells?

**Short answer.**
- In bitchat iOS, **the sender chooses the cell on the way up, and the gateway chooses it on the
  way down.** The gateway reads only one channel: the channel that its own user is viewing. Its
  state has fixed limits. Only its relay connections grow, one set of 5 for each new cell that
  its users write to.
- The cells of one place need few relays: **5 to 18** for all the channel levels of a point and
  the neighbour cells, on the two apps' lists. Distant cells ("teleport") make the number grow,
  up to the size of the list (about 330 servers).
- A gateway with little memory can serve its own place for both apps. It cannot follow every
  cell its users choose. It must announce the cells that it reads, and set a limit on the
  others.

Sources:
- Swift app (iOS and macOS): `permissionlesstech/bitchat`, commit `04995f6` (release `v1.8.0`).
- Android app: `permissionlesstech/bitchat-android`, commit `998f168`.

File paths are relative to these repositories.

## 0. Terms

Each term has one meaning only in this document.

| Term | Meaning |
|---|---|
| relay | A Nostr server. Never a device of the mesh. |
| mesh | The Bluetooth network of the phones. It has no server. |
| peer | A device of the mesh. |
| forward | A peer sends a mesh packet again on the mesh, while the TTL permits it. |
| gateway | A peer that also has Internet and moves events between the mesh and the relays. |
| gateway option | The bitchat iOS option that makes a phone a gateway for the geohash channels (bitchat: "gateway", `GatewayService`). |
| island option | The bitchat iOS option that connects two parts of the mesh in one place through the relays (bitchat: "mesh bridge", `BridgeService`). "Bridge" appears only in code names. |
| publish | Send an event to a relay. |
| read | Subscribe to a relay for the events of a cell, and receive them. |
| deposit | A peer without Internet gives a signed event to a gateway, which publishes it. |
| up / down | Up: from the mesh to the relays. Down: from the relays to the mesh. |
| connection | One WebSocket over TLS between a gateway and one relay. |

## 1. The two options of bitchat iOS

bitchat iOS has two options, the gateway option and the island option, that move signed Nostr events between the mesh and the relays. They
use one mesh packet type, `nostrCarrier` (0x28, `bitchat/Protocols/NostrCarrierPacket.swift`):
a direction, a geohash (1 to 12 characters) and the complete signed event (16 KiB at most).

| | Gateway option (`GatewayService.swift`) | Island option (`BridgeService.swift`) |
|---|---|---|
| User option | "share my internet with the mesh" | connect mesh islands of one place |
| Events | kind 20000 with `g` = the carrier's geohash | kinds 20000 and 20001 with `r` = the carrier's cell |
| Up (mesh → relays) | `toGateway`, directed to one gateway | `toBridge`, directed to one gateway |
| Down (relays → mesh) | `fromGateway`, sent to all peers | `fromBridge`, sent to all peers |
| Capability bit in the announce | `gateway` | `bridge`, with the cell in TLV `0x06` (`bridgeGeohash`, `Protocols/Packets.swift`) |

The sender signs each event with its own key for the cell (section 1.3 of the requirements). The
gateway cannot change it. It checks the signature before it publishes the event or sends it down to the mesh.

## 2. Who chooses the cell

### 2.1 Up: the sender

- **Gateway.** A peer without Internet sends its message to its channel's relays as usual. If no
  relay connection works, it also gives the event to the first reachable gateway peer
  (`GatewayService.uplinkViaMesh`, called from `GeohashSubscriptionManager.sendGeohash`). The
  carrier holds the geohash of the channel that the sender is viewing: any level, and any place
  when the sender "teleports".
- The gateway checks the event, then publishes it to **the 5 relays that the gateway calculates
  for this geohash, from its own list** (`ChatViewModelBootstrapper.configureGateway`,
  `publishToRelays`). If the gateway's list and the sender's list differ (iOS and Android, or two
  ages of one list), the event goes to the relays of the gateway's list.
- The gateway accepts deposits for any valid geohash. It does not compare the geohash with its
  own channel.
- **Island option.** The cell is the cell of precision 6 of the sender's location. A sender
  without location takes the cell that a gateway announces (`BridgeService.currentCell`).
  The gateway publishes deposits to the 5 relays of the carrier's cell.

### 2.2 Down: the gateway

- **Gateway.** The downlink is attached to the gateway's own channel subscription
  (`GeohashSubscriptionManager.switchLocationChannel`, `resubscribeCurrentGeohash`). The gateway
  sends down only the events of **the one channel that its own user is viewing**
  (`GatewayService.rebroadcastRelayEvent`). It does not read the cells of its mesh
  peers.
- A peer that receives a `fromGateway` carrier shows the event only if the carrier's geohash is
  the channel that this peer is viewing (`handleDownlinkBroadcast`). The broadcast still uses
  the airtime of the whole mesh: every node relays it within the TTL.
- **Island option.** The gateway reads its own cell and its 8 neighbours, with one
  filter `#r` on the 5 relays of its own cell (`configureBridge`, `openSubscription`;
  `refreshRendezvous`).

### 2.3 Consequences

- **A peer without Internet sends to any cell, but receives only the gateway's cell.** If it
  views another channel, it reads nothing from the relays. No message tells it.
- **Presence does not go up.** The gateway accepts only kind 20000 (`structurallyValidEvent`).
  The peers without Internet do not appear in the channel's participant count on Nostr.
- **No acknowledgement.** The uplink is "fire-and-forget" (comment of `uplinkViaMesh`). The
  sender does not know if the gateway published, queued or dropped the event.

## 3. What bitchat iOS limits

| Item | Limit (gateway option / island option) | Source |
|---|---|---|
| Deposits per depositor | 10 per minute | `Limits.uplinkEventsPerMinutePerDepositor` |
| Deposits queued while offline | 20 in total, 5 per depositor | `maxQueuedUplinks`, `maxQueuedUplinksPerDepositor` |
| Events sent down to the mesh | 30 / 20 per minute, 30 waiting (oldest dropped) | `downlinkEventsPerMinute`, `maxPendingDownlinks` |
| Event age | 15 min | `maxEventAgeSeconds` |
| Loop and duplicate caches | 3 sets of 512 event IDs, oldest removed | `maxTrackedEventIDs` |
| Carried event | 16 KiB | `NostrCarrierPacket.maxEventJSONBytes` |
| Events read (island option) | 600 events per minute, 120 per signer, 720 signature checks per minute, 128 participants | `BridgeService.Limits` |

Signature checks come after the cheap checks, the duplicate check and the rate limit. A flood of
false deposits costs parsing, not Schnorr verification.

**What has no limit: the relay connections.** `NostrRelayManager.sendEvent` opens a connection to
each target relay that has none (`ensureConnections`). The manager closes relays only when the
user changes the default relays (`dropRelays`). Thus each new cell that the gateway publishes to
can add up to 5 connections, and they stay open for the session. The
number of possible connections is the number of servers in the list.

## 4. The other clients

- **bitchat Android** has the capability bits `GATEWAY` and `BRIDGE`
  (`model/PeerCapabilities.kt`) and a receive limit for packet 0x28 marked "iOS only"
  (`protocol/PacketPayloadLimits.kt`). It has no code for the gateway option or the island option. An Android phone
  forwards the carriers on the mesh, as it forwards every packet type within the TTL
  (`mesh/PacketRelayManager.kt`), but it cannot be a gateway.
- **AirHop** and **Sonar**: not studied here.

## 5. Measurement: how many relays a gateway needs

Method: the selection rule of each app (`as_::select::closest`, 5 nearest to the cell's centre)
on the iOS list of 2026-07-26 (`online_relays_gps.csv`, 326 servers) and the Android list of
2026-10-29 (`nostr_relays-2026-10-29.csv`, 392 lines, 346 servers). The number in brackets
counts the relays that the census of 2026-10-30 found working
(`census-usable-2026-10-30.txt`). The calculation is a re-implementation of `closest`
outside this crate. A Rust example must confirm it before the figures go into
[current-state.md](current-state.md).

**Cells of one place.** The cells that contain the gateway at the channel levels
(`params::CHANNEL_LEVELS`: 2, 4, 5, 6, 7, 8), and the 8 neighbours at levels 5 to 8:

| Gateway at | One cell (level 5), iOS | All levels, iOS list | All levels, Android list | Both lists |
|---|---|---|---|---|
| Paris | 5 (2) | 11 (4) | 10 (7) | **18 (9)** |
| Corrèze (rural France) | 5 (2) | 10 (4) | 9 (6) | **16 (8)** |
| New York | 5 (3) | 9 (4) | 16 (7) | **17 (7)** |
| Chad | 5 (2) | 5 (2) | 5 (4) | **9 (5)** |

- The 5 nearest relays change little from one level to the next: a cell's centre is near the
  centre of the cell that contains it. Thus 6 levels and their neighbours need 2 to 3 times the
  relays of one cell, not 54 times.
- The island option's cell of precision 6 and its 8 neighbours use the same 5 relays on the iOS list, at
  the four points.

**Cells of other places (teleport).** Cells drawn at random, precision 2, 4, 5 or 6, union of
the relays of all of them:

| Cells | 1 | 10 | 50 | 100 | 500 | 1,000 |
|---|---|---|---|---|---|---|
| Uniform over the Earth, iOS list | 5 | 33 | 67 | 88 | 113 | 135 |
| Uniform over the Earth, Android list | 5 | 19 | 67 | 86 | 116 | 142 |
| Near listed relays (populated places), iOS | 5 | 36 | 99 | 158 | 199 | 201 |
| Near listed relays, Android | 5 | 33 | 104 | 144 | 198 | 203 |

A few distant cells cost as much as all the levels of the gateway's own place. Tens of cells
need about a hundred relays.

## 6. A resource model for a gateway

Let *C* be the set of cells that the gateway reads, and *R(C)* the union of their relays
(section 5).

- **Connections:** |*R(C)*| WebSocket connections over TLS. A TLS record can hold up to 16 KiB
  of data (RFC 8446, section 5.1), so each connection needs a receive buffer of this size unless
  the relay accepts a smaller maximum fragment length (RFC 6066). This is the largest cost on a
  microcontroller.
- **Subscriptions:** one `REQ` per relay is sufficient. A filter `#g` takes many values
  (requirement R10). The number of subscriptions follows |*R(C)*|, not |*C*|.
- **Inbound traffic and CPU:** each event arrives once from each relay that has it, up to 5
  times for one cell (more when the lists of two apps are read). The gateway removes duplicates
  by event ID before it checks the signature, so it checks each event once.
- **Outbound traffic:** each uplinked event goes to the relays of its cell: 5 (AS), 8 (A0 write
  set) or 10 (AirHop).
- **Lists:** the two CSV lists are 14 to 17 KB each. The entry point of branch `api` can keep
  only the relays that a home region needs (`home`, `params::HOME_NEAREST`).
- **Mesh airtime:** every peer forwards each event that the gateway sends down. The per-minute budget is a
  limit on the mesh, not only on the gateway.

## 7. Proposals for a gateway with few resources

These are proposals. No client implements them.

1. **Read a fixed set of cells, and announce it.** The owner of the gateway configures the
   cells it reads, for example the levels of its place (section 5: 9 to 18 relays for both
   lists). The gateway announces them in its announce, as the island option announces its cell (TLV
   `0x06`). A peer then knows which channels it can read through this gateway.
2. **Publish to any cell, within a connection budget.** For a cell outside the fixed set, the
   gateway connects, publishes, waits for `OK` and closes, or keeps a small pool of connections
   with least-recently-used replacement. bitchat iOS keeps these connections open for the
   session (section 3).
3. **Read other cells on request, for a limited time.** A peer asks for a cell; the gateway
   counts the request in new relays, not in cells, and ends it after a fixed time unless the peer
   asks again. A limit for each peer and a total limit stop the growth of section 5.
4. **One subscription per relay**, with all the cells of this relay in one `#g` (or `#r`) filter.
5. **Acknowledge deposits** (published, queued, refused) so that the sender knows when its cell is
   not served. This needs a new carrier field; older apps skip unknown TLVs
   (`NostrCarrierPacket.decode`).
6. **Carry presence (kind 20001) up**, within the same per-depositor limit, so that peers without
   Internet appear in the channel.
7. **Calculate the relays with both apps' rules** for the fixed set (approach A4 at the scale of
   one gateway), because the gateway cannot know which list its depositors use.

## 8. A gateway without its own Tor client

The two apps connect to the relays through Tor by default (section 1.5 of the requirements). A
microcontroller has no Tor client of its own. Without Tor, each relay sees the IP address of
the gateway, and the relays of all the cells that it serves can link them.

Proposal for such a gateway:

1. **Use a remote Tor service of trust.** A participant gives the gateway access to a Tor
   client (a SOCKS port) on another machine. This participant can find the network position of
   the gateway. The participant agrees not to show its own network position with traffic that
   others can recognise as coming from the gateway's network.
2. **Hide all the metadata between the gateway and the remote service.** The link is an
   encrypted tunnel (TLS, or QUIC, which costs less on a constrained device). An observer of
   the link must not see the relays, the cells or the event sizes that the gateway uses.
   Another security policy is acceptable if it is audited.
3. **Tell the users.** The gateway knows that it uses a remote Tor service, and reports it: in
   its status and in what it announces to the mesh. A peer can then choose not to deposit its
   events with it.
4. **Never fall back to a direct connection.** If the remote service is not available, the
   gateway stops publishing and reading. It does not connect to the relays without Tor.
