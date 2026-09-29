# 0017: kid mode, handoff, and host device commands

Status: accepted, 2026-09-28.

## Context

Phase 7 introduces device tools. Part 1 covers kid mode and handoff: when a child holds the phone,
the app should present child-friendly views, restrict exit from the app, restrict unsafe handoffs,
and sound an alarm when the child holds the phone in an unsafe context after any granted time has
expired. Returning the phone or granting extra time requires parent authentication.

## Decision

**Safe derive and later() function.** A pack derives `safe` (e.g. from `place.safe`, `here.safe`, or
safe transit/meal/lodging blocks) and `extended` (`now.stamp < store.holder_until`). The expression
grammar adds `later(stamp, minutes)` to compute future ISO timestamps without arbitrary arithmetic.

**Commands carry values.** Effects support `{do: name, with: {k: expr}}`, parsed into engine commands
carrying key-value arguments (`Command { name, args: Map }`). On Android via UniFFI, this maps to
`Command { name: String, args: Map<String, Value> }`.

**device.unlock command with then.** Actions requiring parental confirmation (like `give_back` or
`more_time`) invoke `{do: device.unlock, with: {then: "'returned'"}}` or `{then: "'extend'"}`. The host
prompts for biometric authentication or device credential (`BIOMETRIC_WEAK or DEVICE_CREDENTIAL`).
Upon success, the host dispatches the action named in `then`.

**Fallback when no device lock is set.** When `BiometricManager.canAuthenticate` indicates no lock or
credential is configured on the device (as on a clean development emulator), the prompt succeeds
immediately (`done(true)`), ensuring the interface does not lock out developers or users without PINs.
Any other reason the prompt cannot show refuses ([0023](0023-drift-review.md)).

**Handoff facts and storage.** The relay screen stores `returns_to` (the adult handing over),
sets `holder` to the chosen child, and clears `holder_until`. A child is only offered as holder if
`item.adult or safe`. Returning the phone restores `returns_to` as holder and clears `holder_until`.
Extending time sets `holder_until` to `later(now.stamp, 15)`.

**Screen alarm and host lock task.** When the screen tree specifies `kid: true`, the Android host
initiates screen pinning via `startLockTask()`, and clears it via `stopLockTask()` when `kid` is false.
The menu button is hidden during kid mode. When a screen specifies `Screen: {alarm: true}` (such as the
`complete` screen) and the previous view did not, the host vibrates the device and plays the default
notification sound.

## Deviations from the design

- **No guide script.** The child view presents points for kids directly from the place rather than an
  interactive guided script. Fixed in [0023](0023-drift-review.md).
- **No star.** The bookmark or completion star mechanism is deferred to a subsequent revision. Fixed in
  [0023](0023-drift-review.md).

## Consequences

- Commands are extensible data structures passed from engine to host, keeping the engine pure.
- Lock task and alarm behaviors remain host-managed effects triggered by declarative tree properties.
