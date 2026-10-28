# Multiple physical indexes during INSERT

This continuation extends the separate ACE 16 INSERT engineering preview.
It does not enable ordinary saves or qualify native Access transactions, data
macros or concurrent writing. The
[ordinary-row contract](ACE16_ORDINARY_INSERTS.md) still owns record construction,
and the [recursive INSERT contract](ACE16_INSERT_RECURSIVE.md) owns the established
tree-normalization policy.

## Physical ownership

The preview prepares every declared physical index in ascending physical-number
order. Logical aliases must agree on the same physical root, usage map, key
components, direction and flags. They share one plan and historical counter
update. A missing, unsupported or inconsistent owner refuses the operation;
there is no partial index-maintenance mode.

A required unique ascending Int32 primary remains mandatory. Its
physical slot need not be zero. Secondary single-key indexes admit Byte, Int16,
Int32, Int64, Currency, Boolean, Single, Double, legacy DateTime and GUID fields in
either direction, including represented NULLs where the carrier supports them
and ordinary or unique constraints. Key carriers remain exact; no SQL coercion
or floating conversion supplies native keys. Existing constraint services retain
required-key and nullable-unique behavior.

Secondary Ignore-NULL indexes admit the qualified fixed-key profiles when the
index is not Required. Primary Ignore-NULL and Required+Ignore-NULL combinations
remain refused, as do foreign indexes and other unqualified key families.
Qualified [fixed GUID/numeric tuples](ACE16_FIXED_TUPLE_INSERTS.md) now extend
this service to two-to-ten components with independent directions. Storing a
Decimal, Text, Binary or other unqualified key carrier does not authorize an
index on that field. The ordinary record and physical key profiles remain
separate requirements.

Distinct physical trees must own disjoint original INDEX pages, including their
descendants. Logical aliases within one physical owner do not violate this rule.
The planner retains a paid page inventory; the final audit independently reads
the original trees rather than treating that inventory as authority.

## Shared allocation and final proof

DATA placement happens first. Every physical tree then continues through the
same typed EOF cursor and map-byte ledger. Fresh INDEX pages and index bitmap
metadata retain their physical owner and usage reference. Earlier reservations
must belong to earlier physical owners; an INDEX reservation cannot become an
ordinary DATA page. Original map storage is protected by the complete catalog
ownership census, including indexes unrelated to the primary field.

The final composer merges exact-before patches and checks disjoint rewritten
ranges. It writes the shared map-byte ledger once. The independent audit derives
records from immutable requests, proves their logical ROW placements, rebuilds
each physical key stream and replays every tree/map/counter effect into one full
expected image. Planner key sets and allocation facts are comparison inputs,
not substitutes for that derivation. Ordinary read and constraint checks still
run on the completed candidate.

## Historical counters

The table row counter increases by the number of inserted rows, once. Each
physical index's historical distinct-key counter instead increases by newly
introduced key families absent from the current source tree. Repeated new keys
count once, an already present key counts zero, and a represented NULL is one
family. ROW references distinguish entries but do not distinguish key families.

For example, source secondary keys `10, 10, NULL` followed by inserted keys
`10, 20, 20, NULL` add four index entries and one distinct key. A stored historical
value of 29 becomes 30; it is never reset to the current live distinct count.
Primary uniqueness makes its delta equal the inserted-row count. Zero deltas
emit no counter patch, and overflow refuses the complete operation.

