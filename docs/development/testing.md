# Verification strategy

## Current state

Executable unit tests for Telnet decoding, encoding, and Q-method negotiation, plus in-memory public-API wire integration tests, are configured through the `src-tauri/` Cargo package. There are no transport integration tests, frontend tests, application acceptance tests, fuzz targets, or CI runners yet. The broader verification strategy below remains a requirement for future features, not completed application testing.

## Implemented parser checks

The colocated `telnet/tests.rs` suite uses authored synthetic byte sequences, with no captured traffic, private transcripts, or live services. Follow the [library commands](setup.md#library-commands) for focused/full tests, formatting, Clippy, build, and documentation generation.

Coverage includes raw byte preservation and partial prompts; escaped IAC; standalone/unknown commands; all negotiation verbs and option bytes; repeated negotiation events; empty and escaped subnegotiations; and interleaved event ordering. Whole-stream, byte-at-a-time, every two-part split, and exhaustive partitions of short representative streams are compared after coalescing adjacent data events.

Boundary tests cover data-event size limits, large streams without retaining output, decoded payload limits with and without escaping, overflow, malformed sequences, EOF in each incomplete state, failure latching, finish/reset, and independent instances. Errors and preceding events are compared across chunk boundaries as well as successful results.

## Implemented encoding and negotiation checks

Colocated encoder tests check exact wire bytes, all-byte escaping, every command/option code, empty events, pre-output validation, payload limits, bounded output chunks, and mixed-event decoder round trips under varied chunking. Negotiator tests reach all six public Q states through real operations and check every receive/request transition in both directions against RFC 1143, plus default denial, separate allowlists, policy errors, resets, and instance isolation.

`src-tauri/tests/telnet_wire.rs` connects two synthetic peers through the public encoder → decoder → negotiator APIs. Tests cover mutual acceptance, asymmetric refusal, simultaneous complementary requests, queued reversals/cancellation, and delivery of other events to the caller. Message queues are capped at 32 entries and each settling phase at 64 exchanges; tests assert exact exchange counts, final states, and quiescence. No server or private transcript is involved.

Local automated verification is on macOS with Rust/Cargo 1.99.0. Windows/Linux execution, native Tauri acceptance, fuzzing, and live-server compatibility remain unverified. These tests establish generic wire behavior, not option-specific implementations, transport, text decoding, or rendering.

## Verification layers

| Layer | Location | Evidence required |
|---|---|---|
| Markdown review | Entire scaffold | Valid links, coherent instructions, accurate status, focused changes |
| Protocol and domain unit tests | Telnet decoder, encoder, and negotiator tests colocated with their modules | Observable bytes/events and state transitions for deterministic inputs |
| Backend integration | `src-tauri/tests/` | In-memory Telnet wire core implemented; future commands, sessions, plugins, and storage integration remain planned |
| Frontend checks | Alongside future frontend implementation | Rendering, interaction, accessibility, and bridge behavior |
| Application acceptance | `tests/e2e/` | Complete workflows using disposable data and local services |
| Parser fuzzing | `fuzz/fuzz_targets/` | Bounded runs, reproducible failures, and regression inputs |
| Native platform acceptance | Built application on each target OS | Real Tauri transport, UI, callbacks, persistence, and packaging behavior |
| Live-server acceptance | Explicitly authorized sessions | Server-specific behavior with redacted evidence and stated limits |

## Required behavior scenarios

- Protocol streams split at meaningful boundaries, escaped control bytes, incomplete prompts, unknown options, malformed or oversized payloads, and interleaved text/events.
- Multiple sessions, connection failure, rapid reconnect, stale callbacks, partial writes, queue pressure, and shutdown cleanup.
- Hostile markup, safe links/actions, Unicode, output bursts, keyboard navigation, resizing, scrollback, and password input handling.
- Plugin incompatibility, denied capabilities, runtime errors or traps, resource limits, and repeated load/disable/unload.
- First-run settings, restart persistence, corrupted or unwritable data, migrations, and interrupted writes.

## Execution and evidence

Use focused regression tests before broad checks. Read the actual manifests and configured CI before choosing commands; do not invent script names or assume optional tools are installed. Keep local emulators isolated, waits bounded, and cleanup reliable on both success and failure.

Fuzzing reports should record the toolchain, target, limits, corpus or seed, and minimized reproducer. Captured data must follow the [fixture conventions](../../resources/fixtures/README.md).

Browser-only tests cannot verify the native IPC bridge, system webview, filesystem permissions, or OS packaging. Driver support and native coverage must be established per platform. Live MUD access is opt-in and must not use credentials or accounts as automatic test fixtures.

## Documentation-only acceptance

Inspect all Markdown files, including newly created untracked files. Check local links and fragments, heading structure, unresolved reference-style citations, trailing whitespace, and instruction-chain size. Confirm every scaffolded directory has its scoped guide and all changes stay within the requested file types.

Report commands or checks actually run, their outcome, and omissions with reasons. Do not convert a checklist of future tests into a passing test report.
