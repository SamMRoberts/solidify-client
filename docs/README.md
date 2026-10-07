# Documentation index

## Current status

solidify currently provides Telnet decoding/encoding, configurable Q-method negotiation, bounded Tokio TCP sessions, and independent UTF-8/basic ANSI presentation decoding, with unit, in-memory peer, and loopback integration tests. A Tauri 2 + React/TypeScript desktop client adds one connection, hostname resolution, bounded styled transcripts, command entry, and a local demo. TTYPE/NAWS, remote ECHO masking, and SGA are implemented through an opt-in session profile used by the desktop. Other option behavior, plugins, persistence, installers, CI and a published documentation site remain deferred. Directory guides distinguish the implemented core from reserved subsystems.

## Architecture and extensions

- [Architecture overview](architecture/overview.md): boundaries, ownership, and intended data flow.
- [Protocol requirements](architecture/protocols.md): implemented Telnet and presentation decoding, encoding, and negotiation contracts; planned coverage and acceptance criteria.
- [Desktop contracts](architecture/desktop.md): application ownership, IPC, DNS, bounded delivery, transcript semantics, and deferred work.
- [TCP sessions](architecture/sessions.md): public API, ownership, queue bounds, cancellation, and shutdown contracts.
- [Plugin design](plugins/design.md): extension points, runtime alternatives, isolation, and lifecycle.
- [Plugin example guide](../examples/plugins/README.md): proposed runtime directories and future validation.

## Development

- [Setup](development/setup.md): exact install/run/check commands and a manual desktop checklist.
- [Testing strategy](development/testing.md): behavioral tests, fuzzing, and native acceptance.
- [Agent instructions](development/agent-instructions.md): discovery, scope, and maintenance.
- [Contributing](../CONTRIBUTING.md): focused changes and verification reports.

## Operations

- [Automation roles](operations/automation.md): the nine proposed development and operations roles.
- [Release process](operations/releases.md): versioning, packaging, signing, and release gates.
- [Security guidance](operations/security.md): boundaries, dependency review, and sensitive evidence.

## Resources

The [resource index](../resources/README.md) links to primary references, fixture conventions, and asset provenance. Keep externally sourced evidence there and maintained project guidance here. All capability claims should be updated alongside implementation and its validation.
