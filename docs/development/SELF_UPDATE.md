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

## Remaining release gates

- Provision a dedicated production public key and protected CI signing secrets.
- Implement a working shared updater with a fixed release repository/public key,
  strict newer-version/platform selection, bounded HTTPS downloads, checked
  redirects and cancellation. Reject tampered or mismatched signatures before
  archive processing. No production endpoint/key override through settings or
  environment variables.
- Stage verified archives privately on the installation filesystem, validate
  archive paths/types and product identity, recheck the payload before apply,
  implement safe platform replacement/relaunch and retain rollback on failures.
  Development and unsupported/package-manager installations need explicit manual
  update behavior.
- Connect desktop checking/progress/cancellation on a worker. Never apply an update
  while a native save/recovery worker or unsaved edits still owns the document;
  resolve those through normal application workflows first.
- Require real production signatures for release publication; dry-run signatures
  must not pass that gate. Integrate desktop assets and signature verification
  into the existing tag publication flow.
- Run the no-tag six-platform packaging workflow and retain its result. Verify
  update selection/download/tamper refusal and installation behavior separately;
  packaging smoke checks do not establish platform apply success.

No tag, push, workflow dispatch or release publication was performed by this
implementation. The in-app updater is not implemented yet.

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
