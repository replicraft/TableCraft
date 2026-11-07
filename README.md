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
| ACE 16 `.accdb` · Access 2016 and later | Bounded reads and queries; restricted journaled writes | **Tier 1 — primary:** full native compatibility |
| Jet 4 `.mdb` · Access 2000–2003 | Bounded reads and queries; qualified offline Int32 updates | **Tier 2 — secondary:** maintain and expand qualified support |
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
tablecraft create new.accdb --blank
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
saves remain in development; a separate [journaled BeforeChange profile](docs/access/writes/NORMAL_ACE_BEFORE_CHANGE.md)
supports restricted Int32 and [fixed-scalar updates](docs/access/writes/NORMAL_ACE_FIXED_SCALARS.md).
The [ordinary-content route](docs/access/writes/NORMAL_ACE_ORDINARY_FIXED.md)
also supports fixed updates to qualified tables, with a separate
[fitting Text/Binary route](docs/access/writes/NORMAL_ACE_FITTED.md) and
[restricted row deletion](docs/access/writes/NORMAL_ACE_DELETE.md).
Qualified targets also accept [explicit-row insertion](docs/access/writes/NORMAL_ACE_INSERT.md).

## Desktop preview

Start the native desktop preview with an optional database path:

```sh
cargo run -p tablecraft-desktop -- path/to/example.accdb
```

The first desktop slice provides table navigation, bounded datasheets, field
inspection and a SQL editor over existing Access services. It opens a snapshot of a stable file up to
64 MiB; close other writers first. Restricted reviewed offline cell edits are
implemented on macOS/Linux and local-NTFS Windows; actual Windows execution
remains unqualified. General editing, designers and desktop release
packaging remain in development. Interrupted TableCraft saves have an explicit
[Open-time recovery flow](docs/ui/RECOVERY.md). See the [desktop guide](docs/ui/DESKTOP.md).
File → New → Blank database creates an unencrypted ACE 16 ACCDB. Windows
creation uses local NTFS and still requires runtime qualification.
Existing destinations are never replaced. The new file opens with an unsaved
`Table1` draft. Its bounded initial schema can be saved; qualified stored tables
also support reviewed row insertion. Names, supported types and bounded field
properties can be edited in the initial table's Design view. Fields can also be
named in their headers, and a first record can be entered before saving the table.
That record then uses a separate reviewed row save. General schema authoring
remains unfinished. Windows saves retain an intact retired journal
artifact rather than claiming durable deletion.
See [database creation](docs/ui/DATABASE_CREATION.md) for its limits.
Use `--mock` for [synthetic UI review](docs/ui/PRESENTATION_PREVIEW.md), with
light/dark themes and interface languages under File → Options.

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
