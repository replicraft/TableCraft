# Read-only queries over native scalar tables

The working `crates/storage/tablecraft-access` package adapts ACE snapshots to the
existing `tablecraft-query::Source` interface. `NativeSource` accepts a caller-supplied
Read + Seek source, catalog namespace, cumulative ResourceBudget and cancellation
callback. It inventories metadata inertly, loads only explicitly referenced table
snapshots and caches each completed snapshot for repeated scans/joins/subqueries.
No application host, native provider, GUI, network runtime or new registry dependency
is required. ACE and query do not depend on the adapter; query still performs no
native I/O. Backend-neutral database services remain unimplemented.

The CLI opens only an explicit `--database PATH`, read-only. Use a stable/offline
copy; native shared-file locking, transactions and concurrent snapshot acquisition
are unimplemented. Without that option, `query run` retains its empty catalog and
inline-source behavior. `query parse` and `query eval` remain independent of files.

```sh
cargo run --locked -p tablecraft-cli -- query run --database path/to/copy.accdb "SELECT TOP 5 ORDR_NBR, NBR_VALUE, COLOR_ID FROM ROULETTE_TB ORDER BY ORDR_NBR"
```

On a byte-identical copy of the registered example this prints:

```text
ORDR_NBR  NBR_VALUE  COLOR_ID
1         "2"        0
2         "0"        65280
3         "28"       0
4         "9"        255
5         "26"       0
```

Actual output uses the shared escaped tab-separated formatter. Filtering, sorting,
TOP, parameters, joins, aggregates, subqueries and explicit `--collation ordinal`
use the existing [bounded retrieval profile](QUERY_EXECUTION.md). Text collation
is still explicit and is not Access locale collation. A table's stored row order
is not a SQL ordering guarantee; use ORDER BY for deterministic selection.

## Explicit supported boundary

[Native scalar tables](NATIVE_TABLES.md) defines the accepted unencoded Jet 4 /
ACE-code-2 profile and column carriers. Every column in a referenced table must
be supported, even if a projection selects fewer columns. Unqualified Decimal layouts, calculated/complex/attachment and other unsupported fields
cause failure. Empty tables with unsupported fields also fail. Linked tables,
saved-query sources, external providers and active database content are never
opened/executed as sources. No action/DDL or native writes are implemented.

Snapshots materialize all live rows before binding completes: no automatic index selection,
filter or TOP pushdown exists. The CLI supplies 16 MiB cumulative native bytes,
16,384 cumulative catalog/table rows and 1,000,000 steps across the adapter;
each reader also enforces structural ceilings. Repeated catalog/schema reads for
new tables consume that shared budget. Retrieval separately uses its documented
16 MiB / 100,000-row / 1,000,000-step ceilings. Output is buffered under the shared
1 MiB logical ceiling. Failures during load, bind, execute or formatting exit 2
with no partial result/header and diagnostics that omit paths/names/values.
I/O/broken-pipe behavior remains unchanged.

Cached tables do not repeat native reads. Cancellation applies to cached scans
as well as decoding; visitor rejection stops immediately. Host I/O timeouts and
stable-source ownership remain the caller's responsibility. This adapter is
synchronous and does not expose concurrent mutation or connection pooling.

## Evidence and limitations

Focused adapter tests execute a real query over a candidate copy, count reads to
verify caching, reject unsupported/missing sources, check empty relations and
cancellation/visitor rejection. CLI tests cover the documented TOP result,
filter/parameter/order behavior, Count(*), self-join, empty table, explicit text
collation, action/type/name/open failures, no partial stdout and unchanged bytes.

The underlying 1,488-row Jackcess comparison is recorded in NATIVE_TABLES.md.
It verifies independent reading, not Microsoft Access query/expression behavior.
No Microsoft Access oracle, application activation, native edit/save/reopen,
shared-file operation or Windows/Linux execution has been qualified by these checks.

Legacy Date/Time projection, temporal comparison/order and the bounded ISO literal/
parameter/output profile are implemented; see [native date contracts](NATIVE_TABLES.md#legacy-datetime).

Bounded Long Text snapshots now include inline and overflow/chained native values;
see [Long Text limits](NATIVE_TABLES.md#bounded-long-text). Markup remains inert data.

GUID projection/equality/literals/parameters and binary hexadecimal output are
implemented; [GUID limits](NATIVE_TABLES.md#guid-and-binary-output) distinguish
equality from unqualified native index ordering.

Qualified exact native Decimal fields support projection/comparison/ordering under
the existing scalar rules; [Decimal limits](NATIVE_TABLES.md#exact-native-decimal)
record the accepted storage/metadata profile and independent comparison evidence.

Bounded native OLE values now project as opaque hexadecimal Binary. The
[OLE profile](NATIVE_TABLES.md#opaque-long-binary--ole) records limits, inertness
and comparison evidence. MSysObjects is readable through the same scalar source.

Existing dense appended-column histories can now be read without altering the
file; [history limits](NATIVE_TABLES.md#dense-appended-column-histories) distinguish
absent fields from evaluating current defaults and retain rejection of gaps.

## Explicit native index access

Use `--index TABLE INDEX` with --database, or NativeSource::with_index in library
hosts, to load that table through a qualified single-Int32 native index. Other
tables retain their ordinary access path. The engine validates tree/row references
and key agreement, caches the completed snapshot, and never falls back on an
ineligible/corrupt index. Eligibility is checked when the selected table is loaded.
See [NATIVE_INDEXES.md](NATIVE_INDEXES.md#bounded-indexed-reads) for the exact profile,
limits and evidence. Retrieval clauses still execute after the full snapshot is
loaded; no optimizer or filter/TOP pushdown is claimed.
