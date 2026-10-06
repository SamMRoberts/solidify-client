# Primary references

These links were opened and checked for title and relevance on 2026-10-06, including the RFC 1143 and Tokio additions. They support protocol implementation and future design work; the [protocol contracts](../../docs/architecture/protocols.md) and [session contracts](../../docs/architecture/sessions.md) identify actual support. Tokio 1.53.2 is the selected session dependency; other source versions are references unless explicitly selected.

## Agent instructions and Tauri

| Source | Relevance |
|---|---|
| [OpenAI AGENTS.md guidance](https://learn.chatgpt.com/docs/agent-configuration/agents-md) | Instruction discovery, layering, and size limits |
| [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) | Rust and platform development prerequisites |
| [Tauri security](https://v2.tauri.app/security/) | Native/frontend trust boundaries and permissions |
| [Tauri distribution](https://v2.tauri.app/distribute/) | Platform packaging and distribution |
| [Tauri updater](https://v2.tauri.app/plugin/updater/) | Updater integration and artifact signatures |

## TCP session runtime

Checked on 2026-10-06. Tokio is distributed under the MIT license; Cargo fetches the dependency source, and no upstream manual is vendored here. This does not establish a project license.

| Source | Relevance and limits |
|---|---|
| [Tokio 1.53.2](https://docs.rs/tokio/1.53.2/tokio/) | Selected runtime library and feature flags; the caller owns the runtime |
| [Tokio select cancellation safety](https://docs.rs/tokio/1.53.2/tokio/macro.select.html#cancellation-safety) | Cancel-safe reads/receives and cancellation hazards for compound writes; session partial writes terminate rather than restart |

## Protocols

| Source | Relevance and limits |
|---|---|
| [RFC 854](https://www.rfc-editor.org/rfc/rfc854) | Telnet transport semantics and negotiation |
| [RFC 855](https://www.rfc-editor.org/rfc/rfc855) | Telnet option specifications |
| [RFC 1143: The Q Method of Implementing TELNET Option Negotiation](https://www.rfc-editor.org/rfc/rfc1143) | Section 7 state transitions used by the generic negotiator; no option-specific semantics or transport coverage |
| [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/) | Control functions underlying terminal behavior; the supported subset must be documented |
| [Zugg Software MXP specification](https://www.zuggsoft.com/zmud/mxp.htm) | Protocol-author reference for MUD markup |
| [Zugg Software MSP specification](https://www.zuggsoft.com/zmud/msp.htm) | Protocol-author reference for sound directives |
| [Aardwolf GMCP documentation](https://www.aardwolf.com/wiki/index.php/Clients/GMCP) | Server-owned reference; Aardwolf packages are not universal GMCP requirements |
| [Iron Realms ATCP reference](https://www.ironrealms.com/rapture/manual/files/FeatATCP-txt.html) | Original server ecosystem's ATCP behavior |

## Plugin and verification tooling

| Source | Relevance |
|---|---|
| [Rust type layout](https://doc.rust-lang.org/reference/type-layout.html) | Limits of representation guarantees across native boundaries |
| [Rust external blocks and ABIs](https://doc.rust-lang.org/reference/items/external-blocks.html) | Native calling conventions and ABI considerations |
| [Wasmtime security](https://docs.wasmtime.dev/security.html) | Sandbox model and host responsibilities |
| [Lua 5.4 manual](https://www.lua.org/manual/5.4/) | Example reference for embedding and standard libraries; Lua version is not selected |
| [Rust Fuzz Book: cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) | Candidate parser fuzzing tooling |
| [RustSec](https://rustsec.org/) | Rust advisory data and audit tooling |
| [cargo-deny](https://embarkstudios.github.io/cargo-deny/) | Candidate license, advisory, and dependency policy checks |
| [GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use) | Workflow permissions, untrusted inputs, and supply-chain controls |

No JavaScript engine, release helper, documentation generator, or native ABI library has been selected. Add a verified upstream reference when making each choice; do not retain unresolved citations or imply that a named tool is installed.
