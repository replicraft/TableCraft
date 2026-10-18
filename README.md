# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability, SQLite, and optional peer-to-peer collaboration
are planned capabilities.

The current implementation is a headless build foundation: a clap-based
CLI with help/version, query parsing, scalar SELECT evaluation and bounded inline/native-table retrieval, plus a shared library for exact scalar values,
scoped IDs, typed validation errors and resource budgets. The `tablecraft-sql` library
parses [bounded Access query syntax](docs/SQL_PARSER.md) into owned syntax types;
the CLI can print normalized SQL, calculate supported source-free expressions,
or execute bounded retrieval over inline rows and supported native scalar tables. Read-only [native header and catalog inspection](docs/NATIVE_READING.md) is available;
database editing, desktop and browser interfaces are not implemented yet.
`tablecraft-expressions` provides the first [scalar evaluation rules](docs/EXPRESSIONS.md)
over core values, independently of database execution.
`tablecraft-query` now supplies [bounded read-only retrieval](docs/QUERY_EXECUTION.md)
over explicit host sources, using [validated scalar schema contracts](docs/SCHEMA.md)
from `tablecraft-schema`.
`tablecraft-model` supplies [shared object metadata](docs/MODEL.md) consumed by the
native reader: identity, name, category and parent references.
`tablecraft-io` supplies [bounded synchronous reads](docs/IO.md), independently of
native formats, with shared work limits, cancellation and typed I/O errors.
The CLI exposes retrieval through `query run`; `tablecraft-access` connects
[native scalar tables](docs/NATIVE_QUERIES.md) using explicit `--database` input.
Jiff supplies civil calendar operations; native layouts/text decoding use
zerocopy, wide and bytemuck at validated byte boundaries. The
[benchmark record](docs/testing/NATIVE_BENCHMARKS.md) includes improvements and regressions.

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --catalog
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --table ROULETTE_TB
cargo run --locked -p tablecraft-cli -- query run --database path/to/copy.accdb "SELECT TOP 5 * FROM ROULETTE_TB ORDER BY ORDR_NBR"
cargo run --locked -p tablecraft-cli -- query parse "SELECT Code FROM Samples WHERE Code > 1"
cargo run --locked -p tablecraft-cli -- query eval "SELECT 2 + 3 * 4 AS Total, 'Hello' & ' world' AS Greeting"
cargo run --locked -p tablecraft-cli -- query run "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 3 UNION ALL SELECT 2) AS q WHERE q.ID > 1 ORDER BY q.ID DESC"
```

`query eval` prints one tab-separated header/result pair (`14` and `"Hello world"`
in this example). It accepts source-free SELECT projections; database sources,
query clauses, parameters, actions and unsupported expression forms are rejected.
`query run` in the example prints ID rows `3` and `2`. It supports inline derived
tables, joins, aggregates, unions and subqueries, with optional typed `--param`
bindings and explicit `--collation ordinal`. Add `--database path/to/copy.accdb`
for supported local scalar tables; other hosts can supply their own Source.
Native index metadata is available with `inspect PATH --table NAME --indexes`;
`query run --database PATH --index TABLE INDEX` explicitly selects supported Int32
indexed snapshots. See [native index limits](docs/NATIVE_INDEXES.md) and
[CLI behavior and output](docs/DEVELOPMENT.md#cli-behavior).

Rust 1.99.0 with rustfmt and Clippy is pinned in `rust-toolchain.toml`.
The package is `tablecraft-cli`; the executable is `tablecraft`.
See [development and feature policy](docs/DEVELOPMENT.md) for checks and
[dependency records](docs/DEPENDENCIES.md) for the adopted baseline.
The [dependency/reference review](docs/DEPENDENCY_REVIEW.md) records evaluated
candidates separately from installed packages.

See [implementation status](docs/IMPLEMENTATION.md) for current progress.
The [core type reference](docs/CORE_TYPES.md) records implemented contracts and
the Microsoft documents used to define them.
Current technology choices and SQL parser boundaries are recorded in
[TECH_STACK.md](docs/TECH_STACK.md).
The [workspace architecture](docs/ARCHITECTURE.md) records current domain groups,
package responsibilities and the direction for progressive extraction.
The planned [standalone native engine](docs/NATIVE_ENGINE.md), `tablecraft-ace`,
has bounded header/catalog/schema and scalar table reading; writes, C ABI and
ODBC remain unimplemented and the engine is not published.
The [VBA scope decision](docs/VBA_COMPATIBILITY.md) retains existing-code preservation
and dependency detection as requirements; both remain unimplemented. VBA execution,
authoring, editor/debugger, VBA-specific DAO/ADO, Windows API emulation, COM/Wine and
RustVBA integration are deferred. VBA-dependent buttons/events cannot run.
Access table data macros remain in compatibility scope: required semantics must
work or affected writes must be blocked. [Windows/Wine research](docs/WINDOWS_COMPATIBILITY.md)
is retained outside the active roadmap.
The [compatibility-driven roadmap](docs/ROADMAP.md) records application requirements
and priorities. The [application corpus inventory](docs/testing/ACCESS_APPLICATIONS.md)
has one candidate and no native compatibility results yet.

Licensed under [MIT](LICENSE).
