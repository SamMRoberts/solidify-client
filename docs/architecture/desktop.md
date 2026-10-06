# Desktop application contracts

## Implemented scope

The first desktop slice uses Tauri 2.12.1, React 19.3.0, TypeScript 7.0.2,
Vite 8.3.3, and npm with tracked lockfiles. One main window owns one connection
at a time. The optional Cargo `desktop` feature enables the native entrypoint,
Tauri build support, and serialization. The ordinary library retains its
[protocol](protocols.md) and numeric-address [session](sessions.md) APIs.
The application coordinator uses existing Tokio facilities and is testable
without Tauri or a native webview.

## Ownership and delivery

`application::Application` starts work on the caller's Tokio runtime, assigns
monotonically increasing IDs, and rejects concurrent starts until the previous
worker has finished. IDs cross IPC as decimal strings to avoid JavaScript number
precision loss. Stale IDs cannot poll, send, or disconnect a replacement.
A fresh connection receives a new decoder and transcript. There is no automatic
connection, reconnection, command retry, or replay.

Host input is at most 253 ASCII bytes: an IPv4/IPv6 literal (IPv6 brackets are
accepted) or hostname with nonempty labels of at most 63 bytes. DNS labels use
ASCII letters, digits and interior hyphens; a trailing dot is permitted.
URLs, paths, whitespace and non-ASCII hostnames are rejected. Port is 1–65535.
Numeric addresses bypass DNS. Hostnames resolve through Tokio's OS resolver;
up to eight distinct addresses are tried in resolver order. One ten-second
deadline covers resolution and connection, and each TCP candidate gets at most
two seconds within it.

One DNS lookup may remain outstanding. Cancellation and the overall deadline
stop waiting immediately and ignore late results, but the resolver slot remains
occupied until the underlying OS lookup finishes. OS resolution cannot be
forcibly cancelled. A new hostname attempt during that time returns a sanitized
busy message; numeric connections remain available. Shutdown joins session work
without waiting indefinitely for OS resolution.

The coordinator passes only Telnet data into a connection-local presentation
decoder. Negotiations remain owned by the default-deny session library; other
Telnet events do not interrupt presentation state. After EOF it drains session
events and finishes decoding. Presentation failure preserves preceding output,
reports only the typed error description, and disconnects the session.
A supervisor retrieves the owned session even on an unexpected task exit and
awaits cleanup before marking the worker finished.

| Boundary | Limit and behavior |
|---|---|
| Pending application output | 1,024 events and 256 KiB of text |
| Decoded staging | At most one 4 KiB Telnet data event's decoded output; no further input is consumed while staging is pending |
| Poll response | At most 256 events and 64 KiB of text |
| Empty poll | Waits up to 100 ms; one outstanding poll per connection |
| Application send requests | Eight waiting requests, each at most 16 KiB including CRLF; session queues retain their existing limits |
| Retained transcript | 2,000 logical lines, 1 MiB UTF-8 text, 10,000 styled runs |
| Current logical line | Forced break before exceeding 16 KiB; never split a scalar |

Queue limits bound contents, not allocator overhead or OS buffers. One poll
response may be in transit while the backend queue refills. Full output queues
pause event consumption; status and cancellation use separate paths. The browser
schedules the next poll through `requestAnimationFrame` only after the previous
one returns. A bridge failure stops the chain and requests disconnect. If the
bridge itself is unavailable, window close/application exit remains the native
cleanup path. Late responses are ignored after replacement or unmount.

Closed transport status can appear while buffered output is still draining.
`finished` becomes true in a poll only after worker cleanup and empty output.
The UI waits for this before enabling a new connection. Explicit disconnect or
window close may discard the one staged chunk and unread session events; already
queued application output remains available until replacement or exit. These
user cancellations do not fabricate EOF text or promise to drain unseen data.

## Commands and trust boundary

Four commands exist: `start_connection`, `poll_connection`, `send_line`, and
`disconnect`. Every command verifies the `main` window and bundled application
origin; debug builds also permit the configured localhost Vite origin. Dedicated
DTOs serialize text, style snapshots, controls, statuses and IDs without adding
serialization to protocol-domain types. Error messages contain no command text,
transcript, hostname, raw OS error, or credentials.

The main capability grants no core/plugin permissions. Custom application
commands are guarded explicitly; empty plugin capabilities alone do not authorize
or restrict these commands. There are no filesystem, shell, opener, remote
content, or plugin capabilities. Production loads bundled frontend content with
a restrictive CSP. The development CSP additionally permits Vite's localhost
WebSocket and style injection. See Tauri's [command guide](https://v2.tauri.app/develop/calling-rust/)
and [capability boundary](https://v2.tauri.app/security/capabilities/).

Text is rendered as React text nodes with classes derived from typed styles.
It is never HTML, a navigable link, or executable markup. The client implements
no input/transcript logging, local echo, history, credential storage or profiles.
The manual mask toggle only obscures the command field visually; transport is
plain TCP and automatic password detection is deferred.

## Transcript and input behavior

CRLF commits one line, including across batches. LF commits the current line.
A standalone CR leaves the line visible until subsequent text (or a tab) replaces
it; intervening style changes do not clear the pending CR. Backspace deletes the
last extended grapheme on the current line using `Intl.Segmenter`, including
across style runs. It does not cross a committed or forced line break. Tabs expand
to the next eight-grapheme stop. Bell briefly highlights the output boundary.
Unicode normalization and terminal cell-width calculations are not performed.

Adjacent equal styles coalesce. Retention evicts oldest completed lines. A single
line containing 10,000 alternating style runs is also force-broken before a new
run, allowing completed content to be evicted. Clear removes displayed text and
pending line controls while preserving the decoder's active style. New connections
reset both style and content. Output follows the bottom only when the reader was
already there; otherwise the visible line is anchored where retained and a
“Latest output” button returns to the bottom. If an anchor is evicted, older text
cannot be preserved beyond the retention limit.

Enter or Send accepts one UTF-8 line, including an empty line and ordinary edge
whitespace, and appends CRLF. All Unicode control characters are rejected. The
16 KiB limit includes CRLF and is checked before the backend copies the command.
The draft clears only after the session queue accepts it; acceptance is not a
network delivery acknowledgement. Queue-full and validation failures retain the
draft. Nothing is retried automatically.

## Deferred work

Multiple sessions, saved profiles, TLS, option handlers, automatic password mode,
persistence, plugins, extended colors, cursor addressing, screen editing,
terminal emulation, installers, signing, and CI remain deferred. The local demo
and macOS checks do not establish Windows/Linux or public-MUD compatibility.
See [setup and manual checks](../development/setup.md#manual-desktop-checklist).
