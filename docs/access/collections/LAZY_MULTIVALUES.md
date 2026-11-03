# Source-bound parent multivalue reads

`MultivalueReader::open` accepts an explicit stable/offline native source, catalog
scope, table, stored multivalue field, checked complete directory limits and the
caller's cumulative native budget/cancellation callback. It resolves actual stored
metadata and validates every parent key and child foreign key/identity through
EOF before returning. Unknown templates/carriers, NULL/duplicate child identities,
orphan references, ambiguous parents and the existing 100-value parent limit retain
their actual errors. Child identities remain signed Int32; value duplicates remain.

`read_parent(ordinal)` returns the complete unfiltered values of that reader's
local parent in original flat order. NULL parent keys and assigned empty lists
remain distinct through the returned key. A NULL child value still contributes
a member. Fields use the real typed record decoder without SQL coercion, lookup
translation, Text comparison, provider execution or formula evaluation.

Opening does not decode Value payloads. Each parent operation re-resolves the
actual owner/type/flat metadata and revalidates all parent keys and the full child
ROW/FK/identity census; only the selected parent's Value position is decoded.
The complete key directory stays separate from complete child payload snapshots
and contains no selected-parent or filtered-list cache. This may rescan every key
page per operation and remains subject to complete directory and resource ceilings.
It does not claim O(1) expansion, constant memory, a native snapshot/lock or coherent
reads while another application changes the source.

No parent list is returned after a later key/EOF failure. Invalid parent ordinals
leave a ready reader unchanged. Other operation failures are terminal and later
calls return `CollectionReaderFailed`; reopen explicitly to capture a new view.
The original budget never refills. Owned parent results can outlive the reader,
but their logical ROW/key is meaningful only with the issuing source and schema,
not as a portable row identifier, write witness or foreign snapshot certificate.

`NativeBrowser::multivalue_reader` is the working adapter consumer and borrows its
actual source and original controls. Existing default browsing, native query
collection/presence/indexed-member paths and their complete caches are unchanged.
The [attachment reader](LAZY_ATTACHMENTS.md) shares the actual complete-directory
and selected-parent traversal while retaining attachment-specific carrier
qualification. History and query per-parent contracts still need separate typed
consumers. No CLI grammar or UI is added by these APIs.
Native Access comparisons remain distinct from independent engine/adapter tests.

Nine focused functions and retained regressions pass: 99 engine functions,
61 adapter functions and 35 CLI browsing functions. Strict all-target checks,
minimal builds, warning-free Rust documentation and dependency guards pass after
removing one unused private argument. The
[acceptance record](../../testing/results/lazy-multivalue-reader-2026-11-02.json)
preserves the first lint failure and narrow correction; it supplies no native
Access qualification.
