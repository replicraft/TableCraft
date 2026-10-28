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
Int32, Int64 and Currency fields in either direction, including represented NULLs
and ordinary or unique constraints. Key carriers remain exact; no SQL coercion
or floating conversion supplies native keys. Existing constraint services retain
required-key and nullable-unique behavior.

Ignore-NULL indexes, foreign indexes, tuples and other key families remain
refused. Carrying a Decimal, Text, Binary, floating or other admitted field in a
new record does not authorize an index on that field. The ordinary record profile
and the physical key profile remain separate requirements.

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
