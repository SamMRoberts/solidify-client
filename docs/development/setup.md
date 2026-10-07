# Development setup

## What works today

solidify has a single-connection desktop client and a separately usable Rust
library. Tauri 2.12.1, React 19.3.0, TypeScript 7.0.2, Vite 8.3.3 and npm are
selected; resolved dependencies are locked in Cargo.lock and package-lock.json.
The library baseline is Rust/Cargo 1.99.0 (edition 2024), with Tokio pinned to
1.53.2. Protocol modules do not depend on Tokio. The `desktop` feature gates all
Tauri and serialization dependencies; library-only work needs neither Node nor
native webview development dependencies.

Desktop checks were developed on macOS using Rust/Cargo 1.99.0 and Node 26.10.0 /
npm 11.19.0. Use those versions for the documented baseline. Native builds also
need the platform dependencies in the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
On macOS this includes Xcode Command Line Tools. Windows/Linux runtime acceptance,
installers, signing, persistence and CI remain deferred. No credentials, profiles,
or environment secrets are needed for local verification.

## Launch the desktop and demo

Run from the repository root:

```sh
npm ci
cargo fetch --manifest-path src-tauri/Cargo.toml --locked

# Terminal 1: one-client loopback demo, not a public MUD.
npm run demo

# Terminal 2: Vite and the native Tauri window.
npm run tauri:dev
```

Click **Connect** with `localhost` and port `4000`. Nothing connects automatically.
The demo prints its listening address. An alternate port is available with
`npm run demo -- --port 4001`; enter the same port in the toolbar. The demo binds
IPv4 loopback; hostname connection exercises resolver fallback where applicable.
It handles one client at a time, caps command input at 16 KiB, and applies a
five-second write deadline and five-minute input idle deadline. Stop it with
Ctrl+C after closing the client. It prints no received commands or transcripts.

`npm run dev` alone serves the real frontend at `http://localhost:1420` for layout
inspection. A browser has no native connection bridge; use `tauri:dev` for TCP.
Mocked bridges exist only in tests. Port 1420 must be free; close an independently
started Vite instance before running `tauri:dev`.

## Desktop and frontend commands

```sh
npm run typecheck
npm test
npm run format:check
npm run build

# Native feature validation, using cached locked dependencies.
cargo check --manifest-path src-tauri/Cargo.toml --locked --offline --features desktop --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --features desktop
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --features desktop --all-targets -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked --offline --features desktop
cargo doc --manifest-path src-tauri/Cargo.toml --locked --offline --features desktop --no-deps

# Production frontend plus native binary; no installer or signing workflow.
npm run tauri:build
```

The production binary is `src-tauri/target/release/solidify` (`solidify.exe` on
Windows). Unlike a direct Cargo development build, the Tauri build command enables
its custom protocol and embeds `dist/`. It can run without Vite. On macOS, an
optional local application bundle can be produced after the build:

```sh
npm exec tauri -- bundle --bundles app
open src-tauri/target/release/bundle/macos/solidify.app
```

This creates a local `.app`, not a signed/notarized release or an installer.
Do not run the bundle command on Windows/Linux. Generated assets, native build
output, `dist/`, and `node_modules/` are ignored. Use Prettier on changed frontend
files and `cargo fmt` on changed Rust files before checking formatting.

## Manual desktop checklist

1. Launch the demo and native client. Verify no automatic connection; Connect to
   `localhost:4000`. Read “Streaming Unicode: 🌍 café” and `demo> ` before newline.
2. Send `styles`, `unicode`, `controls`, and `markup`. Check basic colors/style
   flags, whole grapheme deletion, CR replacement, eight-column tabs, and literal
   angle-bracket markup. Try Enter and Send; verify the accepted draft clears.
3. Toggle **Mask input**, send a harmless test command, and toggle it off. A nonempty
   masked draft stays masked until sent or cleared. No local
   command echo or history should appear. Send an empty line; preserve ordinary
   spaces in a normal command. Do not use real credentials for this demo.
4. Send `burst`, scroll upward, then send `help`. The viewport should stay in
   place and show **Latest output**; click it to follow again. Clear output and
   confirm subsequent output still uses the active style.
