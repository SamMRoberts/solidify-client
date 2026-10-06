# Development setup

## What works today

The single Cargo package in `src-tauri/` builds a Rust library for Telnet decoding, encoding, Q-method negotiation, and bounded TCP sessions. It runs deterministic unit tests, in-memory peer tests, and ephemeral loopback TCP tests. Its manifest and lockfile are tracked. No launchable Rust/Tauri application, frontend package, development container, or CI workflow exists.

The library uses Rust edition 2024 and declares Rust 1.99 as its minimum baseline. The initial local verification toolchain is Rust/Cargo 1.99.0 on macOS, with rustfmt and Clippy installed. No Rust toolchain manager configuration is added; use that toolchain for reproducible checks. Windows and Linux execution remain unverified.

Tokio is pinned to `=1.53.2`, with `rt`, `net`, `io-util`, `sync`, `time`, and `macros`; tests also enable `test-util`. The caller supplies a running Tokio runtime with I/O and timers enabled. The protocol modules remain independent of Tokio. No Tauri SDK, native webview dependencies, Node.js, credentials, or profiles are required. Do not run a project generator or install application dependencies solely from the directory scaffold. There is no launch command.

## Library commands

Run these commands from the repository root. Fetch the pinned dependencies first (network access and a writable Cargo cache are required). Subsequent checks use locked, offline resolution; an empty cache cannot run them until the fetch succeeds. Loopback tests need permission to bind local sockets, but no external server or configuration.

```sh
# Fetch Tokio and its locked transitive dependencies before offline checks.
cargo fetch --manifest-path src-tauri/Cargo.toml --locked

# Protocol unit tests, including the existing decoder tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet

# Focused outbound encoding and negotiation checks.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet::encoder
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet::negotiation

# Public-API in-memory peer integration tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --test telnet_wire

# Session unit tests and public-API TCP loopback tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline sessions::tests
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --test tcp_sessions

# Full unit/integration suite and doctests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline

# Read-only formatting check and lint check.
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings

# Compile the library and generate its API documentation.
cargo build --manifest-path src-tauri/Cargo.toml --locked --offline
cargo doc --manifest-path src-tauri/Cargo.toml --locked --offline --no-deps
```

`cargo fmt --manifest-path src-tauri/Cargo.toml` applies Rust formatting when needed. Build output and generated documentation are under the ignored `src-tauri/target/` directory; the documentation entrypoint is `src-tauri/target/doc/solidify_client/index.html`. These commands compile a library, not an application binary or installer.

If Cargo reports an unsupported Rust version, check `rustc --version` and `cargo --version` against the baseline. If formatting or lint tools are missing, that check cannot be reported as passing. Neither issue requires initializing Tauri or a frontend. Tests create and clean up their own local listeners; no manual server or disposable profile setup is needed.

## Future bootstrap prerequisites

When desktop implementation is authorized, select and record compatible Tauri, frontend tooling, and package-manager versions alongside the Rust baseline. Establish their manifests and lockfiles before documenting installation commands.

Tauri development needs Rust and platform-specific native dependencies. A JavaScript frontend toolchain may additionally need Node.js and its chosen package manager. Check the [official prerequisites](https://v2.tauri.app/start/prerequisites/) for the selected Tauri release and host platform. Container-based work will not replace native verification on all target systems.

The future desktop setup guide must additionally describe:

- Supported development hosts, required native dependencies, and pinned project tool versions.
- Exact install, development, test, lint, formatting, and build commands with working directories.
- Environment variables by purpose, without secrets or private endpoints.
- Local test-server and disposable-profile setup.
- Common failures, cleanup, and platform-specific limitations.

`npm run tauri dev` is not configured. It becomes valid only if npm is selected and a matching script is defined; no frontend package-manager command is established here.

## Documentation changes now

Read [AGENTS.md](../../AGENTS.md) and the applicable directory guides. Review links and the complete change set, including untracked files. Use [testing guidance](testing.md) to report the checks performed without implying application validation.
