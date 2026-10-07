# Connected GMCP slice

## Outcome

Connect the implemented generic GMCP codec to bounded TCP sessions through an
explicit `SessionOptions::MudClientGmcp { size }` profile. Preserve `connect`'s
default-deny behavior and the existing `MudClient` profile used by the desktop.
Core handshakes, package subscriptions/state, desktop IPC/UI, persistence and
live-server acceptance are outside this slice. No dependency changes are needed.

## Contract

- Reuse the existing Q-method owner. Passively accept remote option 201, refuse
  the local direction, and expose effective enablement plus an enable-generation
  counter in `OptionSnapshot`. Each re-enable advances the generation.
- Preserve raw `SessionEvent.event` delivery. Add `gmcp`, an optional parsed
  message/error on enabled GMCP subnegotiations, computed in wire order by the
  coordinator. Premature/disabled GMCP retains only its existing raw event.
  Errors reject one message without closing the session or affecting text.
- Add `Session::send_gmcp(&GmcpMessage).await`. Reject closed, disabled, invalid,
  or full-queue submissions. Serialize using the existing 64 KiB codec bound;
  use the same 32-entry command queue and FIFO writer as ordinary sends.
  Recheck enablement and captured generation in the coordinator before accepting
  the frame; reject stale submissions instead of replaying across copyover.
- Success acknowledges writer-queue acceptance, never socket delivery. Cancelling
  an acknowledgment wait does not retract a submitted command. Negotiations not
  yet read cannot revoke already-accepted sends. Cancellation/write failure must
  release pending acknowledgment waits and join both workers.
- Keep 128 inbound event slots, 32 writer slots, 4 KiB reads, and 16 KiB ordinary
  sends. GMCP commands carry up to 64 KiB. Account for up to 128 KiB plus 5 bytes
  per encoded GMCP frame and for a raw payload plus bounded parsed JSON per
  enabled inbound event; no secondary queue, cache or subscriptions are added.
- These are internal Rust contracts: struct literals for `OptionSnapshot` and
  `SessionEvent` and exhaustive `SessionOptions` matches need their new members.
  Frontend IPC and stored profiles remain unchanged.

## Completion gates

1. Verify passive negotiation, direction refusal, generations, duplicate requests,
   and unchanged existing profiles with deterministic protocol tests.
2. Exercise real loopback TCP: fragmented/interleaved messages, raw and parsed
   ordering, exact sends, maximum payload, malformed recovery, disable/re-enable,
   independent/replacement sessions, and clean disconnect/EOF.
3. Use controlled session I/O to prove stale-command rejection, shared queue
   pressure, lossless receive backpressure, cancellation under blocked output,
   partial-write timeout/error behavior, and worker cleanup.
4. Run focused tests, full default/profiles suites, formatting, warnings-denied
   Clippy, builds and API docs. Check desktop compilation for additive Rust API
   compatibility; no desktop behavior or native acceptance is claimed.
5. Update canonical contracts, bounds, status summaries and verification evidence;
   inspect all changed and untracked files, relative links and whitespace.

Completion requires evidence for every gate. No commit, push, live connection,
profile modification or native installation is part of this slice.

All five gates are complete. The [verification record](testing.md#connected-gmcp-verification-2026-10-06)
separates automated library/loopback evidence from deferred desktop and live-server
acceptance.
