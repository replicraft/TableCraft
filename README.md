# TableCraft

A Rust application being developed as a cross-platform Microsoft Access alternative.
Native MDB/ACCDB interoperability and optional peer-to-peer collaboration are
planned capabilities; bounded read-only SQLite queries are available.

**ACE 16 is the primary compatibility target, with 100% compatibility as the
long-term goal.** Implementation and compatibility testing focus on ACE 16;
other Jet/ACE generations are secondary. This is the target, not current support:
ACE 16 has [bounded unencoded reads](docs/NATIVE_ACE16.md); ordinary writes remain
blocked. An opt-in [physical write preview](docs/testing/ACE16_WRITE_PREVIEWS.md)
creates disposable engineering candidates. See the [roadmap](docs/ROADMAP.md).

**The immediate focus is reliable reads and queries for ACE 16 `.accdb`,
ACE 2007/ACE 12 `.accdb` and Jet 4 `.mdb`.** Further write expansion follows that work; the compatibility
goal above does not imply complete current read/query support.

The read scope also includes Access runtime/compiled databases (`.accdr`, `.accde`,
`.mde`), templates (`.accdt`) and signed deployment packages (`.accdc`). Raw native
files already use header-based detection regardless of extension. Bounded
single-database CAB packages now feed inspection, tables, rows and queries;
signatures are reported as unverified. Native compiled/package qualification and
the ACCDT template reader remain pending. See the
[format distinctions and status](docs/features/ACCESS_DISTRIBUTION.md#required-read-scope).

Qualified password-encrypted ACCDB reads prompt for a password with terminal echo
disabled when `-p` is omitted. Explicit global `--password` / `-p` remains available
for noninteractive use:

```sh
tablecraft tables example.accdb --count -p 'your-password'
tablecraft rows example.accdb Contacts -p 'your-password'
tablecraft query run 'SELECT * FROM Contacts' --database example.accdb -p 'your-password'
```

Basic `inspect PATH` reports page encryption status/type;
`encryption inspect PATH` gives the detailed inert envelope report. Neither needs
a password. Prompt input is absent from shell history; an explicit command-line
password can appear in history and process arguments. [Encryption scope](docs/NATIVE_ENCRYPTION.md)
lists supported algorithms and remaining limits. Jet 4 encoded MDB and qualified
legacy Office ACCDB reads now work alongside Agile; unsupported providers and
ordinary encrypted table updates remain blocked. Encryption management supports
fresh add/remove/change-password copies up to 64 MiB:

```sh
tablecraft encryption add example.accdb --output encrypted.accdb
tablecraft encryption remove example.accdb --output unlocked.accdb
tablecraft encryption change-password example.mdb --output changed.mdb
```

Commands prompt for needed credentials with hidden input; existing outputs are
never overwritten. New ACCDB copies use Agile AES-256/SHA-512. Jet 4 retains its
recoverable legacy encoding and separate password field, which does not provide
modern encryption strength. Native Microsoft Access save/reopen remains pending.

The accepted native-project direction is a SQLite-backed `.tcraft` document and
an equivalent unpacked `.tcraftproj/` workspace, with Python and visual actions.
MDB/ACCDB retain their separate native interoperability path. These project,
scripting and conversion features are planned; the existing SQLite reader does
not implement them. See the [decision](docs/decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md)
and [native project requirements](docs/formats/TCRAFT_FORMAT.md).

[Shared application editing](docs/features/AGENT_NATIVE.md) is also an accepted
future direction: humans and agents will use the same structured changesets,
previews and tests through CLI/GUI/MCP adapters. Those services and the MCP server
are not implemented; current CLI capabilities are listed below.

The accepted [External Data direction](docs/features/EXTERNAL_DATA.md) separates
built-in Access/SQLite backends from future external SQL Server, ODBC and SharePoint
Lists adapters. Import/export, saved tasks and the ribbon remain planned.

The current implementation is a headless build foundation: a clap-based
CLI with help/version, query parsing, scalar SELECT evaluation and bounded inline/native-table retrieval, plus a shared library for exact scalar values,
scoped IDs, typed validation errors and resource budgets. The `tablecraft-sql` library
parses [bounded Access query syntax](docs/SQL_PARSER.md) into owned syntax types;
the CLI can print normalized SQL, calculate supported source-free expressions,
or execute bounded retrieval over inline rows and supported native scalar tables. Read-only [native header and catalog inspection](docs/NATIVE_READING.md) is available;
[saved-query fragment inspection](docs/NATIVE_SAVED_QUERIES.md) uses `inspect --query NAME` without running SQL;
`query saved --database PATH NAME` reconstructs a bounded local SELECT profile into owned syntax;
`query run --saved NAME --database PATH` explicitly executes qualified saved local SELECTs
with the existing parameter and comparison controls; named local SELECT dependencies
resolve under bounded planning, parameter-scope and cycle checks;
[Bounded offline Jet 4 UPDATE copies](docs/QUERY_EDITING.md) are available;
desktop and browser interfaces remain unimplemented.
`tablecraft-expressions` provides the first [scalar evaluation rules](docs/EXPRESSIONS.md)
over core values, independently of database execution.
`tablecraft-query` now supplies [bounded read-only retrieval](docs/QUERY_EXECUTION.md)
over explicit host sources, using [validated scalar schema contracts](docs/SCHEMA.md)
from `tablecraft-schema`. `tablecraft-db` owns shared snapshots and
[snapshot-scoped edit contracts](docs/DB.md).
`tablecraft-model` supplies [shared object metadata](docs/MODEL.md) consumed by the
native reader: identity, name, category and parent references.
`tablecraft-io` supplies [bounded synchronous reads](docs/IO.md), independently of
native formats, with shared work limits, cancellation and typed I/O errors.
Explicit [offline read transactions](docs/NATIVE_TRANSACTIONS.md) capture bounded
immutable images under OS locks. Optional library [journaled native edits](docs/NATIVE_WRITES.md)
support a restricted fixed Int32 profile; native shared-file coordination remains unimplemented.
The CLI exposes retrieval through `query run`; `tablecraft-access` connects
[native scalar tables](docs/NATIVE_QUERIES.md) using explicit `--database` input.
`tablecraft-sqlite` supplies [read-only SQLite snapshots](docs/SQLITE.md) through
`query run --sqlite PATH`; query semantics stay in TableCraft. The optional
`tablecraft-sqlite/async` feature adds tokio-rusqlite worker queries; CLI builds
keep that runtime boundary disabled.
Jiff supplies civil calendar operations; native layouts/text decoding use
zerocopy, wide and bytemuck at validated byte boundaries. The
[benchmark record](docs/testing/NATIVE_BENCHMARKS.md) includes improvements and regressions.

Current format coverage is deliberately narrower than the compatibility targets:

| Format family | Current capability | Writes |
| --- | --- | --- |
| Jet 1.0 / 1.1 MDB | Experimental research target; not implemented | No |
| Jet 2.x MDB | Experimental research target; not implemented | No |
| Jet 3 MDB | Header inspection only; table reads planned | No |
| Jet 4 MDB | Limited unencoded metadata, scalar and qualified index reads | Bounded offline fixed Int32 updates |
| ACE 12 ACCDB | Limited unencoded metadata, scalar/index reads, raw complex links, multivalues and stored attachment/history reads | No |
| ACE 14 / 15 ACCDB | Bounded ordinary ACE 2010-family table reads, including qualified Agile inputs; broader fields/features remain unqualified | No |
| ACE 16 ACCDB | Bounded unencoded 2016/2019-family metadata, scalar (including Large Number and 2019 Extended DateTime), qualified index reads | No |
| SQLite | Bounded ordinary scalar-table reads; optional async worker | No |

Engine release numbers do not uniquely identify file layouts. Header inspection
does not qualify table decoding, encryption or application objects. See the
[native profile matrix](docs/NATIVE_READING.md#compatibility-targets) and
[SQLite limits](docs/SQLITE.md). Jet 1/2 may become limited read-only extraction
profiles after fixture/comparison evidence; they do not gate the main engine or UI.

Qualified read sources can be up to 2 GiB; [bounded page/query limits](docs/testing/LARGE_FILE_READS.md)
still apply. Complete-image preservation and editing retain a separate 64 MiB cap.

For table browsing without SQL (use a stable/offline copy):

```sh
tablecraft tables example.accdb --count
tablecraft rows example.accdb --all
tablecraft rows example.mdb --all --limit 250 --offset 0
```

`--count` inserts stored local `Row Count` metadata after `Kind`; linked tables
show `N/A`. These are declared counts, not a full record-integrity audit.
`rows --all` displays named user-table sections with a page per local table
(default 100 rows each). Linked/unknown sources get an explicit unopened status;
system tables stay hidden. Existing scalar/collection opt-ins and read/output
limits apply to the entire command.

Terminal browsing uses comfy-table borders; redirected output remains TSV.
Use `--format table` or `--format tsv` to select explicitly. Inspection also uses
compact bordered summary/detail sections in terminals. System tables are hidden
by default; add `--show-system` to `tables` or `inspect --catalog` to include them.
TSV encoding uses the `csv` crate with a tab delimiter and standard quoting.
Explicit `inspect --table NAME --attachments COLUMN` and `--version-history MEMO`
views expose [stored content](docs/CLI_BROWSING.md#stored-attachments-and-memo-history)
without decoding attachments or selecting latest history. `rows --collections`
also displays these typed stored collections beside ordinary fields in a page. Ordinary scalar
queries can read fields from these mixed tables; references to unavailable fields
and matching wildcards fail explicitly. Qualified `--index TABLE INDEX` reads
also support these scalar views.

```sh
cargo run --locked -p tablecraft-cli -- tables path/to/example.accdb --details
cargo run --locked -p tablecraft-cli -- rows path/to/example.accdb "Customers" --limit 25
cargo run --locked -p tablecraft-cli -- rows path/to/example.mdb "Customers" --limit 25 --offset 25
```

The same commands handle qualified ACE 16, ACE 2007 and Jet 4 scalar tables. `tables` labels
user/system tables from catalog flags, including linked entries; listing does not imply
that every entry is readable. `rows` decodes only a bounded page of selected records
and rejects unsupported field layouts explicitly. Add `--stored-calculations` for
qualified ACE 16 cached calculation results, labelled `[cached]`; formulas never
execute and stored results may be stale. Add `--multivalues` for qualified ACE 2007/ACE 16
stored lists beside scalar cells; combine both flags for mixed cached results.
Collection validation retains complete parent/child snapshot limits. Earlier root slots are scanned
for offsets; unselected payloads are not integrity-checked. See [browsing limits](docs/CLI_BROWSING.md).

```sh
cargo run --locked -p tablecraft-cli -- --help
cargo run --locked -p tablecraft-cli -- --version
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --catalog
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --table ROULETTE_TB
cargo run --locked -p tablecraft-cli -- query run --database path/to/copy.accdb "SELECT TOP 5 * FROM ROULETTE_TB ORDER BY ORDR_NBR"
cargo run --locked -p tablecraft-cli -- query run --sqlite path/to/example.sqlite "SELECT TOP 5 * FROM Items ORDER BY ID"
cargo run --locked -p tablecraft-cli -- query update "UPDATE Items SET Score=Score+7 WHERE Id=2" --database path/to/offline.mdb --output path/to/new-copy.mdb
cargo run --locked -p tablecraft-cli -- query run --database path/to/copy.accdb "SELECT NBR_VALUE FROM ROULETTE_TB WHERE NBR_VALUE LIKE '2*'" --like-mode ansi89 --collation ordinal
cargo run --locked -p tablecraft-cli -- query parse "SELECT Code FROM Samples WHERE Code > 1"
cargo run --locked -p tablecraft-cli -- query saved --database docs/examples/Access_Example_VBA.accdb '~sq_c100_Main_Menu_frm~sq_ccboCREWID'
cargo run --locked -p tablecraft-cli -- query eval "SELECT 2 + 3 * 4 AS Total, 'Hello' & ' world' AS Greeting"
cargo run --locked -p tablecraft-cli -- query run "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 3 UNION ALL SELECT 2) AS q WHERE q.ID > 1 ORDER BY q.ID DESC"
```

`query eval` prints one header/result pair, bordered in terminals and TSV when
redirected (`14` and `"Hello world"` in this example). It accepts source-free SELECT projections; database sources,
query clauses, parameters, actions and unsupported expression forms are rejected.
`query run` in the example prints ID rows `3` and `2`. It supports inline derived
tables, joins, aggregates, unions and subqueries, with optional typed `--param`
bindings and explicit `--collation ordinal`. Add `--database path/to/copy.accdb`
for supported local scalar tables; other hosts can supply their own Source.
Native index metadata is available with `inspect PATH --table NAME --indexes`;
`query run --database PATH --native-bytes 67108864` explicitly allows 64 MiB of
cumulative native reads/allocations. The default is 16 MiB; row/work/query/output
limits stay unchanged. See [native query controls](docs/NATIVE_QUERIES.md).

`query run --database PATH --index TABLE INDEX` explicitly selects qualified native
indexed reads, including composites and single General legacy/General 2010 Short
Text indexes. Text reads verify native overflow digests and row-key agreement;
text range seeks and SQL locale comparison remain pending. Safe single-Int32/Int64 filters can use typed native
seeks; other qualified profiles retain complete reads. See [native index limits](docs/NATIVE_INDEXES.md) and
[CLI behavior and output](docs/DEVELOPMENT.md#cli-behavior).
`inspect PATH --relationships` reports [relationship declarations](docs/NATIVE_RELATIONSHIPS.md).
`inspect PATH --table NAME --constraints` audits [field and foreign-key records](docs/NATIVE_CONSTRAINTS.md).
`inspect PATH --table NAME --properties` reports [inert property declarations](docs/NATIVE_PROPERTIES.md).
`inspect PATH --table NAME --check` combines sequential/indexed reads and reports
[validation coverage](docs/NATIVE_CHECKS.md), including unchecked profiles.

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

Original TableCraft code is licensed under [MIT](LICENSE). The native General
text-key encoder and bundled weight data include Apache-2.0 adaptations from
Jackcess; preserve their [license and notices](crates/engines/tablecraft-ace/data/general/NOTICE)
when distributing them. The ACE package records the combined license obligations.

`query update` requires a fresh output path and never changes its input. Only the
[documented fixed Int32 profile](docs/QUERY_EDITING.md) is supported; ACE writes,
indexed-field updates and unavailable rules/content are rejected.
