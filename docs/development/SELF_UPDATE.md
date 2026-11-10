# Signed desktop updates

## Release priority — 9 November 2026

The v0.1.0 release requires the agreed native CRUD work, signed desktop
self-updates and a successful packaging dry run without creating a tag.
Broader query/application parity is follow-up work, not a requirement to finish
all of Access before v0.1.0. Describe the actual supported operations and profiles
in the release notes. After this release preparation, the next frontend is
`tablecraft-web`; SQLite and native-project work remain paused.

## Relay reference and TableCraft boundary

The reference is [Relay's updater](https://github.com/PixelSoftwareStudios/Relay/blob/main/docs/updates.md)
and its [release workflow](https://github.com/PixelSoftwareStudios/Relay/blob/main/.github/workflows/release.yml),
inspected through GitHub on 9 November 2026. Relay separates a headless updater,
worker-thread desktop integration, artifact signing and platform-specific apply.
Its use of Minisign, version/filename-bound trusted comments and verification
before extraction is the design reference. Relay's code, fixtures and signing
key have not been copied. OS code signing/notarization is a separate concern.

TableCraft's signing contract is `tablecraft-update <version> <archive-name>`.
Use prehashed Ed25519 Minisign signatures (`ED`), not legacy direct signatures.
Every platform archive has a `.minisig` sidecar. Verify the cryptographic
signature and its exact trusted comment before extracting or executing anything.
SHA-256 sidecars remain useful integrity checks; they are not publisher trust.

## Implemented packaging preparation

The [desktop dry-run workflow](../../.github/workflows/desktop-packaging.yml)
can run manually without a tag, from packaging pull requests, or as a reusable
workflow. It builds six native desktop archives, generates locked dependency
notices and icons, verifies complete-set hashes, and executes only extracted
`--help` and `--version`. It signs the archives with an ephemeral test key and
independently verifies them. The uploaded artifact includes the test public key
and a conspicuous dry-run marker. It has no publication job, release write
permission or access to production signing secrets. Remote execution remains
unverified until a real workflow run is recorded.

[`update_signatures.py`](../../scripts/packaging/update_signatures.py) signs or
verifies the exact six CLI or desktop archive names. Production mode requires
`packaging/tablecraft-update-key.pub` and refuses a different supplied public key; the user is replacing the initial generated public key with their own. Dry-run mode accepts an explicit test
key. Signing uses `UPDATE_SIGNING_KEY` in the protected
`release-signing` environment. Encrypted keys use the separate
`UPDATE_SIGNING_PASSWORD` secret, supplied to Minisign through stdin.
Both secrets are removed from child environments; neither appears in arguments
or logs. Verification receives neither secret. The key is written only to a
private temporary file. Generate a passphrase-protected key with `minisign -G`
(omit `-W`), upload its public half to `packaging/tablecraft-update-key.pub`, and set both
CI secrets. An unencrypted key remains supported but is not required. Partial signature outputs are removed on
failure. Never commit a private signing key or promote a dry-run key to release
trust. Keep a recoverable offline backup of the real key.

## Implemented updater and remaining release gates

`tablecraft-update` owns fixed-origin release selection, the maintained public
key, bounded HTTPS/signature-first downloads, streaming authentication, checked
archive extraction and post-review source/staging revalidation. Native portable
replacement and macOS bundle replacement retain rollback backups; detached
helpers wait for normal host exit before relaunch. These are process-failure
rollback guarantees, not power-loss-atomic multi-file installation.

The desktop's About panel uses shared Fluent presentation state and an independent
host worker. The UI thread never performs networking, hashing, extraction or
replacement. Draft/native-worker gates are checked again at Install, interaction
is frozen during replacement, and late native outcomes retain their original
owners outside replaceable progress messages. Unexpected worker exits release
busy state and report an uncertain installation instead of freezing the UI.

The maintainer supplied the public key and configured the `release-signing`
environment. Production signing and independent verification now gate both
products. Manual `sign-packages=true` runs on main exercise that pipeline without
creating a tag or publishing. The original portable rehearsal built and checked
all twelve platform packages; its first attempt hit an artifact-service DNS
failure, and its retry reached assembly but exposed unavailable APT Minisign.
The workflow now provisions reviewed official Minisign 0.12 bytes with a fixed
SHA-256 before signing credentials are present. The expanded installer rehearsal
must verify this correction and the new native packages remotely.

Remaining gates before claiming the updater/release complete:

- Complete signed MSI and original-AppImage update routes with actual local
  installation authority. Program Files and AppImage runtime directories are
  currently manual; they must not be fed to the portable writer. DEB/RPM remain
  deliberately owned by their package manager.
- Obtain passing native CI evidence for all requested installers, runtime
  source/relink checks, complete assembly and real production-key signatures.
- Qualify helper execution and update/relaunch behavior on the supported native
  platforms. Headless argument/filesystem tests are distinct from native relaunch.
- Complete the agreed CRUD scope and ensure release notes describe the actual
  operation/profile limits. Packaging does not establish database compatibility.

## Connected verification — 9 November 2026

The full updater service batch passes 58 tests. The complete desktop batch passes
235 tests with one existing hash-pinned Jet 4 fixture ignore. Strict all-target,
all-feature Clippy passes for both. Focused UI checks and three reviewed source-free
About frames cover translated update controls, late-input identity and draft-safe
restart gates. No native application window or relaunch helper was executed.
The generated desktop notices include the updater's actual dependency closure;
expanded bundles and inventories are not tracked.

## Local verification — 9 November 2026

Five tests execute the real Minisign tool: complete-set verification, tampered
payload refusal, valid signatures carrying the wrong version/filename, wrong key,
missing sidecars, failed-sign cleanup and missing production trust/symlink refusal.
All pass. The 33 existing CLI/desktop archive tests, workflow actionlint and local
documentation-link check pass as well.

A freshly generated notice bundle and retained generated ICNS were packaged with
the existing macOS ARM64 **debug** desktop executable. Extraction, architecture,
manifest/notice/icon checks and extracted `--help`/`--version` pass. That actual
archive was then signed with a fresh disposable key; cryptographic verification
and exact trusted-comment matching pass. Artifacts and the test public key remain
under ignored `.cache/desktop-updater-packaging-dry-run-20261109/`; its private key
was deleted with its temporary directory. This is one local platform's packaging
and signature evidence, not a release build, production-key signature,
six-platform CI run or updater installation test. No native window was launched.
