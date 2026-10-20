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
Decimal/currency/date/boolean storage policies, writes, migrations, linked sources
and browser storage need separate implementations and evidence.
The selected [rusqlite API](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html)
and [table metadata](https://www.sqlite.org/pragma.html#pragma_table_xinfo)
are primary references; exact bundled version/features/licenses are in
[DEPENDENCIES.md](DEPENDENCIES.md#sqlite-reader).

## Optional async worker

Enable `tablecraft-sqlite/async` for `AsyncReader`, `AsyncOptions` and `AsyncError`.
The working adapter uses tokio-rusqlite's worker-owned connection plus Tokio's
semaphore for admission. Defaults are empty; the CLI uses the synchronous source
and its normal dependency closure excludes Tokio/tokio-rusqlite. Hosts own their
executor and lifetime; this feature supplies no GUI or network runtime.

`AsyncReader::open` opens an existing path read-only on a dedicated worker.
`query` takes an owned TableCraft query, owned typed parameter bindings and explicit
storage/evaluation limits, cancellation and optional worker-safe collation. It
allows one admitted request per reader. Further requests receive `AsyncError::Busy`
immediately, without entering the upstream unbounded queue. A host must also bound
how many readers it opens. Each admitted request begins and pins its own read
snapshot, runs TableCraft binding/evaluation on the worker, and disables its progress
hook and rolls back before releasing admission. Completed requests do not retain
read transactions between calls. Cleanup failure makes the reader unavailable.

Dropping a pending future sets its request-specific cancellation flag. The actual
worker keeps its admission permit until it exits and cleans up; abandoning an await
does not prove work has stopped. SQLite VM steps and TableCraft evaluation check
the flag. Host collation callbacks must cooperate and bound their own work;
filesystem stalls cannot be interrupted by the VM hook. A fresh flag is required
for another request; cancellation is never silently reset. `close` drains admitted
work and reports worker close failure. No transactions can span separate requests.

Four worker tests cover Access queries/parameters, fresh snapshots and lock release,
cleanup/reuse after failure/cancellation/exhaustion, noncreating opens and dropping
an active request while a host collation is deliberately blocked. The last verifies
that new requests stay rejected until real worker cleanup. A compiled async API
example accompanies the public type. Async feature checks are separate from the
synchronous CLI profile. See the [upstream worker API](https://docs.rs/tokio-rusqlite/0.8.0/tokio_rusqlite/struct.Connection.html)
and [dependency feature consequences](DEPENDENCIES.md#optional-async-sqlite-worker).

## Future native project container

SQLite-backed `.tcraft` is now the accepted native application document direction;
[the project format](formats/TCRAFT_FORMAT.md) is separate from this ordinary
read-only database adapter. It will include versioned definitions/source/resources,
not just tables. `tablecraft-project` owns metadata/compatibility/internal migration;
this adapter will own SQLite connection/transaction behavior. No `.tcraft` format,
SQLite writer or general transaction API is implemented by this decision. Further
SQLite implementation remains behind the current ACE 16 priority.

The optional worker's `AsyncOptions::year_window` forwards a validated per-request
Gregorian DateSerial policy to shared query execution. Omission retains the named
1930–2029 profile. Reusing a worker does not retain a prior request's override;
this adds no SQLite expression delegation, writes or CLI runtime dependency.
