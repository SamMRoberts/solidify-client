# Proposed release process

## Status

There is no application version, release history, changelog, signing identity, updater feed, packaging configuration, or release workflow yet. This is a readiness checklist for later implementation.

## Versioning and preparation

Use semantic versioning once compatibility contracts are defined. Keep application, package, and release metadata consistent; document plugin API compatibility separately when it exists. Generate release notes from verified changes, including breaking changes and migration guidance. Do not invent historical entries.

Before a release, select the commit, supported platforms/architectures, package formats, and explicit publication destination. Rebuild reviewed source with recorded toolchains and dependencies. Preserve artifact provenance through signing and upload.

## Packaging direction

Candidate outputs are macOS application bundles and DMG images, Windows installer executables and MSI packages, and Linux Debian/RPM packages and AppImages. These formats are targets to evaluate, not a promise that every OS/architecture combination is supported.

Follow [Tauri distribution guidance](https://v2.tauri.app/distribute/) for the chosen release and formats. Verify installation, launch, upgrade, and removal on each supported platform.

Signing and notarization depend on platform and distribution policy. Keep signing keys in scoped secret storage and verify the signed artifacts. Updater signatures are a separate concern from OS code signing; consult the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/) before implementing a feed.

## Release gates

- Confirm version metadata, commit identity, notes, compatibility, and migration requirements.
- Pass relevant tests, dependency/license review, platform builds, and native acceptance.
- Check expected artifacts, checksums, signing results, and installation behavior.
- Test updater behavior, invalid signatures, and failed/interrupted updates if updating is supported.
- Publish only through an authorized trusted workflow with the necessary credentials.
- Read back the published release and asset list; distinguish upload success from install/upgrade acceptance.

## Failure and recovery

Stop publication on missing or inconsistent artifacts, failed signatures, or unresolved compatibility issues. Preserve useful logs without secrets. A partially published release needs an explicit recovery action; do not silently replace immutable tags or shipped assets.

Document recovery instructions for failed upgrades and user-data migrations before shipping those features. Native packaging success on one platform does not verify the others.
