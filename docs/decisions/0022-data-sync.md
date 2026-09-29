# 0022: data sync

Status: accepted, 2026-09-28. Settles the draft parts of 0012: the request, the reply mapping,
and how the host fetches.

## Context

Phase 7 part 5. Decision 0012 let a module take its data from the pack, a sync, or both, and left
the request and the reply mapping as a draft until a first source landed. That source is the
weather: a forecast is only known a few days ahead, while the pack is written weeks before.

## Decision

**The engine plans and reads, the host fetches.** `Engine::requests(world, module)` answers the
fetches to make: `module` alone when an action names it, or, when empty, every automatic sync that
is due. The host makes each HTTPS `GET` and hands the reply to `Engine::received(world, request,
status, body)`, which answers the facts to store, `sync.<module>`: `{as_of, tried, failed, rows}`.
Status 0 means nothing answered, and the body then says why.

**A request is built from expressions.** `request.url` is an `https://` address; `query` maps
parameter names to expressions over the screen's scope, percent-encoded; one with no value (null,
a list or a mapping), say with no place yet, skips the fetch. `tag` sets row keys from the same
scope on every row read, so a reply that does not say which place it is for still lands on the
right one.

**A reply is read by paths.** `read` maps each row key to a dotted path in the JSON reply. A path
that finds a list gives one value per row, one over a list goes into each item, and a single
value goes to every row. Null values are left out and a row with nothing read is dropped. A reply
that is not JSON, or has nothing to read, is a failure like any other.

**A failure keeps the last good rows.** It sets `failed` and `tried`, and keeps `as_of` and
`rows`. An automatic sync is due `every` after the last good reply, and never sooner than 15
minutes after the last try, so a server that is down is not asked every minute. The engine adds
that instant to `watch.until`, and the host checks what is due whenever it asks for a screen.

**Synced climate rows come first.** They rank like the pack's entries, and a stable sort lets them
win a tie. `weather` gains `syncs`, `as_of` and `failed`, and is never null for a module that
syncs, so a screen can always offer the sync.

**The host asks, and keeps.** Before the first fetch of a pack, the app lists `Engine::hosts()`
and asks once; the answer is kept per pack until the list changes. A "not now" stops automatic
syncs until the next button press. A secret is asked the first time it is needed and kept sealed
with a key in the Android Keystore, for the host it was given for only; a 401 or 403 forgets it, so the next sync asks again.

**The travel template** shows the weather only when it has a `high`, and on the agenda offers
`climate.sync` to adults, with the reason when the last try failed.

## Deviations from the design

- **Only `climate` syncs.** Any other module with a `sync` is a load error. Other modules have no
  source yet; each gets its own reading when one appears.
- **A secret is `{name, param}`, not a name.** The host has to know where the value goes; only a
  query parameter is supported, not a header.
- **No `fetch` command.** 0012 had the host run a fetch command like any other tool. Two engine
  calls instead keep the reply out of the screen's actions and let a sync run with no screen
  asking for it.
- **Hosts are approved before the first fetch, not on load.** A pack that never syncs, or whose
  person never presses the button, never asks.
- **Automatic sync runs only while the app is open**, not on a background schedule. No
  WorkManager: a forecast older than the last time the app was opened is not missed.
- **`tag` is new.** It was not in 0012; without it a reply per place could not say its place.
- **No e2e flow.** The example pack stays offline, so a Maestro flow would need a server; the
  engine, FFI and view model tests cover the sync instead.

## Consequences

- `Module.from` is optional; a module with a `sync` and no `from` reads nothing from the pack.
- The FFI gains `Request`, `LoadedPack::hosts`, `requests` and `received`. `yaml::parse` reads a
  document that is one flow collection, which is what JSON is.
- The host command set adds `climate.sync`, and the app asks for the `INTERNET` permission.