5. Send `malformed`. Preceding text remains, a sanitized presentation error appears,
   and the connection closes. Reconnect and verify the previous transcript is gone.
   Send `quit` for clean EOF. Try a closed local port for a connection error.
6. Reconnect, disconnect using the toolbar, reconnect once more, and close the
   window while connected. The application must exit and the demo must accept a
   fresh client. Check keyboard focus and resize to the 720×480 minimum.

The client uses plain TCP. Public MUD connections are a separate, user-initiated
manual activity and are not development acceptance. See the [desktop contracts](../architecture/desktop.md)
for exact bounds, DNS cancellation, input, and transcript semantics.

## Library commands

Run these commands from the repository root. Fetch the pinned dependencies first (network access and a writable Cargo cache are required). Subsequent checks use locked, offline resolution; an empty cache cannot run them until the fetch succeeds. Loopback tests need permission to bind local sockets, but no external server or configuration.

```sh
# Fetch locked dependencies before offline checks.
cargo fetch --manifest-path src-tauri/Cargo.toml --locked

# Protocol unit tests, including the existing decoder tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet

# Focused outbound encoding and negotiation checks.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet::encoder
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::telnet::negotiation

# Bounded UTF-8/basic ANSI presentation and session-to-presentation checks.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::presentation
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --test tcp_sessions presentation

# Public-API in-memory peer integration tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --test telnet_wire

# Session unit tests and public-API TCP loopback tests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline sessions::tests
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --test tcp_sessions

# Application ownership, DNS, polling and command-boundary checks.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline application

# Full unit/integration suite and doctests.
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline

# Read-only formatting check and lint check.
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings

# Compile the library and generate its API documentation.
cargo build --manifest-path src-tauri/Cargo.toml --locked --offline
cargo doc --manifest-path src-tauri/Cargo.toml --locked --offline --no-deps
```

`cargo fmt --manifest-path src-tauri/Cargo.toml` applies Rust formatting when needed. Build output and generated documentation are under the ignored `src-tauri/target/` directory; the documentation entrypoint is `src-tauri/target/doc/solidify_client/index.html`. These commands leave the desktop feature disabled; no native binary or installer is built.

If Cargo reports an unsupported Rust version, check `rustc --version` and `cargo --version` against the baseline. If formatting or lint tools are missing, that check cannot be reported as passing. Neither issue requires initializing Tauri or a frontend. Tests create and clean up their own local listeners; no manual server or disposable profile setup is needed.

## Documentation checks

Read [AGENTS.md](../../AGENTS.md) and applicable directory guides. Review relative
links, headings, instruction consistency, whitespace, and the complete diff,
including new files. Follow [testing guidance](testing.md) when reporting separate
automated, browser, native, and live-server evidence.

## Telnet compatibility checklist

Run `npm run demo` and `npm run tauri:dev` in separate terminals as above.
If port 4000 is occupied, use `npm run demo -- --port 4001` and enter port 4001.
Use synthetic input only; the demo never requires credentials.

1. Connect to `localhost` and send `protocol`: expect `Terminal: SOLIDIFY` and
   nonzero columns/rows. Resize the native window, wait briefly, and send
   `protocol` again; dimensions should follow the transcript's usable area.
2. Send `mask`: expect **Server-requested masking** and an obscured command field.
   Enter a harmless synthetic response and send it. Expect a fixed discarded-input
   acknowledgment, no echo of your response, and normal input after the server
   disables ECHO. Manual **Mask input** can keep masking enabled.
3. Send `options-off`, then `protocol`: identity and viewport should be unavailable.
   Resize while disabled, send `options-on`, then `protocol`; expect the identity
   and latest dimensions to return. Ordinary commands remain line-based throughout.
4. Disconnect/reconnect: expect fresh output and normal input. Check `malformed`
   for a sanitized decoder error and reconnect again. Send `burst` and close the
   window during output; the process should exit and the demo accept a fresh client.

Protocol/state regression tests additionally verify a server disabling masking
with a nonempty draft, failed-send retention, transient ECHO cycles between polls,
queue pressure, stale IDs, and cleanup. These automated checks are distinct from
manual browser/native acceptance.

Focused option checks:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline protocols::options
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline option
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline negotiated_state
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --example demo_server
npm test -- src/bridge/viewport.test.ts src/components/components.test.tsx
```
