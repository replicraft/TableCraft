# ACE 16 ordinary-row INSERT engineering preview

`NativeImage::preview_ace16_insert_rows`, behind `ace16-write-preview`, constructs
a separate `Ace16WritePreview` from complete explicit `RowInsert` values. This
is an engineering candidate for disposable offline copies. It is not a normal
save, SQL INSERT executor, journalable edit plan or Microsoft Access qualification.
The existing [preview safety boundary](ACE16_WRITE_PREVIEWS.md) still applies:
required data-macro semantics remain unqualified.

## Admitted physical profile

The selected ordinary ACE 16 table has two current fields: one fixed Int32 and
one variable Short Binary, in either declared order. Names do not select their
roles. Each request supplies both exact carriers in native column order; no
coercion, omitted values, defaults or generated identities are supplied. Binary
NULL, present empty Binary and nonempty Binary remain distinct. Declared widths
apply, with the native Short Binary ceiling of 255 bytes.

DefaultValue presence refuses, even when every value is explicit or the batch
is empty. AutoNumber, calculated and complex fields refuse. Existing active
payload, catalog-child, unsupported-property and constraint blockers remain.
This admission does not establish absence of unknown data macros.

Exactly one physical unique, required, ascending Int32 primary index is admitted.
Qualified logical aliases share that physical identity. An original root leaf
may grow into a [retained root and two fresh leaves](ACE16_INSERT_ROOT_SPLITS.md).
An existing height-one source with exactly two leaves also admits
[fitted insertions under its original bound](ACE16_INSERT_HEIGHT_ONE.md).
[One overflowing child](ACE16_INSERT_CHILD_SPLITS.md) of that exact two-leaf
source can instead split into a fresh left and retained right under the original
root. Other physical indexes, ignored-NULL profiles, additional key fields,
broader source trees, multiple overflowing children and splits whose halves
cannot fit refuse. Original records and their logical ROW references remain intact.

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
continuation; it does not invoke UPDATE key preparation or its audit. Fitted
height-one insertions preserve the original inclusive separator and both child
identities; only affected fitting leaves change. A separate one-child overflow
continuation derives new cardinality-aware ranges and one new left leaf. Larger
source trees, multiple offenders and oversized halves remain explicit failures.

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
defaults and AutoNumber, additional index families and deeper splits, broader allocation
and reclamation, query INSERT planning, application events and normal native save
qualification remain separate work. Native Microsoft Access comparison evidence
is still deferred by the user.
