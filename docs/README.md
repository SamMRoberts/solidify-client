# Documentation index

## Current status

solidify currently provides a Markdown-only scaffold for a future Rust/Tauri MUD client. There is no runnable application, protocol implementation, plugin API, dependency manifest, CI workflow, or published documentation site. Directory guides describe responsibilities for later work.

## Architecture and extensions

- [Architecture overview](architecture/overview.md): boundaries, ownership, and intended data flow.
- [Protocol requirements](architecture/protocols.md): planned coverage and parser acceptance criteria.
- [Plugin design](plugins/design.md): extension points, runtime alternatives, isolation, and lifecycle.
- [Plugin example guide](../examples/plugins/README.md): proposed runtime directories and future validation.

## Development

- [Setup](development/setup.md): what can be done now and prerequisites for later implementation.
- [Testing strategy](development/testing.md): behavioral tests, fuzzing, and native acceptance.
- [Agent instructions](development/agent-instructions.md): discovery, scope, and maintenance.
- [Contributing](../CONTRIBUTING.md): focused changes and verification reports.

## Operations

- [Automation roles](operations/automation.md): the nine proposed development and operations roles.
- [Release process](operations/releases.md): versioning, packaging, signing, and release gates.
- [Security guidance](operations/security.md): boundaries, dependency review, and sensitive evidence.

## Resources

The [resource index](../resources/README.md) links to primary references, fixture conventions, and asset provenance. Keep externally sourced evidence there and maintained project guidance here. All capability claims should be updated alongside implementation and its validation.
