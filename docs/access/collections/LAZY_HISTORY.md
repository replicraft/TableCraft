# Source-bound per-parent stored Memo history

`VersionHistoryReader::open` accepts a stable/offline source, catalog scope, table,
visible Memo name, complete-directory limits and the original cumulative native
budget/callback. It resolves that actual visible declaration to its hidden history
key using existing template/member/native-number rules. Generated helper names are
not assumed. The actual existing visible Memo hyperlink/compression policy and
separate hidden history field policies remain unchanged.

The current visible Memo and stored history are independent. Opening and selected
history reads do not decode its current value; NULL or a different current value
never replaces historical rows. No latest-value selection, chronological/identity
sort, deduplication, calendar/timezone conversion or callback runs. A result's
`stored_versions()` borrows the same StoredVersion decoder as the full reader and
preserves original flat-root order, signed identities, exact Text/NULL/empty values
and accepted legacy DATE bits. Owned results survive reader/source drop.

The reader exposes complete metadata, the actual `memo_column()`, complete parent
membership and `read_parent(local_ordinal)`. Opening reads key-containing DATA pages
without decoding historical payload fields. Every parent operation revalidates the
actual owner/template/flat definitions and all parent keys, then replays every child
logical ROW/FK/identity through checked EOF. The actual selector also rechecks all
fresh complex templates for a unique visible-Memo association before allocating
result rows; an unrelated template cannot silently create a second binding. Only
the selected parent's complete
flat row payloads are decoded. A NULL historical Text or Modified field still
counts as a member; a NULL hidden key differs from an assigned empty history.
Unsupported extra flat metadata cannot hide behind a selected view.

A malformed unselected historical external LVAL is not fetched by key-only census;
requesting its parent exposes the actual storage error. Invalid record framing,
metadata, orphan/duplicate identities or key membership remain complete-directory
errors. The directory contains no complete payload snapshot, and a selected parent
result cannot populate existing complete query/presence caches. Default full
history, mixed collection browsing and query services remain unchanged.

NativeBrowser lends its actual source and original controls until the reader drops.
Each operation may rescan every key page; this is not O(1), constant-memory expansion,
a live-file snapshot or native locking. Invalid ordinals leave a ready reader
unchanged. Other failures return no partial result and make later calls return
CollectionReaderFailed. Reopen explicitly after drop to capture a new directory;
the original budget never refills. Logical ROW/key identity stays local to the
issuing source/schema and is not a durable bookmark or mutation witness.

The shared history selector/qualifier now meters its actual field/name scans and
checks cancellation in existing full-reader/discovery callers too. Public operation
costs must be measured under those actual passes; unchanged old total costs are not
claimed. This API adds no CLI/query grammar or native Access compatibility claim.
Twelve new functions and retained regressions pass: 83 engine, 63 adapter and
48 CLI functions, followed by strict/minimal/Rust documentation and boundary checks.
The [acceptance record](../../../tests/results/reading/lazy-history-reader-2026-11-02.json)
keeps the static fresh-association finding and first unused-import lint correction.
These results do not qualify larger directories or native Access behavior.
