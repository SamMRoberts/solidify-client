# Security and dependency guidance

## Status and reporting

Rust and frontend dependencies are locked; no security scanning workflow, supported-version policy, or private reporting channel is configured. This document records engineering requirements for the planned client, not a completed security assessment.

Do not put credentials, private transcripts, or sensitive vulnerability details in public issues. If a private reporting mechanism is later configured, document and verify it before directing reporters there. Do not invent an email address or claim that private advisories are enabled.

## Trust boundaries

- **Network and rendering:** Treat MUD bytes, ANSI controls, MXP markup, sound directives, URLs, and structured messages as untrusted. Bound parsing and rendering work; keep executable content and privileged actions behind validated policies.
- **Frontend/native IPC:** Validate commands and session identity in the backend. Expose narrowly scoped operations; a compromised view must not gain arbitrary shell or filesystem access. Review [Tauri security guidance](https://v2.tauri.app/security/).
- **Plugins:** Native libraries have host-process privileges. WASM and scripts need safe imports, restricted capabilities, budgets, and reliable cleanup; see [plugin design](../plugins/design.md).
- **Storage:** Validate paths and imported content; protect user data during writes and migrations. Keep secrets separate from ordinary settings and redact diagnostics.
- **Automation:** Treat fork code and issue/PR metadata as untrusted. Keep privileged signing, publication, and deployment separate from untrusted checks.

## Dependencies and supply chain

Review both Rust and frontend dependencies for provenance, maintenance, advisories, and license compatibility. [RustSec](https://rustsec.org/) and [cargo-deny](https://embarkstudios.github.io/cargo-deny/) are candidate tools, not configured checks.

Review transitive dependencies and justify exceptions with scope, rationale, owner, and expiration or review conditions. A duplicate version is a review signal, not automatically a vulnerability. Passing a dependency scan does not prove application security.

Pin third-party CI actions to reviewed immutable revisions and grant only required permissions. Follow [GitHub's secure-use guidance](https://docs.github.com/en/actions/reference/security/secure-use) when workflows are introduced.

## Evidence handling

Use synthetic data or authorized, redacted captures in tests. Never store live user profiles, tokens, signing material, or account passwords in repository resources. Keep crash reports and fuzz reproducers minimal, reproducible, and safe to share.

When fixing a security-relevant behavior, test the boundary that failed and document remaining limits. Do not infer isolation, cleanup, or authorization correctness from compilation alone.
