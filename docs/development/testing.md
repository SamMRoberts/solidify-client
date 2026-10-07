# Verification strategy

## Current state

Executable unit tests for Telnet decoding, encoding, Q-method negotiation, and UTF-8/ANSI presentation decoding, plus deterministic session I/O tests, in-memory public-API wire tests, and TCP loopback tests, are configured through the `src-tauri/` Cargo package. The application coordinator has injected-DNS and loopback tests; the React frontend has Vitest/jsdom tests. Browser and native macOS acceptance use the loopback demo. There is no automated native driver suite, fuzz target, or CI runner; broader future-feature scenarios below remain requirements rather than passing evidence.

## Implemented parser checks

The colocated `telnet/tests.rs` suite uses authored synthetic byte sequences, with no captured traffic, private transcripts, or live services. Follow the [library commands](setup.md#library-commands) for focused/full tests, formatting, Clippy, build, and documentation generation.

Coverage includes raw byte preservation and partial prompts; escaped IAC; standalone/unknown commands; all negotiation verbs and option bytes; repeated negotiation events; empty and escaped subnegotiations; and interleaved event ordering. Whole-stream, byte-at-a-time, every two-part split, and exhaustive partitions of short representative streams are compared after coalescing adjacent data events.

Boundary tests cover data-event size limits, large streams without retaining output, decoded payload limits with and without escaping, overflow, malformed sequences, EOF in each incomplete state, failure latching, finish/reset, and independent instances. Errors and preceding events are compared across chunk boundaries as well as successful results.

## Implemented encoding and negotiation checks

Colocated encoder tests check exact wire bytes, all-byte escaping, every command/option code, empty events, pre-output validation, payload limits, bounded output chunks, and mixed-event decoder round trips under varied chunking. Negotiator tests reach all six public Q states through real operations and check every receive/request transition in both directions against RFC 1143, plus default denial, separate allowlists, policy errors, resets, and instance isolation.

`src-tauri/tests/telnet_wire.rs` connects two synthetic peers through the public encoder → decoder → negotiator APIs. Tests cover mutual acceptance, asymmetric refusal, simultaneous complementary requests, queued reversals/cancellation, and delivery of other events to the caller. Message queues are capped at 32 entries and each settling phase at 64 exchanges; tests assert exact exchange counts, final states, and quiescence. No server or private transcript is involved.

Local automated verification is on macOS with Rust/Cargo 1.99.0. Windows/Linux execution, fuzzing, and live-server compatibility remain unverified. Native macOS acceptance is recorded separately below. Protocol tests establish generic wire behavior; session tests add deterministic I/O and local TCP evidence. Separate presentation tests establish the documented text/style decoding subset. Option-profile tests establish only the implemented TTYPE/NAWS/ECHO/SGA subset. Protocol/library checks do not establish rendering or public-server compatibility; frontend and native rendering checks are separate.

## Implemented presentation checks

Colocated `presentation/tests.rs` tests compare whole-stream, every two-part
split, bytewise input, and exhaustive partitions of short streams after merging
adjacent text events. Unicode cases include invalid leads, overlong/surrogate/
out-of-range encodings, interrupted and truncated prefixes, and an exhaustive
two-byte ordinary-input comparison with Rust's lossy UTF-8 conversion (excluding
the documented discarded controls).

Style tests cover all basic colors and resets, independent flags, atomic mixed
SGR rejection, empty fields, leading zeroes, and redundant transitions. Other
tests cover unsupported ESC/CSI, all five discarded string kinds, opaque payloads,
both OSC terminators, exact bounds/overflow, embedded controls, scalar boundaries,
replacement expansion, lifecycle, and independent instances. Large-stream checks
use counting callbacks without retaining output.

The added `tcp_sessions.rs` presentation tests explicitly compose session data
with the public decoder. They verify partial prompts before newline/EOF, fragmented
Unicode and SGR across separately handled Telnet events, caller-owned EOF/failure
handling, and independent concurrent sessions. They retain bounded waits and
verify session/socket cleanup. All 66 pre-presentation tests remain intact.

## Implemented session checks

`src-tauri/src/sessions/tests.rs` uses private generic I/O adapters for one-byte writes, stalled partial writes, write-zero and I/O failures, and worker failure. Tokio's controlled clock verifies connection and whole-frame write deadlines without unreliable external endpoints. Queue tests fill the command, writer, and event paths, verify read suspension, recover without loss/reordering, and interrupt pressure with disconnect, consumer drop, or writer failure. Resource probes and retained task handles verify that awaited shutdown releases I/O and joins workers, including after unexpected coordinator exit.

