# solidify-client

solidify is a planned cross-platform MUD client built around Rust and Tauri for Windows, Linux, and macOS.

## Project status

The implemented Rust library provides bounded Telnet decoding/encoding, configurable RFC 1143 Q-method negotiation, TCP sessions on a caller-owned Tokio runtime, and independent streaming UTF-8/basic ANSI presentation decoding. Sessions deny all options, preserve raw bytes, and bound queues and shutdown work. Callers feed only Telnet data into presentation decoding for text, style, and control events. Protocol processing remains independent of Tokio; the crate pins Tokio 1.53.2. See the [protocol contracts](docs/architecture/protocols.md) and [session API and limits](docs/architecture/sessions.md).

There is no launchable application, Tauri initialization, frontend package, installer, or automated workflow yet. The broader client remains planned.

## Run the library checks

With Rust/Cargo 1.99.0, run from the repository root:

```sh
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
```

See [development setup](docs/development/setup.md) for library build, formatting, lint, and documentation commands. These checks do not establish native application or live-server compatibility.

## Planned capabilities

- Option-specific Telnet behavior, DNS/TLS, extended ANSI styling, terminal rendering, and MXP markup.
- GMCP, ATCP, and MSP extensions with session-scoped state and safe rendering.
- An accessible terminal interface and persistent user settings.
- A plugin system with proposed Rust dynamic-library, WebAssembly, Lua, and JavaScript options.
- Cross-platform verification, packaging, release management, and documentation automation.

These are design intentions, not a claim of protocol compliance or available plugin support. Tauri/frontend versions, frameworks, plugin engines, and external interfaces remain unselected. The current Rust interface is an internal project contract, not a stable plugin API.

## Repository layout

The Rust library and its colocated protocol and session unit tests live under `src-tauri/`, the future Tauri backend directory. Public-API wire and loopback TCP integration tests live under `src-tauri/tests/`. Frontend responsibilities are reserved under `src/`, application acceptance under `tests/`, parser fuzzing under `fuzz/`, and plugin examples under `examples/plugins/`. Those reserved areas contain guidance only.

The remaining directories cover tooling (`scripts/`), GitHub collaboration (`.github/`), development environments (`.devcontainer/`), documentation, and reference resources. Their contents are Markdown only.

## Start here

- [Documentation index](docs/README.md)
- [Architecture and data flow](docs/architecture/overview.md)
- [Development setup and current limitations](docs/development/setup.md)
- [Contribution guide](CONTRIBUTING.md)
- [Repository agent instructions](AGENTS.md)
- [Resources and primary references](resources/README.md)
