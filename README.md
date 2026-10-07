# solidify-client

solidify is a Rust/Tauri MUD client targeting Windows, Linux, and macOS.

## Project status

The first desktop client uses **Tauri 2 + React + TypeScript**. It supports one
plain TCP connection, IPv4/IPv6 and ASCII hostnames, streaming Unicode, ANSI styles
with bright/indexed/RGB colors, bounded scrollback, command entry,
automatic/manual input masking, and clean cancellation/disconnect. Named saved connections restore the last selection without
automatically connecting; each profile has live-preview transcript appearance.
A local demo exercises the real TCP/Telnet/presentation path without credentials
or a public server.

The independent Rust library retains bounded Telnet decoding/encoding, RFC 1143
Q-method negotiation, numeric-address sessions, and UTF-8/ANSI presentation
decoding. A stateless GMCP envelope codec adds bounded JSON parsing/encoding;
an explicit session profile adds connected GMCP with ordered interpretation and
bounded sends. Desktop GMCP and package behavior remain deferred. Default sessions deny all
options; desktop sessions opt into TTYPE,
NAWS, remote ECHO, and SGA. Protocol modules remain independent
of Tokio; the crate pins Tokio 1.53.2. Tauri is gated behind the `desktop` feature.
See [protocols](docs/architecture/protocols.md), [sessions](docs/architecture/sessions.md),
[desktop contracts](docs/architecture/desktop.md), and
[saved connection storage](docs/architecture/storage.md).

## Run locally

Use Rust/Cargo 1.99.0, Node 26.10.0 / npm 11.19.0, and the native prerequisites
listed in [setup](docs/development/setup.md). From the repository root:

```sh
npm ci
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
npm run demo
```

In a second terminal:

```sh
npm run tauri:dev
```

Click **Connect** at `localhost:4000`. Try `help`, `styles`, `unicode`, `controls`,
`markup`, `burst`, `malformed`, and `quit`. See the [manual checklist](docs/development/setup.md#manual-desktop-checklist)
for expected behavior and the [verification strategy](docs/development/testing.md)
for evidence boundaries. `npm run tauri:build` builds a production binary without
an installer. Nothing connects automatically.

## Library-only checks

No frontend or native application dependencies are needed to compile the library:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
```

Fetch locked Cargo dependencies first if the cache is empty. Full formatting,
Clippy, build, documentation, desktop-feature and frontend commands are in
[development setup](docs/development/setup.md).

## Deferred capabilities

Multiple sessions, credentials, TLS, other Telnet option handlers, other persistence,
plugins, desktop GMCP and package handling, MXP/ATCP/MSP, cursor addressing,
full terminal emulation, installers/signing and CI remain deferred. Windows/Linux
runtime and public-MUD acceptance remain unverified. Rust and IPC interfaces are
internal project contracts, not stable plugin APIs.

## Repository layout

`src-tauri/` contains the library, optional desktop binary, coordinator tests,
public-API integration tests, and demo example. `src/` contains the frontend and
colocated Vitest tests. `tests/` holds acceptance guidance; fuzzing and plugin
examples remain reserved. Other directories cover tooling, contribution templates,
documentation, and reference resources.

- [Documentation index](docs/README.md)
- [Architecture and data flow](docs/architecture/overview.md)
- [Development setup](docs/development/setup.md)
- [Contribution guide](CONTRIBUTING.md)
- [Repository agent instructions](AGENTS.md)
- [Resources and primary references](resources/README.md)
