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
`packaging/update-key.pub` and refuses a different supplied public key; that
release key has not been provisioned yet. Dry-run mode accepts an explicit test
key. Signing uses `TABLECRAFT_UPDATE_SIGNING_KEY`, an unencrypted Minisign secret
in a protected signing environment; it is written to a private temporary file
and never placed in arguments or logs. Partial signature outputs are removed on
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
