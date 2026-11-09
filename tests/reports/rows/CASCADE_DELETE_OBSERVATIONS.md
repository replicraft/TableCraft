# Original acyclic cascade deletion observations

The [compact receipt](results/ace16-cascading-delete-originals-2026-10-26.json)
separates two experiments over fresh repository-authored V2016/V2019 originals.
Public Jackcess actions produced **24 acyclic closure agreements, two successful
self-tree calls with dangling children, and six stack overflows**. The separate
engineering candidate replay produced **24 complete matching candidates and eight
explicit self/cycle refusals**. This is not 32-way public action parity or native
Microsoft Access SQL/save qualification.

## Originals and reference actions

[CascadeDeleteFixture.java](CascadeDeleteFixture.java) uses the reviewed public
Jackcess 5.0.4 builders; it copies no upstream implementation. Each original has
27 tables, 121 rows, 44 root-leaf physical indexes and 18 enforced Cascade
relationships with 36 reciprocal foreign endpoints. The corpus includes a chain,
a diamond, overlapping/reversed root seeds, and exact Byte, Int16, Int32, Int64,
Currency, GUID and Decimal pairs. Parent lookup keys are genuine required primary
keys with non-NULL components. Nullable all-NULL child keys and an unrelated
Unicode Memo/OLE sentinel remain. No properties, expressions, AutoNumber, stored
link activation or header/relationship flag patches are used.

The generator refuses existing outputs. Its `oracle` mode and the separate
[CascadeDeleteIndexInspection.java](CascadeDeleteIndexInspection.java) open
read-only, disable expressions and reject linked opens. Index inspection includes
all logical aliases and empty declarations. The compact receipt records actual
generation commands and hashes; originals and expanded getter transcripts stay
ignored. Run the helpers from the repository root with the reviewed JAR:

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar -d .cache/new-cascade-classes docs/testing/CascadeDeleteFixture.java docs/testing/CascadeDeleteIndexInspection.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/new-cascade-classes CascadeDeleteFixture generate .cache/new-cascade.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/new-cascade-classes CascadeDeleteFixture oracle .cache/new-cascade.accdb
```

Reference deletion selects label IDs, captures their original public row handles,
and calls `deleteRow` on fresh copies with FK enforcement enabled. All 32 action
JVMs used `-Xss1m` and a ten-second timeout; none timed out or received a larger-stack
retry. All 24 cross-table acyclic actions matched independently frozen original
associations, exact typed rows/locations and complete index orders. NULL child
sentinels survived. Reversing the two diamond roots produced identical files in
both profiles.

Both self-tree calls returned success but left child ID 3 pointing to deleted
parent ID 1. Self-loop, self-cycle and cross-table-cycle actions raised
`StackOverflowError` in both profiles; all six partial copies remained byte-identical
to their originals and reopened. These contrary observations are preserved, not
reclassified as successful closure. Self and cyclic engine profiles remain refused.

## Frozen candidate comparison

[compare_cascading_deletes.py](compare_cascading_deletes.py) and its independent
[cascade_delete_model.py](cascade_delete_model.py) are byte-identical promotions
of the executed private sources. Run from the repository root; the receipt retains
the historical executed paths and hashes. Original associations were frozen before
public actions, without using production coercion, index encoding or action output
as an expected-answer oracle.

Frozen example `074b97e2…`, compiled at `b16c4ece…`, completed 32 controls and 85
commands with no disagreement or retry. Its [build receipt](results/ace16-cascading-delete-example-build-2026-10-26.json)
pins 55 Git source objects, rechecked at that revision before/after. A later live
checkout is not used as a compiler-input inventory. Each run requires explicit
binary hash/revision/build receipt and fresh work/report paths.

The 24 candidates match the original-association model and the successful public
copies' typed rows, physical locations and logical key order. Independent raw
checks filter complete original key/row entries, verify exact slot/count changes,
preserve all 121 record bodies, and audit all 44 root leaves per candidate. Every byte
outside the independently derived mutation spans remains unchanged, including
maps, historical counters, root headers, Memo and OLE. Read-only getters leave all
candidate files unchanged. Eight self/cycle controls return `ConstraintCoverage`
without stdout or candidate publication, independently of the public defects.

All 241 immutable local inputs/tools/transcripts, 32 request files and 24 created
candidates were rehashed. Full commands, getter values and raw reports remain
ignored with public hashes. This root-leaf corpus adds no multilevel cascade
contraction, SET NULL, self/cycle, data-macro, normal-save or per-row mixed-action
timing qualification. The API retains the documented
[engineering boundaries](ACE16_DELETE_PREVIEWS.md#separate-acyclic-cascading-deletions).