`src-tauri/tests/tcp_sessions.rs` uses ephemeral IPv4 loopback listeners and the public API. It covers connection success/refusal, fragmented controls, partial prompts, raw bytes and IAC escaping, exact send limits, ordered negotiation refusals, clean EOF and every incomplete decoder state, malformed/oversized framing, multiple sessions, replacement identities, and owner/consumer drop. Server sockets are driven by test-owned futures, with no detached server tasks; every wait is bounded and unwinding releases sockets/listeners. Any future spawned test server must be explicitly stopped and joined on success and failure.

The original 41 protocol tests remain intact. Loopback TCP evidence and deterministic adapter/clock evidence are separate from native application and public-server acceptance. Current execution is macOS only. See the [session contracts](../architecture/sessions.md) for queue sizes and the [setup commands](setup.md#library-commands) for dependency fetching and focused checks.

## Implemented desktop checks

Coordinator tests cover endpoint validation, immediate start IDs, concurrent/stale
requests, injected DNS errors and empty/capped/duplicate results, ordered address
fallback, overall DNS deadline, cancellation with a retained lookup slot, numeric
bypass, EOF replacement, presentation failure, event/text pressure, poll limits,
concurrent polls, exact command limits, queue-full rejection, and shutdown. A task
panic test verifies that the session guard transfers ownership for awaited cleanup.
Per-candidate connection timers use the existing session timeout implementation,
whose deterministic clock tests remain intact. All 94 pre-desktop tests are retained.

Frontend tests cover CRLF and standalone CR across batches, grapheme deletion
across styles, tabs, bell, prompts, style-preserving clear, scalar-safe long-line
breaks, line/byte/run retention, literal hostile markup, draft acceptance and
rejection, masking, command bounds, focus, replacement/unmount cleanup, stale poll
responses, single-poll delivery, and scroll-follow behavior. jsdom checks use
mocked bridges and dimensions; they do not prove native IPC or browser geometry.

Use the [configured desktop checks](setup.md#desktop-and-frontend-commands) and
[manual checklist](setup.md#manual-desktop-checklist). Record browser, native, and
local transport observations separately. The demo is development-only and uses
synthetic text, one active client, bounded input, and loopback TCP.

## Fifth-slice desktop acceptance evidence — 2026-10-06

- **Automated macOS:** 106 library tests passed; the desktop-feature suite passed
  107 tests, including the command-origin check. All original 94 tests and their
  source files are unchanged. The 16 Vitest tests passed. TypeScript, Prettier,
  Rust formatting, Clippy with warnings denied, library/desktop builds, API docs,
  production frontend/native build, and Markdown/diff checks passed.
- **Browser:** Codex IAB showed the compact toolbar, transcript and command layout
  at the ordinary viewport and 720×480. DOM bounds showed no horizontal overflow
  at the minimum size. Port validation, mask toggle and focus styling were checked;
  no browser console errors were observed. No browser TCP/IPC success is claimed.
- **Native macOS:** The bundled production app displayed fragmented ANSI/Unicode,
  interleaved Telnet negotiation, and a prompt before newline. Enter and Send,
  retained keyboard focus, masking, colors/style flags, CR/backspace/tab behavior,
  literal markup, burst output, scroll hold/Latest output, clear, malformed ANSI,
  refused local connection, clean EOF, disconnect and reconnect were exercised.
  Window close ended the native process and the demo accepted a fresh client.
  Window zoom/restoration worked; minimum-size geometry was verified in the browser.
  The documented `tauri:dev` command also started Vite and its native process;
  the complete UI workflow above was checked in the bundled production build.
- **Unverified:** Windows/Linux runtime, installers/signing, public MUDs,
  option-specific behavior, persistence, and plugins. No live credentials or
  captured server transcripts were used. These observations are slice acceptance,
  not a cross-platform or full-terminal compatibility claim.

## Sixth-slice verification — 2026-10-06

- **Automated macOS:** 115 library-suite tests and 116 desktop-feature tests passed,
  retaining all prior tests. The additional demo example test passed through real
  loopback TCP and the application coordinator. All 23 frontend tests passed.
  Formatting, type checking, Clippy with warnings denied, library/desktop builds,
  API documentation, the production Tauri build, and whitespace/diff checks passed.
  All 99 relative Markdown links and heading targets passed inspection. No
  dependencies changed.
- **Option and loopback evidence:** tests cover exact TTYPE/NAWS responses, IAC
  escaping, fragmented Unicode with negotiation, direction refusal, duplicate
  negotiation, disable/re-enable, independent sessions, latest viewport delivery,
  processed masking state during full output queues, stalled-writer cancellation,
  stale IDs, and shutdown. The demo test checks identity, dimensions, masking
  transitions, discarded synthetic input, re-enable after resize, and clean EOF.
- **Browser:** Playwright Chromium checked `http://localhost:1420` at 1100×760 and
  720×480. Page identity, nonblank layout, no error overlay, no console errors,
  manual masking, and no horizontal overflow passed. A temporary test-only IPC
  fixture additionally exercised automatic masking, protection after server reset,
  retained drafts after synthetic queue rejection, successful-send clearing, and
  measured viewport updates. The masked/error layout also fit 720×480. These are
  rendering checks, not native IPC or TCP evidence. The Browser plugin was absent;
  Chromium required execution outside the sandbox after a macOS permission error.
- **Native limitation:** `npm run tauri:dev` started Vite and the native executable,
  but macOS UI automation reported a locked Mac on both attempts. Native window
  interaction, IPC-driven masking/resizing, reconnect, and window-close cleanup
  remain unverified for this slice. The earlier native record above does not
  establish acceptance of these changes. Use the [compatibility checklist](setup.md#telnet-compatibility-checklist)
  after unlocking the desktop. The development app/server processes started for
  this check were stopped; process termination is not window-close acceptance.
- **Unverified:** Windows/Linux runtime, public MUDs, and all deferred protocols.
  No live-server connections or real credentials were used.

## Seventh-slice verification — 2026-10-06

- **Automated macOS:** all 115 ordinary library-suite tests remain passing. The
  non-native `profiles` suite passes 126 tests and the desktop suite 128, including
  strict profile DTO checks. The demo's real-loopback example test also passes.
  All 40 frontend tests pass. TypeScript, Prettier, Rust formatting, Clippy with
  warnings denied for all three feature configurations, library/profile/desktop
  builds and API documentation, and the configured production Tauri build passed.
  All 107 relative Markdown links/heading targets and final whitespace/diff review
  passed, including newly added files.
  Cargo suites used locked/offline resolution; serde_json reuses the lockfile's
  1.0.151 version. No other dependencies or prior tests were removed.
- **Storage evidence:** temporary-directory tests cover first run, CRUD/round trips,
  stable counters, exact record/name/file limits, invalid colors/endpoints/IDs,
  duplicate names/JSON fields, unknown fields/versions, unreadable destinations,
  ignored interrupted temporary files, injected create/write/flush/sync/replacement
  failures, lock contention, explicit retry, bounded admission and shutdown waiting
  for an accepted write. They verify preservation of the old file and published
  state after failed commits. Power-loss filesystem durability is not simulated.
- **Frontend evidence:** tests cover selection restoration, Custom behavior, late
  responses after edits/connection intent, CRUD and duplicate names, explicit retry
  after failed preference writes, older-success/newer-failure ordering, failed
  editor/appearance saves, deletion,
  active-session restrictions, valid preview/invalid draft/Cancel/Reset, transient
  Custom appearance, unchanged ANSI classes and inverse defaults, visible-line
  anchoring and font-driven debounced viewport delivery. Existing transcript bounds
  and masking tests remain intact.
- **Browser:** Playwright Chromium at `http://localhost:1420`, 1100×760 and 720×480,
  passed page identity, meaningful content, no framework overlay, no console errors,
  no horizontal overflow, and screenshot inspection. A temporary test-only IPC
  fixture exercised create/edit/delete, connected restrictions, live font/color
  preview and Cancel, explicit ANSI/inverse computed colors, scroll anchoring,
  measured viewport invocations, masked-draft retention and synthetic send failure.
  The real frontend without IPC displayed its storage warning and manual controls.
  Browser plugin not available; Playwright used an existing installation and needed
  sandbox escalation for macOS Chromium launch. Mocked bridge evidence does not
  establish native persistence or transport behavior.
- **Native macOS:** initial access was blocked by the locked Mac; later interaction
  succeeded using a separately identified production acceptance bundle and a fresh
  `/tmp` configuration directory. A named localhost:4001 profile saved through real
  IPC, appeared in the versioned file, and restored after File → Close Window and
  relaunch without connecting. Live Save changed retained/incoming text from 18 to
  24 px and changed default text color; Reset preview/Cancel restored the applied
  appearance without clearing output or exposing a protected draft. Real `protocol`
  replies identified SOLIDIFY and changed from 95×16 to 71×12 after font change,
  and to 113×22 after window zoom. Options-off reported unavailable values;
  options-on restored identity and the latest measured dimensions.
- **Native masking/cleanup:** the real demo's mask request changed the field to
  secure input. A synthetic control-character command failed validation while
  retaining the masked draft; an accepted replacement received only the fixed
  discarded-input acknowledgment and restored normal entry. Manual mask on/off
  protected a nonempty draft across appearance Cancel. Disconnect/reconnect gave
  fresh output and unmasked entry. File → Close Window immediately following burst
  requests exited the acceptance process and released its profile lock; relaunch
  restored saved appearance and the demo accepted a fresh connection. This completes
  the sixth slice's previously blocked native masking, resize, reconnect and close
  checks. Queue-full/transient server-toggle cases retain automated/browser evidence.
- **Remaining limits:** Windows/Linux runtime, public MUDs, installers/signing,
  crash/power-loss durability and native multi-instance/corrupt-file recovery UI
  remain unverified. Lock contention and corrupt-file recovery have automated
  storage coverage and a manual checklist. Only synthetic local data was used;
  the test app, development server and demo were stopped after verification.

## Eighth-slice verification (2026-10-06)

- **Automated Rust:** locked/offline ordinary suite **122 passed** (105 unit,
  13 TCP, 4 wire); `profiles` **133 passed**; `desktop` **136 passed**, including
  three command-boundary tests. Both demo-example tests passed. Existing tests
  remain; unsupported-color fixtures now use values outside the expanded subset.
  New coverage includes all indexed values, RGB boundaries and syntax variants,
  splits/bytewise/exhaustive short partitions, atomic rejection, exact limits,
  typed IPC, real fragmented TCP with negotiation, and real demo/application output.
- **Automated frontend:** **45 tests passed**. Numeric style equality, fixed palette,
  RGB and inverse, profile defaults, hostile values/markup, clear and retention
  limits are covered. TypeScript, Prettier, Vite build, Cargo formatting, all-target
  warnings-denied Clippy, builds and API docs passed for ordinary, profiles and
  desktop feature configurations. The production Tauri macOS app build passed.
- **Browser:** installed Playwright Chromium at `http://localhost:1420`, using a
  temporary test-only IPC fixture, passed 1100×760 and 720×480 checks. The Browser
  plugin was unavailable; sandboxed Chromium could not register its Mach port, so
  the approved unsandboxed run was used. Page identity, meaningful content, absence
  of a framework overlay, console health, screenshots, RGB/indexed inverse colors,
  profile preview/Cancel, clear and fresh-ID reconnect passed. The fixture was
  corrected to match real connection IDs, control names and bounded poll batches;
  no production bridge mocks were added. These are browser rendering checks, not
  transport evidence.
- **Native macOS:** a separately identified production `.app` used a fresh `/tmp`
  configuration directory and the real loopback demo on port 4001. `colors`
  displayed the 256 swatches, bright samples, fragmented RGB orange/blue background,
  inverse and reset. Changing default text/background preserved explicit colors.
  Clear followed by fresh color output and disconnect/reconnect passed. File →
  Close Window exited the native process, removed the TCP connection, and released
  the disposable profile lock. No real user profiles or public MUDs were used.
- **Review and limits:** relative Markdown links/headings, whitespace, instruction
  consistency and the complete diff including new files were reviewed. No new
  dependencies, profile schema, capabilities, transport APIs or terminal identity
  were introduced. `TextColor` gains indexed/RGB variants, requiring exhaustive
  internal consumers to handle them. Windows/Linux runtime and public-server
  compatibility remain unverified; full terminal emulation is outside this slice.

## Verification layers

| Layer | Location | Evidence required |
|---|---|---|
| Markdown review | Entire scaffold | Valid links, coherent instructions, accurate status, focused changes |
| Protocol and domain unit tests | Telnet decoder, encoder, and negotiator tests colocated with their modules | Observable bytes/events and state transitions for deterministic inputs |
| Backend integration | `src-tauri/tests/` | In-memory Telnet wire core and loopback TCP sessions implemented; application coordinator loopbacks are colocated under `src/application`; storage uses colocated temporary-directory tests; plugins remain planned |
| Frontend checks | Colocated Vitest tests under `src/` | Rendering, interaction, accessibility, and bridge behavior |
| Application acceptance | `tests/e2e/` | Manual checklist with the Rust loopback demo; no automated native driver suite |
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
