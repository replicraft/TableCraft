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
cargo check -p tablecraft-schema --no-default-features --locked
cargo check -p tablecraft-query --no-default-features --locked
cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings
cargo test --workspace --no-default-features --locked
cargo clippy -p tablecraft-expressions --all-targets --no-default-features --locked -- -D warnings
cargo test -p tablecraft-expressions --no-default-features --locked
cargo clippy -p tablecraft-expressions --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-expressions --all-features --locked
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

The workspace additionally contains `crates/foundation/tablecraft-schema` and
`crates/query/tablecraft-query`, the bounded read-only retrieval layer in
[QUERY_EXECUTION.md](QUERY_EXECUTION.md). Schemas validate source carriers and
nullability using [shared identities/type descriptors](SCHEMA.md). Apply explicit
SchemaPolicy limits/comparison at each consumer boundary; SQL spelling conversion
stays in query binding. Query sources remain explicit host interfaces.
The workspace contains `apps/tablecraft-cli`, `crates/foundation/tablecraft-core` and
`crates/query/tablecraft-sql`, plus `crates/query/tablecraft-expressions` for
[scalar evaluation](EXPRESSIONS.md). The SQL crate uses sqlparser's lexer and parser primitives behind an explicit
bounded Access grammar; upstream token/parser types stay private.
Its owned query syntax and tests are described in [SQL_PARSER.md](SQL_PARSER.md).
The core uses `thiserror` and offers an optional `serde` feature. Its generated
tests use `proptest`; `serde_json` is a development dependency for typed round-trip
tests. The implemented contracts and primary sources are in
[CORE_TYPES.md](CORE_TYPES.md). The CLI uses the SQL crate for parsing, the
expression crate with `sql` for scalar evaluation and the query crate for
read-only inline retrieval, consuming core values/budgets. Add libraries
under `crates/` when they own working functionality. Do not create empty packages
to match a plan.

Follow [ARCHITECTURE.md](ARCHITECTURE.md) for domain grouping and package
ownership. Keep explicit workspace members so adding a directory does not
silently add a package. The syntax package is `tablecraft-sql` (`tablecraft_sql`
in Rust); `tablecraft-query` now owns working binding/planning/execution over explicit
sources and consumes the existing scalar semantics. Current syntax
types stay in the SQL package. Keep internal feature modules until independent
consumers, dependencies or testing/platform needs justify extraction.

The future native engine must satisfy [NATIVE_ENGINE.md](NATIVE_ENGINE.md): keep
`tablecraft-ace` independently usable and keep `tablecraft-access` as an adapter.
Backend-neutral/query contracts cannot import concrete adapters or application
runtime. Add external-consumer and dependency-closure checks with the first
working engine package; C ABI and driver tooling require their own actual consumers.

Follow [VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) for preservation and execution
tracks. Keep interpreter/macro implementations separate from supplied host
interfaces, and keep native fixture inspection inert. Add required host-call,
event/transaction, cancellation and budget evidence with actual working slices;
source parsing or a planned API is not native language/authoring evidence.

Use [WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) for future provider work.
Discovery must remain inert; qualify individual signatures/behaviors and denied
operations. Windows/native/helper dependencies stay optional and target-specific.
Test replay ownership and required database validation independently; API emulation
or in-process capability checks do not establish isolation or native compatibility.

Use [ROADMAP.md](ROADMAP.md) for application priorities and feature-local contracts.
Follow [application corpus policy](testing/ACCESS_APPLICATIONS.md) for provenance,
disposable copies, exact oracle profiles and independent operation outcomes.
Introduce runnable comparisons with working slices rather than empty harnesses;
inspection must not execute startup, ribbon callbacks or saved data tasks.

Expressions build without the parser by default. Enable their optional `sql`
feature for bounded tree evaluation, and run both minimal workspace checks and
the expression package's standalone minimal/all-feature checks. The CLI explicitly
enables `sql`, so workspace checks unify that feature; standalone expression checks
verify the independent parser-free profile. See EXPRESSIONS.md for the explicit
numeric carrier profile, host binding/collation boundaries and evaluation budgets.

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
Core, SQL and expression proptest suites use 256 cases per property and fixed seeds; retain their generated
regression files if a failure occurs. JSON tests deserialize directly into typed
values; production JSON output still needs an explicit format policy for its
consumers.

