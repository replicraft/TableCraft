# Implementation status

## Headless build foundation

Implemented the minimal Cargo workspace, pinned Rust toolchain, clap-based
headless CLI, build/feature/licensing policy and three-platform CI configuration.
The MIT license is preserved.

The CLI only implements help and version output. Integration tests launch it
without display environment variables and check output streams, version identity
and rejection of unsupported commands/extra arguments.

Local verification passed on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026:

- rustfmt check and Clippy with warnings denied.
- All three CLI integration tests (help, version and invalid arguments).
- Locked, offline release build and executable version smoke check.
- Dependency tree contains clap and its derive dependencies; the CLI remains
  free of graphics, network and database runtimes. Versions, licenses, features
  and minimum toolchains are recorded in DEPENDENCIES.md.
- Git whitespace check.

Remote CI and native Windows/Linux execution have not yet been observed.
Database commands and a desktop host do not exist yet.

## Access example

The supplied docs/examples/Access_Example_VBA.accdb is registered in FIXTURES.md with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
