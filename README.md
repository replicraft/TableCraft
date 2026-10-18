# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability, SQLite, and optional peer-to-peer collaboration
are planned capabilities.

The current implementation is a headless build foundation: a clap-based
CLI with help/version, query parsing, scalar SELECT evaluation and inline retrieval, plus a shared library for exact scalar values,
scoped IDs, typed validation errors and resource budgets. The `tablecraft-sql` library
parses [bounded Access query syntax](docs/SQL_PARSER.md) into owned syntax types;
the CLI can print normalized SQL, calculate supported source-free expressions,
or execute bounded retrieval over inline rows. Native database inspection,
editing, desktop and browser interfaces are not implemented yet.
`tablecraft-expressions` provides the first [scalar evaluation rules](docs/EXPRESSIONS.md)
over core values, independently of database execution.
`tablecraft-query` now supplies [bounded read-only retrieval](docs/QUERY_EXECUTION.md)
over explicit host sources, using [validated scalar schema contracts](docs/SCHEMA.md)
from `tablecraft-schema`.
The CLI exposes retrieval through `query run`.

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
cargo run --locked -p tablecraft-cli -- query parse "SELECT Code FROM Samples WHERE Code > 1"
cargo run --locked -p tablecraft-cli -- query eval "SELECT 2 + 3 * 4 AS Total, 'Hello' & ' world' AS Greeting"
cargo run --locked -p tablecraft-cli -- query run "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 3 UNION ALL SELECT 2) AS q WHERE q.ID > 1 ORDER BY q.ID DESC"
```

`query eval` prints one tab-separated header/result pair (`14` and `"Hello world"`
in this example). It accepts source-free SELECT projections; database sources,
query clauses, parameters, actions and unsupported expression forms are rejected.
`query run` in the example prints ID rows `3` and `2`. It supports inline derived
tables, joins, aggregates, unions and subqueries, with optional typed `--param`
bindings and explicit `--collation ordinal`. Named tables need a library host
Source; the CLI opens no database files.
See [CLI behavior and output](docs/DEVELOPMENT.md#cli-behavior).

Rust 1.99.0 with rustfmt and Clippy is pinned in `rust-toolchain.toml`.
The package is `tablecraft-cli`; the executable is `tablecraft`.
See [development and feature policy](docs/DEVELOPMENT.md) for checks and
[dependency records](docs/DEPENDENCIES.md) for the adopted baseline.

See [implementation status](docs/IMPLEMENTATION.md) for current progress.
The [core type reference](docs/CORE_TYPES.md) records implemented contracts and
the Microsoft documents used to define them.
Current technology choices and SQL parser boundaries are recorded in
[TECH_STACK.md](docs/TECH_STACK.md).
The [workspace architecture](docs/ARCHITECTURE.md) records current domain groups,
package responsibilities and the direction for progressive extraction.
The planned [standalone native engine](docs/NATIVE_ENGINE.md), `tablecraft-ace`,
will be reusable outside TableCraft; the engine, C ABI and ODBC driver are not
implemented or published yet.
The [VBA compatibility direction](docs/VBA_COMPATIBILITY.md) covers native
preservation, separate VBA/macro execution and future authoring; those language
engines and hosts are not implemented yet.
The [Windows API compatibility direction](docs/WINDOWS_COMPATIBILITY.md) defines
future per-API providers and explicit portability limits; no Windows/COM provider
or binary emulator is implemented.
The [compatibility-driven roadmap](docs/ROADMAP.md) records application requirements
and priorities. The [application corpus inventory](docs/testing/ACCESS_APPLICATIONS.md)
has one candidate and no native compatibility results yet.

Licensed under [MIT](LICENSE).
