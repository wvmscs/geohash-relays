# A5: a federation of synchronised relays

How relays of a region can serve BitChat location channels together (A5 of
[geohash-relays-requirements-and-alternatives.md](geohash-relays-requirements-and-alternatives.md)). A client then
needs one member, not the same relay as its neighbours (`a5::Federation`). This is
infrastructure: the operators must agree.

## What to synchronise

Kinds 20000 (channel messages), 20001 (presences) and 1 (notes), with a `g` tag. NIP-01
filters cannot say "has a `g` tag": give the cells of the region, or the kinds only
(`a5::sync_filter`):

```json
{"kinds":[20000,20001,1],"#g":["u09","u0d","u0e"]}
```

Requirements on each member (§3): R1 to R15, and **R11**: keep kinds 20000 and 20001 for at
least one hour, or the history the apps ask for (`since: -1 h`) is lost.

## With strfry

Two complementary mechanisms:

- **Live** events, ephemeral kinds included: `strfry router`, a stream in both directions
  between members. Example `strfry-router.config`:

  ```text
  connectionTimeout = 20

  streams {
      bitchat {
          dir = "both"
          filter = { "kinds": [20000, 20001, 1] }
          urls = [
              "wss://member-a.example"
              "wss://member-b.example"
          ]
      }
  }
  ```

- **Stored** events (notes, and the ephemeral ones a member keeps): NIP-77 (negentropy),
  for example every few minutes from cron:

  ```sh
  strfry sync wss://member-b.example --filter '{"kinds":[20000,20001,1]}' --dir both
  ```

  For the ephemeral kinds to be kept long enough, set strfry's lifetime of ephemeral events
  (`relay.ephemeralEventsLifetimeSeconds`) to 3600 or more (R11).

Check the option names against the strfry version in use.

## Checking the federation

Events are signed: a member can only **omit** some. `a5::Comparer` reads the same window from
every member, and `a5::omissions` lists what each one lacks; bitchat-console runs it with
`--federation <urls> --federation-check`.
