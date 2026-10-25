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
and Microsoft Access qualification are separate evidence; this API alone does
not enable progressive CLI publication or complete the read/query roadmap.
