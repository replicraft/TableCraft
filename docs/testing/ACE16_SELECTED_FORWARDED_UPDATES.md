# ACE 16 selected forwarded-record UPDATE previews

Status: engineering verification passed. All 24 selected-forwarding and
ignored-NULL functions passed, including the eight new forwarding functions.
All nine cascade integration functions passed, including three new policy
controls. Source/expected gates passed before their corresponding API checks.
The full engine library passed 665 tests with two existing ignored tests, and
140 shared regressions passed across 20 targets. Strict all-target/all-feature
Clippy, minimal-feature Clippy and formatting checks passed. Native Access
qualification is not claimed.

## Entry points and storage scope

The explicit `ace16-write-preview` entry points
`NativeImage::preview_ace16_fixed_updates` and
`NativeImage::preview_ace16_batch_updates` can stage eligible fixed scalar and
existing-slot Decimal changes in the resolved record of a selected forwarded
logical row. They retain the existing non-journalable `Ace16WritePreview` result.
Field carriers, expected values, column flags, constraints and physical index
profiles still require their existing qualification.

This is an existing-record change. It allocates no DATA or INDEX pages, moves no
records and rewrites no forwarding pointers. It does not create, shorten or
repair a forwarding chain. Record resizing, new forwarding placement, Memo/OLE
replacement, normal ACE saves and native shared-file transactions remain separate
services or qualification work. The
[ignored-NULL index continuation](ACE16_IGNORED_NULL_UPDATES.md) supplies its own
membership, counter and retained-topology rules.

## Capture before callbacks

The actual shared [root capture](../../crates/engines/tablecraft-ace/src/table/mutation/page_update/roots.rs)
now accepts the existing row-assignment contract. Its earlier variable-record
consumers retain their concrete `SlotScalarUpdate` binding; it is not a copied
forwarding implementation or a new interpretation of pointer bytes.

Capture enumerates the original ordinary owned pages and every live logical
root. The existing bounded [reader](../../crates/engines/tablecraft-ace/src/format/reader.rs)
resolves pointers, validates page owners and slots, and enforces cycle and hop
limits. Capture requires each resolved final page to belong to the source table's
ordinary owned map. It records final physical offsets for the full logical-row
population, then proves that each selected final record has exactly one logical
owner. A matching page header alone does not establish allocation ownership;
an address reached by two roots is not exclusively mutable.

The complete capture, requested-ordinal matching and selected-target ownership
checks finish before the first fixed patch callback. Reader-valid shared or
unowned targets therefore produce `Unsupported(RowLayout)` without invoking a
callback. Wrong owners, malformed pointers, cycles and exhausted hop limits
retain their reader errors rather than being normalized into writable rows.

## Logical identity and physical payload

The [resolved visitor](../../crates/engines/tablecraft-ace/src/table/mutation/rows/final_record.rs)
keeps the two locations explicit. `Row.reference` is the original logical ROW used
by indexes and changed-row accounting. `Row.data` and `Row.offset` describe the
resolved physical payload and its absolute file location. The slot offset and
word still describe the logical root's original directory entry, never the
hidden target's slot.

The [fixed callback](../../crates/engines/tablecraft-ace/src/table/mutation/fixed/masked.rs)
patches only qualified payload and presence spans. A non-NULL value uses its exact
fixed encoding; clearing presence retains ignored payload bytes, and NULL-to-NULL
preserves them. Decimal updates retain their existing 17-byte slot and footer.
The logical root, intermediate pointer records, page headers, free-space fields,
slot directories and hidden/deleted target flags remain unchanged.

Index changes continue to use the logical ROW. A batch can change two indexed
fields in one resolved physical record while preserving that single original
identity. Existing mask coalescing, physical-owner deduplication, historical
counters and final table/relationship checks still apply to the complete image.

## Admission and bounded failure

The direct fixed and batch callers explicitly select the resolved-record
visitor. The DELETE-generated NULL consumer explicitly selects the previous
inline-only visitor. The ordinary Jet 4 planner and slot/variable placement paths
do not acquire a new mutation policy from generic root types. UPDATE-cascade
wrapper now selects the internal strict batch policy on both its no-facts and
expanded-facts routes: original `gates::before`, ordinary index patches and the
inline-only masked visitor. Public fixed/batch selects the extended policy.

The `60b3315` ignored-NULL checkpoint had delegated the UPDATE-cascade wrapper
to the public batch API. That transitively inherited ignored-NULL admission;
the new resolved-record branch would also have leaked through that call. The
current policy split corrects both paths. Earlier statements that cascade
admission was unchanged did not identify this bug, and the earlier passing
verification counts do not establish isolation. The
[causal controls](../../crates/engines/tablecraft-ace/tests/cascade_updates/policy.rs)
qualify a real enforced relationship and independent complete explicit-batch
image before comparing direct batch support with strict automatic-cascade
refusal. They also distinguish an empty table-scoped request from an operation
with no selected tables, which remains an unchanged no-op.

Page reads, retained records, request directories, ownership sorting and binary
lookups charge the supplied byte, row and work pools. Cancellation is observed
through capture, callbacks and final validation. Stale expectations, conflicting
assignments, shared or unowned final records, resource exhaustion and cancellation
return no candidate. No source file, journal or external content is executed or
written by these previews.

## Independent controls and verification

The [core literal controls](../../crates/engines/tablecraft-ace/src/table/mutation/fixed/ignored_updates/selected_forwarded.rs)
construct complete expected images before invoking previews. They cover both
header profiles and key directions, single-hop and chained forwarding, NULL
transitions and no-ops, and multiple physical keys changed on one logical row.
They protect pointer records, hidden slot metadata, map pages and file length.
Wrong-owner, cyclic-target, stale-value and late-cancellation controls are separate.

The [authority controls](../../crates/engines/tablecraft-ace/src/table/mutation/fixed/ignored_updates/selected_forwarded/authority.rs)
first establish that two problematic sources are readable and their indexes are
checked. One has two logical roots sharing a final target, with genuine ordinary
nonunique index declarations so primary uniqueness does not reject it first. The
other has a matching-owner final page outside the ordinary owned map. Direct
visitor assertions require zero callbacks, and public fixed previews must refuse.
Separate controls exercise all three resource pools.

The core and authority source/expected gates passed before the forwarding API
checks. The separate cascade-policy source gate also passed before its API
checks. The focused batches then passed all 24 selected-forwarding/ignored-NULL
functions and all nine cascade functions. The latter includes the three causal
policy functions; these counts overlap the existing regression coverage rather
than representing 33 newly added functions.

The first generic-root compile stopped before tests because two existing
slot-backed consumers still omitted the new `Root` type argument. Their explicit
types were migrated without changing fixture or expected bytes. A later first
cascade-policy source attempt stopped during archive creation with `No space
left on device`, also before tests. After rebuildable-cache cleanup, the same
source gate passed; neither failure was a runtime preview failure.

Final verification passed 665 engine-library tests with two existing ignored
tests, 140 shared regressions across 20 targets, strict all-target/all-feature
Clippy, minimal-feature Clippy and formatting checks. These forwarding and
cascade-policy controls are repository-authored literal models, not original-file
or Microsoft Access save/reopen comparisons. The separate
[original-file ignored-NULL proof](ace16_fixed_ignore_null_updates/README.md)
uses the older `60b3315` producer and does not qualify forwarding or cascade
isolation. Normal write, data-macro and native qualification boundaries remain
unchanged.
