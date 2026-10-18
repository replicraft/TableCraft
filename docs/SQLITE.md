# Read-only SQLite backend

`tablecraft-sqlite` is a working synchronous `tablecraft-query::Source` adapter.
The CLI opens an existing SQLite file with `query run --sqlite PATH`; `--database`
continues selecting MDB/ACCDB, and the two options conflict. Native `--index`
selection cannot be combined with SQLite. No filename-extension inference occurs.

```sh
sqlite3 example.sqlite "CREATE TABLE Items(ID INTEGER, Label TEXT); INSERT INTO Items VALUES(1,'one'),(2,'two');"
cargo run --locked -p tablecraft-cli -- query run --sqlite example.sqlite "SELECT TOP 1 ID, Label & '!' AS Caption FROM Items ORDER BY ID DESC"
```

This prints `ID`/`Caption` followed by `2`/`"two!"`, tab separated. Use a fresh
example path when running the creation command. The adapter does not create files.

## Query and type boundary

Access SQL is parsed, bound and evaluated in TableCraft. SQLite receives only
adapter-generated metadata and quoted table/column SELECTs. There is no SQL
translation, predicate pushdown, action query, schema editing or mutation API.
Joins, aggregates, parameters, TOP and supported expressions use the existing
[query execution profile](QUERY_EXECUTION.md), including explicit text collation.

Only ordinary main-database tables are inventoried. Views, virtual tables and
their shadow tables are unavailable as sources. Generated columns make a selected
table unsupported; triggers are disabled and no extensions/providers are loaded.
Metadata binding does not scan user records. Cached column metadata is tied to
one read transaction; records stream through borrowed-row visitor calls.

| Declared SQLite affinity | TableCraft carrier |
| --- | --- |
| Integer (`INT` substring) | `Value::Int64` |
| Text (`CHAR`, `CLOB`, `TEXT`) | `Value::Text`, strict UTF-8 |
| Blob (`BLOB`) | `Value::Binary` |
| Real (`REAL`, `FLOA`, `DOUB`) | `Value::Double` |
| NUMERIC, missing type or STRICT ANY | Unsupported; no guessed decimal/date/boolean representation |

Affinity precedence follows [SQLite's documented rules](https://www.sqlite.org/datatype3.html).
Runtime storage classes must match the declared carrier after SQLite's own
read representation; mixed carriers fail without coercion. NULL remains distinct
from empty text/blob. Nullability uses explicit NOT NULL metadata, conservatively
allowing SQLite's nullable ordinary primary keys. This does not reinterpret them
as Access primary keys. Shared SchemaPolicy validates identity and width.

## Snapshot, cancellation and limits

`SqliteSource::open` uses read-only flags without URI interpretation or creation,
sets query-only/defensive mode, disables trusted schemas/views/triggers, begins a
deferred transaction and reads the main schema b-tree before returning. Thus
later scans retain the same SQLite read snapshot, including across another WAL
connection's commits. Reads may hold locks or delay WAL checkpoint cleanup until
`close`/drop. No shared-file Access protocol or native journal is involved.

Busy handling is immediate: no implicit timeout/retry. Callers provide a cumulative
ResourceBudget and shared `Arc<AtomicBool>` cancellation flag. Adapter boundaries
check cancellation/work; SQLite's VM progress hook checks every approximately
100 instructions and charges 100 Steps. Interrupted work retains its typed
cancellation/limit cause. Each scanned row charges Rows, and Rust-owned metadata,
generated SQL, rows and copied payloads charge Bytes before allocation. Repeated
scans spend the same allowance. Visitor errors stop scanning immediately.

Structural ceilings: 256 tables/columns, 255-byte identities, 1 MiB SQLite
row/string/blob length, 256 KiB generated SQL, expression depth 100 and 100,000
compiled VM operations. Attached databases and SQLite auxiliary worker threads
are disabled. These are reader limits, not SQLite format maxima. SQLite's internal
pager/cache/parse allocations and filesystem I/O bytes are not metered by the Rust
budget; the VM callback is not a hard deadline for filesystem stalls. `close`
disables the callback before rollback so exhaustion cannot prevent teardown.

## Evidence and remaining work

Original in-memory and disposable-file tests cover exact carriers, extreme i64s,
Unicode, NULL/empty values, dynamic-type rejection, malformed UTF-8, metadata-only
binding, unsupported executable objects, literal identifiers, visitor failures,
budgets, VM interruption, structural ceilings and WAL snapshot consistency.
A 256-case fixed-seed property compares bound insertions with decoded rows.
CLI tests verify Access queries, parameters, joins, buffered/redacted failures,
option conflicts and unchanged original database bytes.

This is native read-only SQLite support, not complete Access-to-SQLite fidelity.
Decimal/currency/date/boolean storage policies, writes, migrations, linked sources,
async hosts and browser storage need separate implementations and evidence.
The selected [rusqlite API](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html)
and [table metadata](https://www.sqlite.org/pragma.html#pragma_table_xinfo)
are primary references; exact bundled version/features/licenses are in
[DEPENDENCIES.md](DEPENDENCIES.md#sqlite-reader).
