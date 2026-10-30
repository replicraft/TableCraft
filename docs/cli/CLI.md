# CLI guide

The `tablecraft` executable provides local database inspection, browsing and
bounded query execution. This guide describes the 0.1.0 command surface; it does
not imply complete Access compatibility. See [current support and planned tiers](../COMPATIBILITY.md).

Use an explicit stable/offline database copy. Reading a file does not execute
startup macros, VBA, lookup RowSource queries or embedded connection strings.

## Help and source builds

```sh
tablecraft --help
tablecraft --version
tablecraft query run --help
tablecraft rows --help
```

For installation, see the [README](../../README.md#install). From the repository,
`cargo run --locked -p tablecraft-cli -- <arguments>` runs the same CLI.
[Development](../development/DEVELOPMENT.md) documents the pinned toolchain and verification.

## Inspect and browse

```sh
tablecraft inspect example.accdb
tablecraft inspect example.accdb --catalog
tablecraft inspect example.accdb --table Customers --indexes
tablecraft inspect example.accdb --table Customers --extra
tablecraft inspect example.accdb --table Customers --check
tablecraft tables example.accdb --details --count
tablecraft rows example.accdb Customers --limit 25 --offset 25
tablecraft rows example.accdb
tablecraft rows example.mdb --all --limit 5
```

`inspect` reports metadata; `--check` explicitly reads records and qualified
indexes and reports unchecked coverage. `tables --count` shows declared stored
row counts, not a record-integrity audit. `rows` uses physical encounter order;
use an SQL `ORDER BY` when ordering matters. `rows --all` emits a separate page
per local user table and leaves linked targets unopened.

`inspect --table NAME --extra` explicitly reports bounded inert
[table Extra metadata](../access/schema/NATIVE_TABLE_EXTRA.md). It retains numeric
group/property facts without printing stored XML or certifying data-macro absence.
Add `--macro-xml` to explicitly inspect supported
[data-macro XML envelopes](../access/schema/NATIVE_DATA_MACRO_XML.md). Event and
Name metadata stays separate from action execution and absence qualification.

When the database has exactly one user table, `rows PATH` selects it automatically;
system tables do not count. With zero or multiple user tables, supply a table
name or use `--all`. Buffered rows include qualified stored calculations and
collections automatically. Calculated headers say `[cached]`: values may be stale,
and no formula runs. Attachments remain encoded and are displayed as stored
metadata; no files or URLs are opened. Query execution retains its explicit
stored-calculation and collection options.

Output uses bordered tables in terminals and TSV when redirected. Select
`--format table` or `--format tsv` explicitly. TSV uses CSV quoting with a tab
delimiter; displayed Text/Binary/date spellings are presentation, not SQL values.
System tables are hidden from listings by default; add `--show-system` to
`tables` or `inspect --catalog`. See [browsing](CLI_BROWSING.md),
[native reading](../access/NATIVE_READING.md) and [check coverage](../access/NATIVE_CHECKS.md).

## Parse, evaluate and run queries

```sh
tablecraft query parse 'SELECT ID FROM Customers WHERE ID > 10'
tablecraft query eval "SELECT 2+3*4 AS Total, 'Hello' & ' world' AS Greeting"
tablecraft query run --database example.accdb 'SELECT TOP 10 ID,Name FROM Customers ORDER BY ID'
tablecraft query run --sqlite example.sqlite 'SELECT ID,Name FROM Customers ORDER BY ID'
```

`parse` prints normalized syntax without execution. `eval` calculates supported
source-free SELECT expressions. `run` binds and executes the implemented Access
query profile over the selected source; SQLite does not evaluate Access SQL.
`--database` and `--sqlite` are separate, conflicting source choices. Ordinary
query output is buffered before stdout. See [query execution](../query/QUERY_EXECUTION.md),
[expressions](../expressions/EXPRESSIONS.md), [built-ins](../expressions/ACCESS_BUILTINS.md) and [SQLite](../backends/SQLITE.md).

Supply each declared parameter using its exact carrier:

```sh
tablecraft query run --database example.accdb \
  'PARAMETERS Floor LONG; SELECT ID FROM Customers WHERE ID >= Floor' \
  --param Floor=long:10
```

Repeat `--param NAME=TYPE:VALUE` for additional parameters. Examples include
`Label=text:hello`, `Price=currency:0.0001` and `Missing=null:`; the SQL declaration
and supplied type must agree. Shell-quote each SQL argument and any parameter
containing spaces.

Text and contextual conversions use explicit policies rather than ambient locale:

| Option | Purpose |
| --- | --- |
| `--collation ordinal` | Explicit ordinal Text comparisons. |
| `--collation database` | Qualified stored native database/per-field General policies. |
| `--like-mode ansi89` / `ansi92` | Explicit LIKE wildcard convention. |
| `--number-format invariant` | Numeric Text parsing/formatting; `en-us` and `de-de` are also available. |
| `--date-format iso` | Gregorian date parsing/formatting; `en-us` and `de-de` are also available. |
| `--declared-arithmetic` | Select the documented declaration-aware arithmetic profile. |

Other context options and their limits appear in `query run --help` and the
owning [text](../access/indexes/NATIVE_TEXT.md), [numeric](../expressions/formatting/NUMBER_FORMAT.md),
[date](../expressions/dates/DATE_CONVERSIONS.md) and [arithmetic](../expressions/numeric/DECLARED_ARITHMETIC.md) contracts.

## Saved queries and explicitly selected sources

```sh
tablecraft inspect example.accdb --query RecentCustomers
tablecraft query saved --database example.accdb RecentCustomers
tablecraft query run --database example.accdb --saved RecentCustomers
tablecraft query run --database frontend.accdb --link People backend.accdb \
  'SELECT * FROM People'
tablecraft query run --database frontend.accdb --external remote backend.accdb \
  'SELECT * FROM People IN "remote"'
```

`inspect --query` reads inert fragments. `query saved` reconstructs a qualified
SELECT/UNION/crosstab without executing it; `run --saved` executes explicitly.
Linked aliases and external tokens select only files supplied by the caller.
Stored Database/Connect text never opens a target automatically.
See [saved queries](../access/queries/NATIVE_SAVED_QUERIES.md), [named schema discovery](../query/NAMED_SCHEMA_DISCOVERY.md),
[links](../access/queries/NATIVE_LINKED_QUERIES.md) and [external sources](../access/queries/NATIVE_EXTERNAL_QUERIES.md).

## Collections and calculated values

```sh
tablecraft rows example.accdb Issues --multivalues
tablecraft query run --database example.accdb --multivalues \
  'SELECT Title,AssignedTo FROM Issues'
tablecraft query run --database example.accdb --multivalues \
  'SELECT Title,AssignedTo.Value FROM Issues'
tablecraft query run --database example.accdb --multivalues \
  'SELECT ID,AssignedTo,AssignedTo.Value FROM Issues WHERE AssignedTo.Value=7'
tablecraft query run --database example.accdb --multivalues \
  'SELECT ID,Count(AssignedTo.Value) AS AssignedCount FROM Issues GROUP BY ID ORDER BY ID'
tablecraft query preview-calculation --database example.accdb Items Score
```

Bare multivalue fields display complete stored lists on parent rows; `.Value`
selects flattened scalar rows. List order, duplicates, NULL members and empty
lists remain distinct under the [typed result contract](../query/collections/MULTIVALUE_QUERIES.md).
Selecting both retains the full list beside each displayed child. A predicate on
that displayed member filters children while keeping the whole list intact.
Member counts use the scalar `.Value` expression and ignore NULL children; a
parent with no children has count zero. Bare-list Count is unsupported. Native
edge behavior remains a separate qualification gate.
Use `--collections` instead of `--multivalues` to enable qualified attachment and
history members as well. Stored attachment envelopes and explicit
`--decoded-attachments` results are [different representations](../access/collections/STORED_ATTACHMENT_DATA.md).

`--stored-calculations` reads qualified caches, which may be stale. It never
recalculates formulas. `preview-calculation` explicitly recomputes a restricted
formula without saving. For recomputation inside queries, repeated
`--recomputed-column VIEW TABLE COLUMN` selections create an explicit local view.
See [cached values](../access/calculations/CACHED_CALCULATION_QUERIES.md), [previews](../access/calculations/CALCULATION_PREVIEWS.md)
and [recomputed views](../access/calculations/RECOMPUTED_CALCULATION_VIEWS.md).

## Streaming, indexes and limits

```sh
tablecraft rows example.accdb Customers --stream --limit 128
tablecraft query run --database example.accdb --stream --batch-rows 128 \
  'SELECT ID,Name FROM Customers WHERE ID > 0'
```

Explicit streaming publishes TSV batches and can leave earlier output after a
later failure. Query streaming admits the documented SELECT, aggregate/group/HAVING,
fixed DISTINCT and static pipeline profiles; other shapes refuse before cursors.
Collection output, sorting and joins retain buffered execution. See
[streaming rows](STREAMING_ROWS.md) and [streaming queries](STREAMING_QUERIES.md).

`query run --auto-index` chooses only qualified safe index hints and retains the
original predicate and sequential table order. Explicit `--index TABLE INDEX`
requires the selected index and retains selected-read failures. See
[automatic indexes](../access/indexes/AUTOMATIC_INDEXES.md) and [native indexes](../access/indexes/NATIVE_INDEXES.md).

Native reads, query execution, comparison providers and final output have separate
cumulative limits. Relevant flags include `--native-bytes`, `--native-rows`,
`--native-steps`, `--execution-bytes`, `--execution-rows`, `--execution-steps`,
`--comparison-bytes`, `--comparison-steps`, `--text-bytes` and `--output-bytes`.
Available flags depend on the command. Raising one pool does not raise the others
or enable an unsupported profile. Larger stored/decoded values have additional
controls; see [large payloads](../access/storage/LARGE_PAYLOADS.md), [native snapshots](../access/queries/LARGER_NATIVE_QUERIES.md)
and [comparison limits](../query/COMPARISON_BUDGETS.md).

## Encryption and fresh-copy changes

Supported encrypted reads prompt for a password with hidden terminal input when
needed. Global `-p` / `--password` supports noninteractive use, but command-line
credentials can appear in shell history and process arguments.

```sh
tablecraft encryption inspect example.accdb
tablecraft encryption add example.accdb --output encrypted.accdb
tablecraft encryption remove example.accdb --output unlocked.accdb
tablecraft encryption change-password example.mdb --output changed.mdb
tablecraft query update 'UPDATE Items SET Score=Score+7 WHERE Id=2' \
  --database offline.mdb --output updated.mdb
```

Encryption rewrites and qualified Jet 4 fixed Int32 UPDATEs use fresh outputs;
existing files are not overwritten. Encryption failure behavior is described in
its owning guide. Jet 4 UPDATE uses a complete synced temporary and no-clobber
publication. General native saves and in-place CLI editing remain unavailable.
Read [encryption](../access/storage/NATIVE_ENCRYPTION.md), [offline UPDATE limits](../access/writes/QUERY_EDITING.md)
and [native write boundaries](../access/writes/NATIVE_WRITES.md) before using these commands.

## Experimental ACE 16 write copies

```sh
tablecraft query preview-update 'UPDATE Items SET Score=Score+1 WHERE Id=2' \
  --database offline.accdb --output updated.accdb
tablecraft query preview-delete 'DELETE FROM Items WHERE Id=2' \
  --database offline.accdb --output remaining.accdb
tablecraft query preview-insert "INSERT INTO Items (Id, Score) VALUES (4, 42)" \
  --database offline.accdb --output inserted.accdb
```

These commands require schemas admitted by the selected engineering profile.
They keep the original file and publish one complete copy under a fresh name.
UPDATE can combine fixed fields and one fitted Short Text/Binary field through
its default metadata route. Add `--variable` to permit joint DATA placement with
fixed siblings; their INDEX owners must still fit. Mixed `--variable --index-growth`
requests refuse, while the single-variable combination retains its old profile.
INSERT can omit named ordinary fields: only needed
qualified literal defaults or NULL are resolved, while explicit NULL stays
supplied. Positional VALUES remain complete-width. Generated IDs require explicit
`--incrementing` and a named companion field list; ordinary companions may also
be omitted. The complete-row typed TSV command retains its full-width contract.
Encrypted/deployment inputs and unsupported schema/constraint shapes refuse.
The engine's complete-image limit is 64 MiB; native Access saves, required data
macros and in-place transactions remain unqualified.

See [SQL UPDATE](../access/writes/SQL_ENGINEERING_COPIES.md),
[SQL DELETE](../access/writes/SQL_DELETE_COPIES.md),
[SQL INSERT](../access/writes/SQL_INSERT_COPIES.md) and
[typed write-copy inputs](../access/writes/ENGINEERING_COPIES.md) for exact limits.
The typed `write preview-update --variable` command also accepts fixed siblings
with one Short Text/Binary field in one table. Its `--set` ordinals and expected
values stay caller-supplied; this is not SQL row selection. Binary values use
hexadecimal, and mixed INDEX growth or mixed Memo/OLE requests refuse before
publication. Default typed updates remain fixed-field maintenance.
