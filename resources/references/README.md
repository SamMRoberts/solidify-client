# Primary references

These links were opened and checked for title and relevance on 2026-10-06. They support future design work, not a declaration of implemented support. Source versions are references rather than selected project dependencies. Recheck version-specific guidance when implementation begins.

## Agent instructions and Tauri

| Source | Relevance |
|---|---|
| [OpenAI AGENTS.md guidance](https://learn.chatgpt.com/docs/agent-configuration/agents-md) | Instruction discovery, layering, and size limits |
| [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) | Rust and platform development prerequisites |
| [Tauri security](https://v2.tauri.app/security/) | Native/frontend trust boundaries and permissions |
| [Tauri distribution](https://v2.tauri.app/distribute/) | Platform packaging and distribution |
| [Tauri updater](https://v2.tauri.app/plugin/updater/) | Updater integration and artifact signatures |

## Protocols

| Source | Relevance and limits |
|---|---|
| [RFC 854](https://www.rfc-editor.org/rfc/rfc854) | Telnet transport semantics and negotiation |
| [RFC 855](https://www.rfc-editor.org/rfc/rfc855) | Telnet option specifications |
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
