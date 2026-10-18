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

## Shared core types

`crates/tablecraft-core` implements distinct object/table/operation/cursor IDs,
table- and generation-scoped record locators, cursor bookmarks, scalar values,
typed validation failures and checked resource budgets. It has no dependencies
or optional features and does not yet participate in CLI database commands.

Exact Currency, 28-digit field Decimal, width-preserving integers, bit-preserving
floats, GUIDs and distinct legacy/extended timezone-free dates are implemented.
NULL stays separate from empty text, and conversion rejects expression-only
Empty/Missing/Error states. Value and locator diagnostic output redacts payloads.
[CORE_TYPES.md](CORE_TYPES.md) maps the implemented contracts to Microsoft primary
references and records source inconsistencies and remaining boundaries.

Local core verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 passed 15 contract tests and four compile-fail doctests, alongside the three
existing CLI tests. Core coverage is organized as 12 feature-local unit tests
and three integration tests spanning feature boundaries. The decimal rescaling
test exhaustively checks coefficients
-100..=100 across all pairs of supported scales. Formatting, Clippy with warnings
denied, standalone core checking, release builds and rustdoc with warnings denied
also passed. Native codecs, serialization protocols, Access
oracle comparison and SQL evaluation remain unimplemented.

## Access example

The supplied [Access example](examples/Access_Example_VBA.accdb) is registered in
[FIXTURES.md](FIXTURES.md) with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