Review each new dependency's exact version, license, features, targets, MSRV,
native requirements and unsafe/FFI boundaries in DEPENDENCIES.md. Commit a tested
lockfile and keep experimental candidates separate from adopted dependencies.
Toolchain/dependency upgrades must update these records and rerun applicable CI.
Do not imply native or browser qualification from a successful headless build.

## Native inspection

`tablecraft inspect <path>` reports the bounded header profile in
[NATIVE_READING.md](NATIVE_READING.md). It opens only the named file read-only,
buffers output and never executes database content. It requires a stable/offline
source; native shared-file coordination is not implemented.

## CLI behavior

No arguments, --help and -h print help. --version and -V print package identity.
`tablecraft query parse "<SQL>"` parses one bounded Access query and prints
normalized SQL plus a newline. It never executes SQL, opens external databases
or reads ACCDB files. Declarations and action syntax can be inspected safely as
syntax. Invalid/unsupported SQL or argument errors exit 2, write diagnostics to
stderr and produce no success SQL. Query diagnostics include locations and omit
SQL payloads. Unsupported commands/options still fail through clap.

`tablecraft query eval "<SELECT>"` calculates scalar projections of one source-free
SELECT (optionally ALL). It rejects FROM, WHERE, GROUP BY, HAVING, ORDER BY,
TOP/DISTINCT/DISTINCTROW, external/owner-access options, PARAMETERS, UNION, actions
and schema/saved-query statements. It has no field/parameter bindings or implicit
text collation. Supported operations and explicit semantic limits are recorded
in [EXPRESSIONS.md](EXPRESSIONS.md). It opens no database and invokes no functions,
providers, application objects or active ACCDB content.

Successful evaluation prints two tab-separated lines: headers and one result row.
Aliases name columns; unnamed columns use Expr1, Expr2, etc. Numbers use
locale-independent display, booleans are TRUE/FALSE and unknown is NULL. Text is
double-quoted, so `"NULL"` differs from NULL and `""` is an empty string.
Backslashes, quotes, tab/newline/carriage returns and other control characters
are escaped in text/headers; controls use `\u{hex}` when no short escape applies.
This is a human-readable CLI table, not a production serialization protocol.

Parsing keeps its 65,536-byte/1,024-token ceilings. Eval shares 1,024 node steps
and 65,536 cumulative Text allocation bytes across all columns, with default
evaluator depth/per-value limits. Output is buffered with a 1 MiB logical byte
ceiling and fallible reservation. Parsing, validation, evaluation or formatting
failure produces no stdout; diagnostics identify the one-based projection index
for expression failures, omit SQL/value payloads and exit 2. No query clause is
silently ignored. Output I/O follows the exit behavior below.

Output failures exit 1; a closed stdout pipe exits successfully. Help/version
retain clap's standard short-circuit behavior. Machine-readable database output,
database query execution and file inspection are separate future capabilities.

## Inline retrieval CLI

`tablecraft query run "<SQL>"` prepares/executes the bounded read-only profile in
[QUERY_EXECUTION.md](QUERY_EXECUTION.md). Its empty host catalog accepts source-free
SELECT and inline derived tables/UNION, joins, filtering, ordering, aggregates and
subqueries. Named tables, saved query sources and native files are unavailable.
Action/DDL/crosstab/external forms fail explicitly. `query parse` and the narrower
`query eval` retain their existing contracts.

Repeated `--param NAME=TYPE:VALUE` binds declared parameters; type names are
`boolean` (`true`/`false`), `byte`, `short` (Int16), `long` (Int32), `bigint` (Int64),
`text` (literal text, including empty) and `null` (empty payload: `P=null:`). Other
carriers remain available to library hosts, not this CLI parameter syntax. Names
may contain spaces when the whole argument is shell-quoted. No implicit SQL
literal evaluation or type coercion occurs. At most 256 bindings and 65,536 total
UTF-8 argument bytes are accepted; duplicates/unknown/omitted/wrong-carrier values
fail. SQL INTEGER uses the `long` carrier.

Text comparisons require `--collation ordinal` explicitly. This case-sensitive
Unicode ordinal policy neither normalizes text nor claims Access locale fidelity.
Without it a query that needs text comparison fails; plain Text projection works.

Run uses the query library's shared work ceilings and the same escaped TSV-style
formatter/1 MiB logical output ceiling as eval. Output contains one header and
zero or more rows. All evaluation and formatting finish before stdout, so every
parse/bind/parameter/evaluation/format failure exits 2 with no partial output.
I/O/broken-pipe handling remains as documented above. Diagnostics omit SQL, names
and values. No runtime file access, network provider, VBA or active database
content is invoked.
