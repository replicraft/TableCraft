# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability, SQLite, and optional peer-to-peer collaboration
are planned capabilities.

The current implementation is a headless build foundation: a clap-based
CLI with help and version output, plus a shared library for exact scalar values,
scoped IDs, typed validation errors and resource budgets. A separate query library
parses a [bounded Access SELECT subset](docs/SQL_PARSER.md) into owned syntax types;
the CLI does not expose it yet. Database inspection,
editing, desktop and browser interfaces are not implemented yet.

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
```

Rust 1.99.0 with rustfmt and Clippy is pinned in `rust-toolchain.toml`.
The package is `tablecraft-cli`; the executable is `tablecraft`.
See [development and feature policy](docs/DEVELOPMENT.md) for checks and
[dependency records](docs/DEPENDENCIES.md) for the adopted baseline.

See [implementation status](docs/IMPLEMENTATION.md) for current progress.
The [core type reference](docs/CORE_TYPES.md) records implemented contracts and
the Microsoft documents used to define them.
Current technology choices and SQL parser boundaries are recorded in
[TECH_STACK.md](docs/TECH_STACK.md).

Licensed under [MIT](LICENSE).
