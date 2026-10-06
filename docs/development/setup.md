# Development setup

## What works today

The repository can be read and edited as Markdown with Git and a text editor. No Rust/Tauri application, frontend package, dependency lockfile, development container, executable test suite, or CI workflow exists.

Do not run a project generator, install dependencies, or infer build commands solely from the directory scaffold. There is no launch command to troubleshoot yet.

## Future bootstrap prerequisites

When implementation is authorized, select and record compatible Rust, Tauri, frontend tooling, and package-manager versions. Establish manifests and lockfiles before documenting installation commands.

Tauri development needs Rust and platform-specific native dependencies. A JavaScript frontend toolchain may additionally need Node.js and its chosen package manager. Check the [official prerequisites](https://v2.tauri.app/start/prerequisites/) for the selected Tauri release and host platform. Container-based work will not replace native verification on all target systems.

The eventual setup guide must describe:

- Supported development hosts, required native dependencies, and pinned project tool versions.
- Exact install, development, test, lint, formatting, and build commands with working directories.
- Environment variables by purpose, without secrets or private endpoints.
- Local test-server and disposable-profile setup.
- Common failures, cleanup, and platform-specific limitations.

## Conditional command guidance

The existing design proposes Cargo formatting, linting, testing, and documentation checks. Commands such as `cargo fmt --check`, `cargo clippy`, `cargo test`, and `cargo doc` become relevant only after a valid manifest exists and their scope/features are documented.

Likewise, `npm run tauri dev` is valid only if npm is selected and a matching script is actually defined. This scaffold does not establish that script or any alternative package-manager command.

## Documentation changes now

Read [AGENTS.md](../../AGENTS.md) and the applicable directory guides. Review links and the complete change set, including untracked files. Use [testing guidance](testing.md) to report the checks performed without implying application validation.
