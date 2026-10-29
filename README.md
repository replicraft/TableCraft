# TableCraft

A cross-platform Microsoft Access reimplementation in development, targeting
faithful native MDB/ACCDB behavior and familiar Access workflows. The CLI supports
bounded browsing and queries; the desktop UI and general native editing are
still in development.

Current work focuses on Microsoft Access compatibility, with ACE 16 primary.
SQLite and `.tcraft` projects are paused, including Python automation and project
conversion. The [product direction](docs/decisions/0003_ACCESS_BEHAVIORAL_PARITY.md)
sets the target for Access workflows, defaults, restrictions and documented quirks.

## Compatibility and planned tiers

| Format | Current support | Planned tier |
| --- | --- | --- |
| ACE 16 `.accdb` · Access 2016 and later | Bounded reads and queries | **Tier 1 — primary:** full native compatibility |
| Jet 4 `.mdb` · Access 2000–2003 | Bounded reads and queries; limited offline copy edits | **Tier 2 — secondary:** maintain and expand qualified support |
| ACE 12 `.accdb` · Access 2007 | Bounded reads and queries | Tier 2 |
| ACE 14 `.accdb` · Access 2010 | Bounded ordinary table reads; advanced variants need qualification | Tier 2 |
| ACE 15 `.accdb` · Access 2013 | Not separately qualified; support follows the actual stored profile | Tier 2 |
| Jet 3 `.mdb` · Access 95/97 | Header recognition | **Tier 3 — legacy:** planned reads and queries |
| Jet 1/2 `.mdb` · Access 1/2 | Not implemented | **Experimental:** optional read-only research |

Tiers describe planned priorities, not completed compatibility. ACE 16's long-term
target is 100% compatibility; native Access qualification remains pending. VBA
execution is deferred. Runtime/compiled databases and bounded template/deployment
packages use their [documented read profiles](docs/COMPATIBILITY.md).

## Install

Prebuilt archives will be available from
[Releases](https://github.com/replicraft/TableCraft/releases) for Windows, macOS
and Linux. Extract `tablecraft` (`tablecraft.exe` on Windows) onto your `PATH`.
Archives include license notices and SHA-256 checksums accompany each release.

To build from source, install the pinned Rust toolchain and run:

```sh
cargo install --path apps/tablecraft-cli --locked
```

## Use the CLI

```sh
tablecraft --help
tablecraft inspect example.accdb
tablecraft tables example.accdb --count
tablecraft rows example.accdb Customers --limit 25
tablecraft rows example.mdb --all --limit 5
tablecraft query run --database example.accdb 'SELECT TOP 10 * FROM Customers'
```

Omit the table name in `rows` when the database has exactly one user table.

Tables display with borders in terminals and as TSV when redirected. Use
`--format table` or `--format tsv` to choose explicitly. System tables are hidden
from listings by default; add `--show-system` to `tables` or `inspect --catalog`
to include them. Commands that read encrypted
files prompt for a password when needed; `-p` / `--password` is also available.

See the [CLI guide](docs/cli/CLI.md) for saved queries, multivalue fields, encryption,
streaming and resource controls.

Explicit [experimental write-copy commands](docs/access/writes/ENGINEERING_COPIES.md)
provide ACE 16 UPDATE, INSERT and DELETE previews on fresh files. General native
saves remain in development.

## Documentation

- [Documentation index](docs/README.md)
- [Compatibility and support tiers](docs/COMPATIBILITY.md)
- [Backend roadmap](docs/ROADMAP.md)
- [Implementation status](docs/IMPLEMENTATION.md)
- [Development](docs/development/DEVELOPMENT.md)
- [Releasing the CLI](docs/development/RELEASING.md)

## License

[Apache-2.0](LICENSE). Distributed builds also include
[third-party notices](packaging/ThirdPartyLicenses.txt).
