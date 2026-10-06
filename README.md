# solidify-client

solidify is a planned cross-platform MUD client built around Rust and Tauri for Windows, Linux, and macOS.

## Project status

This repository is a Markdown-only architecture and contributor scaffold. It does not yet contain application code, dependency manifests, runnable tests, installers, or automated workflows. There is no build or launch command at this stage.

## Planned capabilities

- Telnet transport and option negotiation, ANSI color handling, and MXP markup.
- GMCP, ATCP, and MSP extensions with session-scoped state and safe rendering.
- An accessible terminal interface and persistent user settings.
- A plugin system with proposed Rust dynamic-library, WebAssembly, Lua, and JavaScript options.
- Cross-platform verification, packaging, release management, and documentation automation.

These are design intentions, not a claim of protocol compliance or available plugin support. Frameworks, engines, versions, and public interfaces will be chosen during implementation.

## Repository layout

Frontend responsibilities live under `src/`; the Rust/Tauri backend and its integration tests under `src-tauri/`. Application acceptance tests belong in `tests/`, parser fuzzing in `fuzz/`, and plugin examples in `examples/plugins/`. Each scaffolded directory contains scoped agent guidance.

The remaining directories cover tooling (`scripts/`), GitHub collaboration (`.github/`), development environments (`.devcontainer/`), documentation, and reference resources. Their contents are Markdown only.

## Start here

- [Documentation index](docs/README.md)
- [Architecture and data flow](docs/architecture/overview.md)
- [Development setup and current limitations](docs/development/setup.md)
- [Contribution guide](CONTRIBUTING.md)
- [Repository agent instructions](AGENTS.md)
- [Resources and primary references](resources/README.md)
