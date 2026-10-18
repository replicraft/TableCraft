# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability, SQLite, and optional peer-to-peer collaboration
are planned capabilities.

The current implementation is a headless build foundation: a clap-based
CLI with help/version, query parsing and scalar SELECT evaluation, plus a shared library for exact scalar values,
scoped IDs, typed validation errors and resource budgets. The `tablecraft-sql` library
parses [bounded Access query syntax](docs/SQL_PARSER.md) into owned syntax types;
the CLI can print normalized SQL or calculate supported source-free expressions. Database inspection,
editing, desktop and browser interfaces are not implemented yet.
`tablecraft-expressions` provides the first [scalar evaluation rules](docs/EXPRESSIONS.md)
over core values, independently of database execution.

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
cargo run --locked -p tablecraft-cli -- query parse "SELECT Code FROM Samples WHERE Code > 1"
cargo run --locked -p tablecraft-cli -- query eval "SELECT 2 + 3 * 4 AS Total, 'Hello' & ' world' AS Greeting"
```

`query eval` prints one tab-separated header/result pair (`14` and `"Hello world"`
in this example). It accepts source-free SELECT projections; database sources,
query clauses, parameters, actions and unsupported expression forms are rejected.
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

Licensed under [MIT](LICENSE).
