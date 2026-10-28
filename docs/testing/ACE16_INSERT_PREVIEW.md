# ACE 16 ordinary-row INSERT engineering preview

`NativeImage::preview_ace16_insert_rows`, behind `ace16-write-preview`, constructs
a separate `Ace16WritePreview` from complete explicit `RowInsert` values. This
is an engineering candidate for disposable offline copies. It is not a normal
save, SQL INSERT executor, journalable edit plan or Microsoft Access qualification.
The existing [preview safety boundary](ACE16_WRITE_PREVIEWS.md) still applies:
required data-macro semantics remain unqualified.

## Admitted physical profile

The selected ordinary ACE 16 table now admits the
[ordinary scalar-row profile](ACE16_ORDINARY_INSERTS.md): supported fixed scalars,
Boolean masks and multiple Short Text/Binary fields, with complete current
physical geometry. Each request supplies every exact carrier in native column
order. The actual declared Int32 primary selects its key column, not the first
Int32 or a name. NULL and present empty values remain distinct. Text bounds count
UTF-16 units; Binary follows its positive native declared width through the
existing 510-byte ceiling. The historical original-file fixtures below use the
narrower two-field Int32/Binary255 schema; wider-schema qualification is unrun.

DefaultValue presence refuses, even when every value is explicit or the batch
is empty. AutoNumber, calculated and complex fields refuse. Existing active
payload, catalog-child, unsupported-property and constraint blockers remain.
This admission does not establish absence of unknown data macros. Actual empty
Text additionally requires an explicit AllowZeroLength declaration; stored false
still rejects the value through normal constraints. Missing declarations are not
inferred as native defaults.

Exactly one physical unique, required, ascending Int32 primary index is admitted.
Qualified logical aliases share that physical identity. An original root leaf
uses the established [two-fitting-halves path](ACE16_INSERT_ROOT_SPLITS.md)
first. Larger/repeated leaf overflow and arbitrary qualified NODE sources use the
[recursive continuation](ACE16_INSERT_RECURSIVE.md), retaining the existing depth
ceiling of 16 and requiring source/new heights below 16. Original tail-present
and all-bounded forms retain their routing policy; internal overflows propagate
through independently normalized child ranges. One root growth event is allowed
per operation, and the resulting top must fit. Other physical indexes,
ignored-NULL profiles and additional key fields remain refused. Original records
and their logical ROW references remain intact.

## Placement, index membership and counters

Every new record has a complete ordinary row layout with explicit NULL bits,
variable offsets and stored values. New logical roots are visible and have no
deleted/hidden/forwarding flags. Deleted slots are retained rather than reused;
existing rows are not compacted or rewritten to make room.

The allocator tries the highest qualified free-space hint. A failed fit clears
that hint, preserves lower untried hints and proceeds to represented EOF space.
New DATA pages and any required hint bitmap metadata have distinct reservations.
The shared [allocation rules](ACE16_ALLOCATION_TRANSITIONS.md) remain bounded by
the original map authority and represented global free-page coverage.

The primary receives the exact new key/ROW pairs, preserving its root identity
and unrelated residue bytes. Duplicate original or batch primary values retain
typed uniqueness failures. Root-leaf growth uses a separate cardinality-aware
continuation; it does not invoke UPDATE key preparation or its audit. Recursive
insertion routes each assigned subtree range against the first original child
maxima and clamps its remainder to the last child. Fitting pages retain identity;
overflows normalize to fresh LEFT symbols and retained RIGHT pages. Existing
source form is preserved, with all-bounded last maxima updated as needed. A grown
NODE root becomes all-bounded; a grown LEAF root retains the established tail
form. Fresh pages use zero seeds/padding, while the grown root keeps its original
residue under a zero seed.

The declared table count increases by the number of inserted rows. The actual
[historical physical index counter](../access/indexes/NATIVE_INDEXES.md#explicit-historical-counter-inspection)
increases by the same delta once, including when logical aliases exist. The
historical counter is not reconstructed from the current live row count.

## Authority and publication

Builder plans are provisional. A separate final audit derives records, placement,
map transitions, primary membership and counter deltas from the original image
and immutable requests. Complete-image comparison protects bytes outside those
effects. Fresh catalog, table/index and constraint checks follow before returning
the candidate. Cancellation, allocation and work limits cover construction and
proof; failure never returns an incomplete candidate.

An empty qualified batch produces a byte-identical candidate with zero changes.
It still checks the table's admission profile and consumes inspection resources.
Preview construction consumes an in-memory capture and does not mutate the
source file.

The explicit `preview_ace16_insert` example uses clap and the
[bounded quoted TSV input protocol](ACE16_INSERT_INPUT.md). It validates complete
request input before opening the database and uses the existing fresh-output
publisher. Ordinary CLI writes remain unchanged.

## Verification and independent evidence

[Scoped verification](results/ace16-insert-native-2026-10-27.json) covers 339
distinct passing runtime functions, three compile-fail checks, strict lint,
minimal-feature compilation and rustdoc. The end-to-end hint-promotion control
constructs DATA page 600 and separate bitmap page 601, retains the lower untried
hint and checks the complete expected image before invoking the API.

The [committed producer](results/ace16-insert-example-build-2026-10-27.json)
passed four first-attempt original-file comparisons. Each complete image matched
its independently frozen source/request-derived expectation before fresh getters
ran. Eight read-only Jackcess getter/map calls then agreed, covering 4,528 typed
rows and 37,056 logical index entries across the four snapshots. Source files,
requests and expected images remained unchanged.

| Fixture | New visible root | Table/historical counts | Image growth |
| --- | --- | --- | --- |
| V2016 fitted | Page 200, slot 10 | 10 to 11 | None |
| V2016 EOF | Page 255, slot 0 | 22 to 23 | 4,096 bytes |
| V2019 fitted | Page 217, slot 10 | 10 to 11 | None |
| V2019 EOF | Page 272, slot 0 | 22 to 23 | 4,096 bytes |

The [saved-evidence verifier](ace16_insert_candidates/README.md) rederives the
expectations and checks archived reports without executing a database or JVM.
Its optional Git mode checks the producer's committed source closure. It requires
the retained private engineering artifacts; absence is reported as unavailable
and is not a passing result or a CI prerequisite. These independent comparisons
do not establish Microsoft Access conformance.

## Remaining work

This profile does not finish general INSERT. Other scalar/complex fields,
defaults and AutoNumber, additional index families, nesting an oversized new top,
broader allocation
and reclamation, query INSERT planning, application events and normal native save
qualification remain separate work. Native Microsoft Access comparison evidence
is still deferred by the user.
