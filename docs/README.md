# Documentation index

## Current status

solidify currently provides a dependency-free Rust Telnet framing library with a Cargo manifest, lockfile, and unit tests. The desktop client remains planned: no runnable application, transport, plugin API, CI workflow, or published documentation site exists. Directory guides distinguish the implemented parser from reserved subsystems.

## Architecture and extensions

- [Architecture overview](architecture/overview.md): boundaries, ownership, and intended data flow.
- [Protocol requirements](architecture/protocols.md): implemented framing contract, planned coverage, and parser acceptance criteria.
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
