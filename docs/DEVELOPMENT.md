# Development baseline

The repository retains its existing MIT license. Implemented behavior and its
verification are recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md). Repository
working guidance lives in [AGENTS.md](../AGENTS.md). Private planning materials
stay under the ignored `docs/temp/` directory and are not required to build or
test. Extract applicable guidance into current documentation as functionality is
implemented; keep the remaining plans private.

## Toolchain and commands

Install Rust through rustup. The repository selects Rust 1.99.0, edition 2024,
with rustfmt and Clippy. The manifest's current minimum Rust version is 1.99;
older toolchains have not been qualified. No Access installation is needed for these checks.

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings
cargo test --workspace --no-default-features --locked
cargo clippy -p tablecraft-core --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-core --all-features --locked
cargo build --workspace --release --no-default-features --locked
cargo doc --workspace --all-features --no-deps --locked
cargo run --locked -p tablecraft-cli -- --version
```

Commit `Cargo.lock`. CI runs the Rust commands on Linux, macOS and Windows.
Configuring those jobs does not establish that they passed. Local execution evidence is recorded in IMPLEMENTATION.md.
CI has read-only repository permissions and does not retain checkout credentials.

## Package and feature policy

The workspace contains `apps/tablecraft-cli`, `crates/tablecraft-core` and
`crates/tablecraft-query`. The query crate uses sqlparser's lexer and expression
parser behind a bounded Access adapter; upstream AST/token types stay private.
Its owned SELECT syntax and tests are described in [SQL_PARSER.md](SQL_PARSER.md).
The core uses `thiserror` and offers an optional `serde` feature. Its generated
tests use `proptest`; `serde_json` is a development dependency for typed round-trip
tests. The implemented contracts and primary sources are in
[CORE_TYPES.md](CORE_TYPES.md). The CLI
does not depend on it until database commands need those types. Add libraries
under `crates/` when they own working functionality. Do not create empty packages
to match a plan.

The CLI uses clap's `Parser` derive and `Cli::try_parse()` for argument handling,
with default features disabled and `std`, `help`, `usage`, `error-context`,
`suggestions` and `derive` enabled. Terminal color detection and environment-variable
parsing are disabled. Keep its default
closure headless and free of GUI, network and Access-oracle runtimes. Introduce
optional subsystems with explicit features and test their minimal configurations
as they become real. Libraries must not depend on applications; compatibility
laboratory code must never enter production dependency closures. Use typed
library errors.

Follow [LIBRARY_POLICY.md](LIBRARY_POLICY.md) for the requested future library
choices. Exercise both the minimal and serde-enabled core configurations.
Core and query proptest suites use 256 cases per property and fixed seeds; retain their generated
regression files if a failure occurs. JSON tests deserialize directly into typed
values; production JSON output still needs an explicit format policy for its
consumers.

Review each new dependency's exact version, license, features, targets, MSRV,
native requirements and unsafe/FFI boundaries in DEPENDENCIES.md. Commit a tested
lockfile and keep experimental candidates separate from adopted dependencies.
Toolchain/dependency upgrades must update these records and rerun applicable CI.
Do not imply native or browser qualification from a successful headless build.

## CLI behavior

No arguments, `--help` and `-h` print help. `--version` and `-V` print the package
version. Unsupported commands, options and extra arguments print a diagnostic to
stderr and exit 2 without success output. Clap's built-in help and version flags
short-circuit parsing using its standard behavior. Output failures exit 1; a closed stdout
pipe exits successfully. No files are opened or modified. Machine-readable
database output is deferred to the inspection slice rather than inventing a
protocol during bootstrap.
