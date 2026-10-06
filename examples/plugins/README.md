# Plugin examples

These directories are documentation-only reservations for proposed plugin runtimes. There are no compilable examples, package manifests, host API bindings, installation commands, or supported compatibility versions yet.

| Directory | Proposed option | Key constraint |
|---|---|---|
| [rust](rust/AGENTS.md) | Native dynamic libraries | Privileged execution and explicit ABI/ownership contract |
| [wasm](wasm/AGENTS.md) | WebAssembly with scoped host imports | Engine limits and capability checks |
| [lua](lua/AGENTS.md) | Embedded Lua | Restricted libraries and bounded callbacks |
| [javascript](javascript/AGENTS.md) | Embedded JavaScript | Isolated host access and asynchronous cleanup |

Read the [plugin design](../../docs/plugins/design.md) before implementing an example. An example should teach one supported capability and document its permissions, interface/runtime compatibility, build or execution prerequisites, lifecycle, storage behavior, and cleanup.

Add runnable instructions only when the host and example manifests exist. Validate examples against the actual supported host, including denied operations and unloading where supported. Do not use live accounts or private server data to demonstrate plugin behavior.
