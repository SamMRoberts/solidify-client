# Bounded TCP sessions

## Implemented scope

`solidify_client::sessions` connects the existing [Telnet wire core](protocols.md) to TCP using pinned [Tokio 1.53.2](https://docs.rs/tokio/1.53.2/tokio/). It is a library on the caller's runtime, which must have I/O and timers enabled and remain running through cleanup. No runtime or global session manager is created internally. Rust interfaces are initial internal contracts, not stable plugin APIs.

Only numeric `SocketAddr` endpoints are accepted. The session itself performs no text/ANSI decoding; callers may compose it with the independent [presentation decoder](protocols.md#presentation-decoder-contract). This module has no DNS, TLS, reconnect, retry/replay, idle timeout, half-open operation, arbitrary option handlers, persistence, or UI. The separate [application layer](desktop.md) adds DNS, decoding composition, and Tauri/frontend ownership without changing this API. Local tests do not establish public MUD compatibility or Windows/Linux runtime acceptance.

## Public API

| API | Contract |
|---|---|
| `connect(SessionId, SocketAddr, SessionConfig).await` | Returns `(Session, SessionEvents)` only after TCP connection succeeds. Invalid timeouts, connection timeout, and sanitized I/O failures return `ConnectError`. Dropping the connecting future cancels it without session workers. |
| `SessionId(u64)` | Caller-assigned identity; never reuse for concurrent or replacement sessions. There is no registry enforcing uniqueness. |
| `SessionConfig` | Positive `connect_timeout` and `write_timeout`, defaulting to 10 seconds and 5 seconds. Write timeout covers completing one frame, starting when the writer takes it, not time spent in queues. |
| `Session::try_send_data(&[u8])` | Copies and accepts at most 16 KiB into the command queue. `Ok(())` acknowledges acceptance only, not socket delivery. Empty data is an open-session no-op. `SendError` reports closed, full, or oversized input without copying it. |
| `Session::status()` | Snapshot of `SessionStatus { id, state }`; state is `Connected` or `Closed(CloseReason)`. Closure publication uses a separate watch channel, never the event queue. |
| `Session::closed().await` | Waits for termination and joins both workers. Cancelling this wait preserves their handles for another wait. |
| `Session::disconnect().await` | Signals cancellation and joins both workers. Idempotent; preserves an already-published terminal reason. Does not promise to flush accepted sends. |
| `SessionEvents::recv().await` | Returns ordered `SessionEvent { id, event }` values, then `None` after closure and queue drain. Cancelling this receive does not consume an event. |

Both public owners are non-cloneable. Dropping either signals cancellation; `Drop` cannot await cleanup. Keep the runtime running and await `closed()`/`disconnect()` on a retained session when a cleanup barrier is needed. Each connection gets fresh decoder, negotiator, and channels; old queued events keep their original identity after a replacement connection is created.

## Ordering and negotiation

One coordinator owns the read half, decoder, option handler with one Q-method negotiator, and application command processing. A separate writer owns the write half. All application data and generated option responses enter the same FIFO writer queue in coordinator processing order. A command racing with a socket read has no priority guarantee; once processed, their resulting frames cannot interleave. A caller must not equate a `try_send_data` return with completed processing or delivery.

For the unchanged default-deny API, incoming `WILL` generates `DONT`, and `DO` generates `WONT`, according to the existing Q method. Negative acknowledgments do not produce reply loops. Neither profile generates startup negotiation. `connect_with_options` offers the implemented MUD profile alongside default-deny. Negotiation and subnegotiation events are still forwarded, even for unsupported options, and all received data remains untrusted. A response is enqueued before its corresponding inbound event is delivered. Permitting an option in the standalone negotiator does not implement its semantics or change connected-session policy.

Data uses the existing encoder, doubling IAC and preserving every other byte without newline normalization or automatic terminators. The writer tracks each frame's offset across short writes. Cancellation or failure after a partial write ends the connection; no frame is restarted. This follows [Tokio's cancellation guidance](https://docs.rs/tokio/1.53.2/tokio/macro.select.html#cancellation-safety).

## Resource bounds and backpressure

| Resource | Fixed bound |
|---|---:|
| Read scratch buffer | 4 KiB |
| Application payload before escaping | 16 KiB |
| Application command queue | 32 entries |
| Encoded writer queue | 32 frames, each at most 32 KiB |
| Received-event queue | 128 events |
| Data event | 4 KiB |
| Decoded subnegotiation payload | 64 KiB |

The coordinator retains at most one read's decoded batch while delivering events. That batch can include a completed 64 KiB payload retained from earlier reads, alongside the current read's events. The decoder may also retain its own incomplete payload up to 64 KiB. Queue slots have allocator/type overhead; the table bounds content, not total process RSS. The received queue can therefore hold up to 128 payloads of 64 KiB, not merely 128 read buffers.

In addition to the queues, one command/encoded frame can be in coordinator processing and one frame in the writer. Encoding uses the existing fixed 4 KiB scratch buffer. No unbounded pending list or output queue is introduced. User-retained events and OS socket buffers are outside these library queue limits.

A full event queue pauses delivery and further reads; application commands may also wait. A full writer queue similarly pauses the coordinator. The independent writer continues while the consumer catches up. Normal backpressure preserves events and order. Cancellation and writer completion/failure use separate channels, and every blocking queue operation also observes termination.

## Termination and failure

EOF calls `TelnetDecoder::finish()`: a complete boundary reports `PeerEof`, while incomplete framing reports `Protocol(DecodeError::Truncated { .. })`. Malformed framing, payload overflow, read/write errors, and write deadline expiry terminate the entire session. Errors contain categories, control metadata, and I/O operation/error kind; they omit transcript data, endpoint addresses, and raw OS error strings.

Exactly one terminal status is published. Normal termination cancels the other worker, releases both socket halves, and ends event production. `closed()` and `disconnect()` join both task handles. The coordinator guards the writer with abort-on-drop ownership, so an unexpected coordinator exit cannot orphan it; unexpected worker failure reports `WorkerFailed`. The caller's runtime must remain available to perform asynchronous destruction.

Already-enqueued inbound events remain drainable. Undelivered events from the current batch, queued commands, and unsent frames may be discarded on termination, including a batch containing a framing error. Failure does not replay bytes or roll back bytes already sent to the peer. Racing terminal causes have no guaranteed precedence; the published reason remains final.

See [testing evidence and limits](../development/testing.md#implemented-session-checks) and [setup commands](../development/setup.md#library-commands).

## Opt-in terminal option profile

`connect_with_options(id, address, config, SessionOptions::MudClient { size })`
shares the original connection and cleanup contracts. `connect` still chooses
`DenyAll`; existing `SessionConfig` fields and raw event delivery are unchanged.

`Session::subscribe_options()` returns a Tokio watch receiver of the fixed
`OptionSnapshot`: TTYPE, NAWS, remote ECHO, local/remote SGA, and masking generation.
The coordinator publishes changes before awaiting response/event queue capacity.
A subscriber can therefore observe processed negotiations despite downstream
pressure. Unread negotiations behind transport backpressure remain unread.
The final snapshot remains available after closure; a new session starts at zero.

`Session::update_viewport(TerminalSize)` replaces one watch value and returns
`SendError::Closed` after observed cancellation/closure. Intermediate sizes may
coalesce; success is not wire delivery. A blocked coordinator processes the latest
size when it resumes. Responses use the existing FIFO writer with the same write
deadline and partial-write termination rules. One receive stages at most two
responses (a three-byte acknowledgment and at most thirteen-byte NAWS frame);
TTYPE frames are fourteen bytes. No new unbounded queues are introduced.