The [historical-counter reference](../access/indexes/NATIVE_INDEXES.md#explicit-historical-counter-inspection)
records the pinned Jackcess source for this distinction. Native Access mutation
timing and save/reopen equivalence remain separate qualification work.

## Verification scope

Independent synthetic fixtures construct real three-field records, complete
index streams, separate physical maps and logical aliases before invoking the
preview. They predict full images, including a case where DATA grows and both
index roots split through shared global-map bytes. Additional width-aware
fixtures exercise the secondary key families rather than relabeling Int32
payloads. The complete ACE unit batch passes 426 functions with one existing
ignored engineering generator; strict Clippy across all ACE targets/features,
minimal-feature compilation, formatting and dependency/documentation guards
also pass.

Two earlier public primary-NULL tests exposed a changed error classification in
the generic nullable planner. The public explicit-Int32 guard was restored;
secondary required-NULL validation retains its own typed constraint error.
Another new negative test reached the existing Ignore-NULL constraint-coverage
gate before the physical planner; the test now records that exact earlier gate.
Old private single-owner refusal tests were updated for the new per-owner
constructor, retaining missing-owner and public complete-maintenance controls.

The separate [independent-writer reference](ace16_insert_multiple_indexes/README.md)
admits four Jackcess-generated originals and verifies four reference transitions.
An initial authored source gate required a native leaf to use its best possible
prefix even though its uncompressed entries fit. The failed seven-call phase is
preserved; a frozen continuation validates the actual framing and executes only
the eighteen remaining calls. No original bytes were repaired or calls repeated.
These references do not substitute for TableCraft candidate checks or native
Microsoft Access qualification.

The [isolated producer build](results/ace16-insert-multiple-index-example-build-2026-10-28.json)
uses committed source after all four independent source/request expected images
were frozen. All four first-attempt candidates match those complete images before
any independent reader is launched. Eight subsequent fresh, read-only Jackcess
calls pass all four typed/raw ROW, index, counter, map and protected-byte proofs;
all 806 old ROWs remain, and 112 explicit new rows are added across the cases.
Original and reference files remain unchanged. The
[candidate receipt](results/ace16-insert-multiple-index-native-2026-10-28.json)
and maintained saved replay retain exact inputs, outputs and execution phases.

Both pressure cases grow one ordinary DATA page followed by four INDEX pages:
physical zero reserves its right then left leaf, followed by physical one in the
same order. The ordinary cases fit existing DATA/index pages. New index padding,
retained-root residue and every unowned original byte are independent engineering
expectations, not copied from a public writer's after-image.

Two pure model-preparation errors were corrected before the expected-image freeze
and producer build: import setup and bytearray transport into the saved raw-tree
checker. Their initial attempts are preserved; the already derived ordinary image
remains byte-identical. No candidate output or public-writer after-image repaired
the expected model. Native Access save/reopen and production write admission
remain unqualified.


## Secondary indexes omitting NULL keys

The INSERT preview now omits a NULL key from each qualified secondary
Ignore-NULL index while still storing the complete row and primary entry.
Non-NULL keys retain direction, uniqueness and logical aliases. This admission
is specific to INSERT; ordinary UPDATE and slot-update coverage is unchanged.

Before construction, a completed structural check must certify the original
index. The final image audit independently locates the actual primary physical
group, proves its complete unique logical ROW universe, decodes the selected
secondary field from every original root and compares the exact eligible
key/ROW stream. Forwarded payloads retain their original logical roots. A matching
entry count alone cannot establish membership.

NULL requests still undergo ROW geometry and duplicate-reference validation.
Only represented non-NULL families affect that physical index's historical
counter. If all inserted keys are NULL, its original tree, map and history
remain byte-identical, including an existing NODE tree. Other owners and DATA
may still change their shared global allocation bytes.

The current ACE unit batch passes 443 functions with one existing ignored
engineering generator. New complete-image controls cover both ACE 16 layouts,
all admitted numeric widths, both directions, empty and NODE trees, unique keys,
forwarded roots and shared DATA/index growth. Negative controls reject malformed
membership, forged no-op changes, counter overflow, unsupported flags, exhausted
budgets and cancellation. Strict ACE Clippy across all targets/features and
minimal-feature compilation pass.

Ten first-attempt candidates from the committed isolated producer match ten
independently frozen source/request images before any independent reader runs.
Twenty fresh read-only Jackcess getter/map calls then pass typed/raw rows, exact
physical membership, counters, maps and protected-byte proofs. Across these
cases, 1,818 original logical ROWs remain and 132 rows are added. Cases include
mixed nullable keys, an empty secondary with either all-NULL or first non-NULL
requests, an unchanged secondary NODE, and shared DATA/two-owner root growth.
Both ACE 16 layouts are represented. Authentic references cover ascending
nonunique Int32 secondary keys; unique, descending and wider numeric coverage
comes from the separate synthetic whole-image controls.

The independent reference controller first stopped after two successful child
calls because it rejected a Jackcess warning emitted for omitted NULL keys.
The pinned library source explains that unchanged-entry commit path. The
failed controller and raw warning remain preserved. A frozen continuation
accepts only that exact warning context and its recipe-derived occurrence count;
it executes the remaining 53 calls once, with all eight source gates before
any of ten reference appends. Unrecognized warnings still fail. No source or
candidate was repaired, and no completed call was repeated. These checks do
not establish native Access writes.


The [maintained saved proof](ace16_insert_ignored_indexes/README.md) and
[candidate receipt](results/ace16-insert-ignored-index-native-2026-10-28.json)
retain those phases separately. Default replay starts no child process and
reports missing private evidence explicitly. The
[isolated build receipt](results/ace16-insert-ignored-index-example-build-2026-10-28.json)
binds the committed producer to the previously frozen expected images.


## Boolean, IEEE and legacy DATE secondary keys

The same preview now admits single Boolean, Single, Double and legacy DateTime
secondary indexes. Physical key construction uses the existing native codecs;
the final audit owns an independent bit-order implementation and validates
inverse DATE serials. No new runtime, dependency or public preview API is needed.
The mandatory primary remains required unique ascending Int32.

Boolean false is a real non-NULL value even when its stored mask bit is clear.
Its key payload is distinct from true; an explicit Boolean NULL is refused.
A non-Required Ignore-NULL Boolean index therefore retains complete membership.
Single and Double preserve signed zeros, subnormals, infinities and individual
NaN payloads as physical bit families. Legacy DATE preserves validated serial
bits, including negative civil aliases and sub-millisecond values. These are
native storage-key rules, not SQL numeric equality or calendar conversion.

Eight new end-to-end functions construct genuine width/mask-aware original and
expected whole images before invoking the preview. Both ACE 16 layouts and both
directions cover logical aliases, unique duplicate refusal, represented/omitted
NULLs, historical deltas, malformed carriers, unsupported Required+Ignore-NULL
flags, all resource pools and terminal cancellation. Pressure cases split Single
401-to-403-entry and Double/DATE 278-to-279-entry secondary leaves through the
shared allocator while preserving the primary. Four new independent audit
functions additionally reject corrupt Boolean payloads, invalid DATE wires and
impossible narrow atoms. All 158 INSERT regressions, including these twelve
functions, pass; strict all-target/all-feature ACE Clippy also passes.

The separate [scalar reference/candidate corpus](ace16_insert_scalar_indexes/README.md)
now qualifies sixteen originals: both ACE 16 layouts and both directions for
Boolean, Single, Double and legacy DATE. All 97 source/reference calls pass,
with every original qualified before any public-writer append. Actual stored
bits, getter projections and the independent encoder agree; authentic float
cases use finite values and signed zeros. NaN/infinity coverage remains the
separate synthetic evidence above.

Sixteen source/request-derived complete images were frozen before an isolated
committed producer build. All sixteen first-attempt candidates match those images
before thirty-two fresh read-only getter/map calls pass. Across the cases, all
3,840 original ROWs remain, 24 rows are added and 7,728 physical key/ROW pairs
are checked. Boolean requests add no new secondary family; Single adds two,
Double and DATE one each. Twelve pressure cases append two INDEX pages each,
with no DATA growth; fitting primary changes retain their existing pages.

The initial pure static API check exposed the independent encoder's package-private
Java visibility before any fixture call. The corrected helper uses the existing
narrow reflection probe pattern; failed static inputs remain preserved. No
source recipe or expected image was repaired, and actual calls were not repeated.
The [candidate receipt](results/ace16-insert-scalar-index-native-2026-10-28.json)
and [producer receipt](results/ace16-insert-scalar-index-example-build-2026-10-28.json)
keep build provenance and separate execution phases. Native Access save/reopen
and deeper scalar-specific reference observations remain unqualified.


## Broader key preparation

The recursive tree kernel now uses generic retained keys and borrowed encoding.
Its current numeric caller and exact allocation/image policy are unchanged.
A non-Copy retained-key test brought that checkpoint to 159 passing INSERT
functions. The later [fixed GUID/tuple continuation](ACE16_FIXED_TUPLE_INSERTS.md)
now implements complete-row NULL rules, original membership and independent
wire proof, with 171 passing INSERT functions. Variable-key and Decimal index
continuations remain separate work.
