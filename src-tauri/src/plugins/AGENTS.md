# Plugin host

## Scope

Own plugin discovery, compatibility checks, lifecycle, and host capabilities. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Treat all runtime options as proposals until implemented; keep host interfaces independent of a particular engine.
- Make permissions explicit for UI, commands, storage, events, and raw network hooks. Isolate plugin and session state.
- Matching Rust toolchains does not establish native ABI safety. WASM and scripts still need bounded execution, safe host imports, and cleanup.
- Reject incompatible plugins deliberately; unload or reload only when callbacks, tasks, and owned resources are quiescent.

## Verification

Cover denied capabilities, incompatible versions, callback failures, resource exhaustion, and repeated load/unload with disposable plugins.
