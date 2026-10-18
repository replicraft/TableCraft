# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability, SQLite, and optional peer-to-peer collaboration
are planned capabilities.

The current implementation is a headless build foundation: a clap-based
CLI with help and version output. Database inspection, editing, desktop and browser
interfaces are not implemented yet.

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
```

Rust 1.99.0 with rustfmt and Clippy is pinned in `rust-toolchain.toml`.
The package is `tablecraft-cli`; the executable is `tablecraft`.
See [development and feature policy](docs/DEVELOPMENT.md) for checks and
[dependency records](docs/DEPENDENCIES.md) for the adopted baseline.

See [implementation status](docs/IMPLEMENTATION.md) for current progress.

Licensed under [MIT](LICENSE).
