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
python3 scripts/check_dependency_boundaries.py
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo check -p tablecraft-schema --no-default-features --locked
cargo check -p tablecraft-query --no-default-features --locked
cargo check -p tablecraft-model --no-default-features --locked
cargo check -p tablecraft-db --no-default-features --locked
cargo check -p tablecraft-io --no-default-features --locked
cargo check -p tablecraft-ace --no-default-features --locked
cargo clippy -p tablecraft-ace --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-ace --all-features --locked
cargo check -p tablecraft-access --no-default-features --locked
cargo check -p tablecraft-sqlite --no-default-features --locked
cargo clippy -p tablecraft-sqlite --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-sqlite --all-features --locked
cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings
cargo test --workspace --no-default-features --locked
cargo clippy -p tablecraft-io --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-io --all-features --locked
cargo clippy -p tablecraft-expressions --all-targets --no-default-features --locked -- -D warnings
cargo test -p tablecraft-expressions --no-default-features --locked
cargo clippy -p tablecraft-expressions --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-expressions --all-features --locked
cargo clippy -p tablecraft-core --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-core --all-features --locked
cargo build --workspace --release --no-default-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo run --locked -p tablecraft-cli -- --version
```

Declare dependencies in root `[workspace.dependencies]`: registry versions, local
crate paths and shared feature/default selections have one owner. Member manifests
use `dependency.workspace = true`, or `{ workspace = true, ... }` for local optional
flags and consumer-specific features. Jiff uses std/perf-inline for civil calendars;
its direct expression dependency remains SQL-feature-only and core Serde stays optional.
Workspace inheritance does not add every listed dependency to every package.
Recheck standalone feature profiles when changing shared features, which are additive.
The I/O journal feature enables optional SHA-256 integrity for the working image
transaction protocol. Normal CLI/native-reader builds leave it disabled; verify
the standalone all-feature I/O profile as well as the default workspace. See
[journal scope and durability preconditions](IMAGE_JOURNAL.md).

Group existing root dependency declarations by purpose, keeping related registry
and local package responsibilities easy to scan. Grouping does not change package
inheritance or authorize unused entries. Consult the complete
[candidate/reference review](DEPENDENCY_REVIEW.md) before adopting alternatives;
actual adoption still needs the exact version/feature/target checks below.

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

`tablecraft-io` owns format-independent synchronous read primitives and their
retry/cancellation tests; see [IO.md](IO.md). Keep native geometry/format errors in
ACE and map I/O errors there. Read byte charging stays explicit at the consumer;
I/O charges work before callbacks. The I/O crate uses core budgets without importing
query, storage or application packages. Standalone checks exercise this boundary.

The native engine must satisfy [NATIVE_ENGINE.md](NATIVE_ENGINE.md): keep
`tablecraft-ace` independently usable and keep `tablecraft-access` as an adapter.
Backend-neutral/query contracts cannot import concrete adapters or application
runtime. Add external-consumer and dependency-closure checks with the first
working engine package; C ABI and driver tooling require their own actual consumers.

Follow [VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) for required preservation and
inert dependency detection. VBA execution/authoring, editor/debugger, VBA-specific
DAO/ADO, Windows/COM/Wine and RustVBA integration are deferred. Preserve modules
through qualified unrelated saves; report dependent actions as unavailable. Keep
Access macros and non-VBA automation in scope, and block writes whose required
validation/data macros are unavailable. Reserve the future host boundary without
scaffolding packages. [Windows research](WINDOWS_COMPATIBILITY.md) is deferred;
ordinary OS I/O and engineering oracle work are independent of that decision.

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

## Native benchmarks

[Larger-source checks](testing/LARGE_FILE_READS.md) separately exercise high-offset
pages and padded independent inputs. They are correctness/resource evidence, not
large-table throughput benchmarks.

[Decimal arithmetic measurements](testing/DECIMAL_ARITHMETIC.md) use a separate
clap-based release example and Hyperfine; no database I/O or query planning is timed.

[Currency conversion measurements](testing/CURRENCY_CONVERSIONS.md) separately
compare the expression library with the existing integer-kernel reference using
the release example and Hyperfine. They are not native I/O benchmarks.

[Native benchmark instructions/results](testing/NATIVE_BENCHMARKS.md) use release
public-API workloads and bitmap contenders with Hyperfine. Build before timing,
compare deterministic checksums, retain samples and report regressions as well as
improvements. Benchmark tools stay outside production dependency closures.

## Native table browsing

`tablecraft tables PATH` lists catalog table names and system/user markers from
native flags, without reading their rows. System tables are hidden by default;
`--show-system` includes flagged system tables. The same flag on `inspect --catalog` filters tables while retaining other
object kinds.
`tables PATH --details` adds captured local/non-ODBC/ODBC storage classification;
it does not connect to external sources. `inspect PATH --table NAME` summarizes
supported scalar carriers and unsupported columns before its original metadata
columns, without reading user records. These are metadata support reports, not
successful row reads or full integrity checks.
`tablecraft rows PATH TABLE --limit 25 --offset 25` displays a page of native
scalar values, without SQL construction or evaluation. Default limit is 100;
accepted limits are 1–16,384 and offsets are unsigned 32-bit live-row ordinals.
Both use `tablecraft-access::NativeBrowser` and require stable/offline input.
`rows --stored-calculations` explicitly permits qualified cached ACE 16 results,
labelled `[cached]`; results can be stale and formulas are never run. Its distinct
`stored_page`/`StoredTablePage` API preserves inert declarations and column origins.
`rows --multivalues` displays qualified stored lists beside scalars; both flags
allow mixed cached results. The distinct `multivalue_page` API validates complete
parent-key/child snapshots under their existing limits before returning explicit
collection cells, without lookup/formula execution.
ACE owns the bounded page decoder; only selected payloads are decoded. Earlier
live slots are scanned for offsets, and unselected payloads are not audited.
Shared budget and output limits still apply. See [browsing scope and evidence](CLI_BROWSING.md).

## Native inspection

`tablecraft inspect <path>` reports the bounded header profile; `--catalog` adds
object descriptors and native origins under the narrower catalog profile in
[NATIVE_READING.md](NATIVE_READING.md). It opens only the named file read-only,
buffers output and never executes database content. It requires a stable/offline
source; native shared-file coordination is not implemented.

Shared metadata belongs to [MODEL.md](MODEL.md); native codes and physical references
stay in the engine. Catalog CLI formatting uses the shared escaped table formatter
and 1 MiB logical output bound. No header is published if catalog decoding fails.

`inspect <path> --table NAME --indexes` reports native logical/physical index
metadata and ordered key components without traversing indexes or rows; see
[NATIVE_INDEXES.md](NATIVE_INDEXES.md).

Native table schemas are available through `inspect <path> --table NAME`; see
[NATIVE_TABLES.md](NATIVE_TABLES.md) for native metadata and the scalar carrier boundary.

`inspect <path> --table NAME --stored-calculation COLUMN` explicitly reads
qualified ACE 16-family stored Int32 calculation results. It labels stored values
and zero-based snapshot-local ordinals; it never recalculates an expression or
enables ordinary calculated-field queries. Other inspection modes conflict with
this selection. See [stored calculation scope](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-int32-calculations).

`inspect <path> --table NAME --complex-links COLUMN` reports checked stored parent
keys and raw child counts in a scoped complex storage view. It preserves encoded
attachment payloads and does not enable collection query expansion or editing.
See [complex link scope](features/ACCESS_ADVANCED_FIELDS.md#scoped-complex-parent-and-child-links).

`inspect <path> --table NAME --check` reads the complete scalar table and eligible
Int32/Int64 indexes, reporting checked/unverified coverage separately. It conflicts
with --indexes. Exit zero means a completed report, not full database integrity;
see [check limits and evidence](NATIVE_CHECKS.md). Unsupported/failed selected
reads, cancellation or budget exhaustion produce no partial stdout.

## Structured diagnostics

`-v` / `--verbose` enables debug operation diagnostics on stderr; repeat (`-vv`)
for low-level I/O spans. Normal invocation stays quiet, and stdout/exit behavior
is unchanged. No RUST_LOG/environment filter is read. See [TRACING.md](TRACING.md)
for recorded fields, ownership and verification. Libraries never install a subscriber.

## CLI source organization

All command arguments, dispatch and handlers live under
`apps/tablecraft-cli/src/commands/`. `commands/mod.rs` composes the top-level
subcommands; `tables.rs`, `rows/`, `inspect/` and `query/` own their command
features. Inspection and query argument declarations have dedicated `args.rs`
modules. Native command errors are shared in `commands/error.rs`.
`main.rs` retains the top-level clap Parser, global flags, diagnostics startup,
stdout/stderr and exit handling. Shared `native`, `output` and `presentation`
modules remain outside the command tree. This layout adds no library or runtime
dependencies and leaves reusable database work in the backend/engine crates.

## CLI behavior

No arguments, --help and -h print help. --version and -V print package identity.
`tablecraft query parse "<SQL>"` parses one bounded Access query and prints
normalized SQL plus a newline. It never executes SQL, opens external databases
or reads ACCDB files. Declarations and action syntax can be inspected safely as
syntax. Invalid/unsupported SQL or argument errors exit 2, write diagnostics to
stderr and produce no success SQL. Query diagnostics include locations and omit
SQL payloads. Unsupported commands/options still fail through clap.

`tablecraft query saved --database PATH NAME` opens an explicit stable/offline
MDB/ACCDB copy read-only, reconstructs the [qualified local SELECT profile](NATIVE_SAVED_QUERIES.md#owned-select-reconstruction)
and prints normalized SQL. It shares the native host's 16 MiB/16,384-row/one-million-
step budget across fragment reading and adaptation, plus the parser's clamped
limits. It does not bind functions, expand saved sources, activate providers or
execute SQL. Names beginning with a dash can follow `--`. Only `--database`, the
name and global diagnostics options apply; parameter/index/SQLite/write options
fail through Clap. Unknown, ambiguous or unsupported metadata and output errors
exit 2 without partial stdout. SQL output buffers under the common 1 MiB cap;
diagnostics exclude paths, names and fragments. Native concurrent snapshot/locking
guarantees remain unimplemented; use a stable/offline copy.

`tablecraft query eval "<SELECT>"` calculates scalar projections of one source-free
SELECT (optionally ALL). It rejects FROM, WHERE, GROUP BY, HAVING, ORDER BY,
TOP/DISTINCT/DISTINCTROW, external/owner-access options, PARAMETERS, UNION, actions
and schema/saved-query statements. It has no field/parameter bindings or implicit
text collation. Supported operations and explicit semantic limits are recorded
in [EXPRESSIONS.md](EXPRESSIONS.md). Pure NULL/conditional, bounded text/date and checked numeric/Boolean conversion built-ins are supported;
arbitrary/qualified calls remain unavailable. It opens no database and activates
no providers, application objects or active ACCDB content.

Successful evaluation prints headers and one result row. Terminals use comfy-table
borders; redirected stdout uses escaped TSV. Global `--format table|tsv|auto`
selects this policy for query run/eval, native tables/rows and inspection.
Inspection uses compact bordered summary/detail sections; TSV uses the `csv` crate with a tab delimiter and standard quote doubling.
Fields contain escaped display representations; see [TSV parsing](CLI_BROWSING.md).
Aliases name columns; unnamed columns use Expr1, Expr2, etc. Numbers use
locale-independent display, booleans are TRUE/FALSE and unknown is NULL. Text is
double-quoted, so `"NULL"` differs from NULL and `""` is an empty string.
Backslashes, quotes, tab/newline/carriage returns and other control characters
are escaped in text/headers; controls use `\u{hex}` when no short escape applies.
This is a human-readable CLI table, not a production serialization protocol.

Parsing keeps its 65,536-byte/1,024-token ceilings. Eval shares 1,024 node steps
and 65,536 cumulative Text allocation bytes across all columns, with default
evaluator depth/per-value limits. Output is buffered with a 1 MiB logical byte
ceiling and fallible reservation. Bordered presentation also performs a conservative
layout check before entering the dependency, whose internal allocations are not
fallible APIs; see [presentation limits](CLI_BROWSING.md).
Parsing, validation, evaluation or formatting
failure produces no stdout; diagnostics identify the one-based projection index
for expression failures, omit SQL/value payloads and exit 2. No query clause is
silently ignored. Output I/O follows the exit behavior below.

Output failures exit 1; a closed stdout pipe exits successfully. Help/version
retain clap's standard short-circuit behavior. Machine-readable database output
remains a future capability. Bounded native scalar retrieval is documented in
[NATIVE_QUERIES.md](NATIVE_QUERIES.md). Native inspection is documented separately above.

## Inline retrieval CLI

`tablecraft query run "<SQL>"` prepares/executes the bounded read-only profile in
[QUERY_EXECUTION.md](QUERY_EXECUTION.md). Its empty host catalog accepts source-free
SELECT and inline derived tables/UNION, joins, filtering, ordering, aggregates and
subqueries. Optional `--database PATH` supplies supported local scalar tables through
`tablecraft-access`; see [NATIVE_QUERIES.md](NATIVE_QUERIES.md). Linked sources remain unavailable. Named local saved SELECT dependencies now resolve
under shared planning/source limits and cycle checks. Explicit
`query run --saved NAME --database PATH` reads and executes one qualified native
saved SELECT instead of positional SQL; see [saved execution](NATIVE_SAVED_QUERIES.md#explicit-saved-select-execution).
With --database, `--index TABLE INDEX` requires an eligible native index for that
table's snapshot (single Int32 or qualified ACE 16-family Int64 key,
ascending/descending, including NULL). It has no
silent fallback. Other tables retain sequential reads. Binding reads scalar
metadata only; safe matching Int32 filters can select native ranges before the
original WHERE runs. Other predicates use complete snapshots. TOP/grouping/sorting
remain in the executor. See [NATIVE_INDEXES.md](NATIVE_INDEXES.md) and
[safe source constraints](QUERY_EXECUTION.md#safe-source-constraints).

Action/DDL/crosstab/external forms fail explicitly. `query parse` and the narrower
`query eval` retain their existing contracts.

Repeated `--param NAME=TYPE:VALUE` binds declared parameters; type names are
`boolean` (`true`/`false`), `byte`, `short` (Int16), `long` (Int32), `bigint` (Int64),
`text` (literal text, including empty), `date` (the bounded ISO profile in
[NATIVE_TABLES.md](NATIVE_TABLES.md#legacy-datetime)) `guid` (36-byte hyphenated GUID) and `null` (empty payload: `P=null:`). Other
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
and values. Without --database no file is opened. Native file input opens only the explicit
stable/offline source read-only; no network provider, VBA or active database
content is invoked.

## Offline query UPDATE copies

`query update SQL --database INPUT --output NEW_COPY` applies the bounded profile
in [QUERY_EDITING.md](QUERY_EDITING.md). One existing non-NULL fixed
Int32 field in a qualified Jet 4 table can change, with pure expressions/WHERE,
optional alias and explicit text collation. All binding/evaluation/native checks
finish before output creation. Existing paths are never overwritten and input
bytes remain unchanged. Matched and changed row counts print after output sync.
Failures publish no success stdout; a copy I/O failure may leave incomplete output.
This is not an in-place CLI transaction or native Access shared-file operation.

Native Date/Time Extended query values display as hash-delimited ISO civil dates
with exactly seven fractional digits. Legacy DateTime retains its nine-digit
rounded display. The CLI `date` parameter remains a legacy carrier; implicit
legacy/extended conversion is unsupported.

The dependency-policy guard uses Python 3.11+ as an engineering tool and locked,
offline Cargo metadata; it does not install the future application Python host.
Run it and its focused tests when package dependencies, ownership policy or CI
changes. New local packages require an explicit reviewed policy entry. See
[enforced boundaries](architecture/DEPENDENCY_BOUNDARIES.md) for scope and limits.

`inspect PATH --table NAME --features` joins inert schema/property declarations for
calculated/complex and presentation-related fields. It never runs expressions or
loads child values; absent optional declarations print UNKNOWN. Raw complex IDs
are native references, not portable object IDs. This mode conflicts with other
inspection/check modes and requires a stable/offline input like ordinary inspection.

`inspect PATH --table NAME --complex` resolves checked complex type/flat schemas
and reports child-key/identity column names and declared child counts. Counts are
metadata, not decoded/verified child rows. The mode is mutually exclusive with
other check/inspection modes and executes no content or child queries.

Source-free `query eval` now supports eager BETWEEN/NOT BETWEEN and IN/NOT IN lists
through the shared scalar kernels. For example,
`tablecraft query eval "SELECT 2 BETWEEN 3 AND 1 AS InRange"` prints TRUE.
Exact integer/NULL behavior and ordinary buffered-output/error limits apply;
text comparisons still require a qualified explicit collation at a supporting host.


## LIKE retrieval controls

`query run` supports `--like-mode ansi89` or `--like-mode ansi92` together with
an explicit text collation for non-NULL text matches. No database ANSI setting is
inferred. Use `--collation ordinal` for the current CLI scalar ordering profile.
Both options apply to inline and file-backed retrieval; matching remains in the
shared expression/query libraries. `query eval` has no text-policy controls and
`query update` retains its existing restricted profile. [LIKE contracts](PATTERN_MATCHING.md)
record supported patterns, resource accounting and native locale limitations.


`inspect PATH --table NAME --attachments COLUMN` and `--version-history MEMO`
use shared NativeBrowser stored-content APIs. They report complete checked parent
memberships and typed children with exact DATE bits; attachments retain encoded
payloads and history retains stored order. They conflict with other inspection
modes and do not extend ordinary scalar retrieval. See
[limits and verification](CLI_BROWSING.md#stored-attachments-and-memo-history).


`rows PATH TABLE --collections` uses the shared typed stored-collection page API
for lists, encoded attachment metadata and hidden Memo history beside scalar
fields. It conflicts with `--multivalues`; cached results still need separate
`--stored-calculations` opt-in. Complete collection validation remains bounded
and eager, including on empty requests; scalar payloads outside the selected page
remain unchecked. See [the contract](CLI_BROWSING.md#mixed-scalar-and-stored-collection-pages).


Native queries may reference ordinary scalar fields in tables with unavailable
collection/calculation/other fields. Unavailable names remain explicit source
schema metadata: matching wildcards and field references fail, without omissions
or NULL placeholders. Mixed native views use bounded scalar projection snapshots;
ordinary scalar values are still materialized. Explicit indexed mixed projections
use qualified native projected reads; unknown/ineligible indexes fail without
sequential fallback. Dense scalar constraint positions are mapped to native keys. See [the source contract](QUERY_EXECUTION.md#explicit-unavailable-source-fields)
and [native limits](NATIVE_QUERIES.md#scalar-views-of-mixed-native-schemas).
