# Plugin system design

## Status and extension points

The plugin system is proposed. No runtime, package format, host API, ABI, or compatibility version is implemented or selected.

The intended host should support UI panels and notifications, pre-command input hooks, post-text output hooks, custom commands, event subscriptions for connection/text/lifecycle events, plugin-scoped persistence, and explicitly privileged raw Telnet preprocessing.

Define ordering, cancellation, error handling, and ownership before exposing each hook. UI integrations should use constrained host-provided presentation capabilities. Plugin code must not inherit the application's privileged frontend bridge.

## Runtime alternatives

| Option | Performance and footprint | Isolation and compatibility | Authoring and lifecycle |
|---|---|---|---|
| Rust dynamic library | Native execution; footprint depends on linking and dependencies | Full host-process privileges; platform/architecture-specific ABI and ownership contract required | Rust/native build expertise; reload limited by safe quiescence and unload |
| WebAssembly, potentially Wasmtime/WASI | Runtime overhead and module size depend on workload and engine | Sandbox plus explicitly granted imports; interface and target compatibility still required | Cross-platform guest artifacts are possible; traps and resource limits need host handling |
| Embedded Lua | Interpreter/runtime footprint and workload-dependent cost | Host must restrict libraries, imports, execution, and resources | Accessible scripting; handlers, timers, and state still require lifecycle management |
| Embedded JavaScript | Cost depends on selected engine | Execution environment must isolate plugins from trusted frontend capabilities | Familiar scripting; asynchronous tasks and subscriptions complicate disable/reload |

Do not promise benchmark results, binary sizes, or portability before measurement. Examples for these alternatives have documentation-only directories in [examples/plugins](../../examples/plugins/README.md).

## Native ABI constraints

Matching Rust compiler versions alone does not establish safe ABI compatibility. The [Rust Reference](https://doc.rust-lang.org/reference/type-layout.html) describes limited layout guarantees; its [external ABI documentation](https://doc.rust-lang.org/reference/items/external-blocks.html) is relevant to boundary design.

Before implementing native loading, define ABI versioning, representation, memory allocation/deallocation ownership, panic behavior, callback lifetimes, and target compatibility. An incompatible library must be rejected before use. Native plugin manifests cannot sandbox native code.

## Isolation and lifecycle requirements

Grant only required host capabilities and isolate plugin data by plugin identity and session. JSON files or SQLite remain possible persistence choices, not a selected schema.

For WASM, review [Wasmtime security](https://docs.wasmtime.dev/security.html). Sandbox protection does not remove the need for safe host functions, execution budgets, memory limits, and restricted filesystem/network access. Script engines likewise need a deliberately restricted environment; the selected runtime's standard library is not automatically an approved capability set.

Loading, enabling, disabling, and unloading must have explicit ownership and cleanup. Stop callbacks, cancel outstanding work, release handlers and timers, and settle persistent state before replacing an instance. WASM or scripting support alone does not guarantee safe hot reload.

## Compatibility and validation

Define a versioned host contract when implementing the first supported runtime. Document breaking changes, migration requirements, and supported examples together. A template generator or catalog should follow working examples and a stable contract; no unresolved macro citation establishes an API.

Test normal and denied operations, incompatible interfaces, failures, resource exhaustion, and repeated lifecycle transitions. Native execution and engine isolation require their own observed evidence, not just example compilation.
