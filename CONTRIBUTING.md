# Contributing to solidify

## Before making a change

Read the [repository instructions](AGENTS.md), the nested instructions for the affected directories, and the relevant [design documentation](docs/README.md). Inspect Git status and preserve existing work.

The project is currently a Markdown-only scaffold. Documentation tasks should not initialize an application or add dependencies. Future implementation changes should establish actual manifests, tooling, and documented commands within their approved scope.

## Contributions

Describe the problem, intended behavior, affected subsystems, and compatibility impact. Keep changes focused; explain material assumptions and avoid unrelated formatting or refactors. Update design and user-facing documentation when behavior changes.

For bugs, include minimal reproduction steps, expected and actual behavior, operating system, and relevant tool or application versions. Remove credentials, private MUD transcripts, and personal paths from evidence. Distinguish a proposed capability from an implemented feature.

No project license or contributor agreement is established by this scaffold. Record third-party provenance and licensing before adding external material; do not invent a project license.

## Validation

For Markdown, check relative links, heading structure, instruction consistency, whitespace, and new files as well as tracked diffs. For later implementation, follow the [testing strategy](docs/development/testing.md) and commands defined by the actual manifests.

A change report should name the checks performed, their results, and any checks not run with reasons. Separate automated results from native application and live-server acceptance. Use local emulation and disposable data by default.

Do not weaken tests to obtain a passing result. Do not claim that a screenshot, build, or documentation review proves transport, persistence, or plugin isolation.

## Review and publishing

Use the [pull request template](.github/PULL_REQUEST_TEMPLATE.md) to explain observable behavior and relevant verification. Reference related issues only when they exist. Commits, pushes, releases, and external messages require task authorization; this guide does not trigger them.

Potential security reports should omit exploit-sensitive or private data from public issues. No private reporting channel is configured here; see the [security guidance](docs/operations/security.md) before sharing details.
