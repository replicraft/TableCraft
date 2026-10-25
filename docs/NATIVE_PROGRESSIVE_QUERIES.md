# Native progressive scalar query sources

`NativeSource` implements the explicit
[progressive query capability](PROGRESSIVE_QUERIES.md) through a fresh native
`TableCursor`. It supports ordinary local scalar projections, including qualified
scalar views of mixed schemas. It never substitutes a complete snapshot or fills
a table/projection/selected-row cache with a progressive prefix.

## Source and cursor contract

Preparation captures scalar/unavailable columns and Text policies without
reading user rows. Before callbacks, the adapter validates projection indices,
opens the requested native projection and checks both complete bound metadata
and exact selected-column alignment. Changed metadata fails explicitly.

Selected required indexes, linked tables and configured multivalue/collection/
decoded-attachment/stored-calculation modes are refused before visiting rows.
Automatic index hints are not used for this unconstrained sequential capability;
there is no fallback from a required selected index. Other normal source methods
retain their existing complete-cache behavior.

Each call starts a new cursor, even if an earlier complete snapshot exists.
Batches use the source's configured batch size, validate continuation offsets,
and verify declared count and cursor position at EOF. Late native errors,
count mismatches and cancellation remain failures after any delivered prefix.
The snapshot retention ceiling does not cap progressive row count; cumulative
native row/read/allocation/work allowances remain authoritative.

The adapter releases its native-budget borrow before invoking a visitor, so query
and sink callbacks do not inherit that mutable budget borrow. The cursor still
holds exclusive source access. Reentrant native reads requiring that reader fail
through typed borrow checks; no retry or implicit second reader is opened.
Consumers and expression providers must obey the source-exclusive callback rule.
This is stable/offline reading, not coherent native shared-file coordination.

## Verification

Eleven focused adapter tests pass alongside scoped strict Clippy. They cover
priority layout selectors and batch sizes, scalar pruning, count continuation,
late corruption, limits/cancellation, source reentry, metadata changes, existing
cache isolation and unsupported mode refusals. Successful and failed streams
leave ordinary later reads complete. A deliberately small snapshot-retention cap
does not truncate an independently budgeted progressive scan.

These are independent synthetic engine/adapter checks. Original-file CLI replay
and Microsoft Access qualification are separate evidence; the [explicit CLI host](STREAMING_QUERIES.md) now supplies publication, while
the broader read/query roadmap remains unfinished.


## Successive UNION ALL arms

The query runtime can now stream flat UNION ALL chains of admitted SELECT arms.
Each arm uses a fresh cursor on the same NativeSource; its exclusive borrow is
released after checked EOF before the next cursor starts. Native byte/row/work
allowances remain cumulative. Neither a successful arm nor a failed suffix becomes
a complete snapshot cache. Six new controls cover repeated/different tables,
EOF/read/evaluation failures, TOP tails, cancellation, sink failures and retry
under four synthetic priority profiles. All seventeen native progressive tests
and scoped strict Clippy pass; this does not qualify coherent live-file reads.

## Global count consumers

The query service can now consume this cursor for [global Count(*) and Count(expr)](PROGRESSIVE_QUERIES.md#global-count).
Seven new adapter controls pass alongside the seventeen existing progressive-source
tests and scoped strict Clippy. Four profiles exceed a snapshot retention ceiling
of one while retaining cumulative native budgets and checked EOF. Repeated ordinary
buffered counts still refuse that ceiling, demonstrating that progressive counts
never fill a complete snapshot cache.

Unfiltered Count(*) requests zero scalar columns. It counts framed records without
validating unselected value payloads; a predicate requiring an invalid field still
fails. This is a selected-read count, not a whole-file consistency check. Late page,
I/O, count, predicate, cancellation and resource failures publish no aggregate row,
and prevent later UNION arms from opening their data. Final count publication runs
after the cursor and native-budget borrows are released.
