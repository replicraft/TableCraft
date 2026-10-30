# ACE 16 historical-column fixed UPDATE previews

All six focused functions pass: two source-only gates and four API/control
functions, including 64 complete fixed/batch image comparisons. Existing
fixed-update regressions pass 54 library functions and 36 functions across five
shared integration targets. Strict ACE all-target/all-feature Clippy and minimal
ACE checks pass. The
[result record](results/ace16-historical-fixed-updates-2026-10-29.json) retains the
source chronology. Native Access comparison is not claimed.

## Qualified source and operation

ACE 16 engineering `FixedUpdate` can qualify surviving columns whose native
numbers have gaps left by deleted columns. Every original live record must still
carry the table's full physical column count, including historical positions.
That count differs from the number of surviving declarations. Shorter records
and newly absent fields remain refused, including with empty update requests.

The qualified public routes are `preview_ace16_fixed_updates` and
`preview_ace16_batch_updates`, returning the existing non-journalable engineering
candidate. Carriers, flags, active content, properties, INDEX profiles and
constraints retain their checks. INSERT, DELETE, slot/variable updates, strict
update cascades and normal Jet 4 UPDATE retain their historical-layout refusal.
Normal ACE saves and required data macros remain separate qualification work.

The source qualifier also belongs to the existing allocating FixedUpdate
pipeline. It introduces no new allocation policy; this historical corpus
qualifies fitting INDEX pages, not growing historical-table images.

## Identities and preservation

The corpus retains live native column zero and selected native column two or
four. It includes removed fixed bytes and, in the variable shape, a removed
variable payload, its offset slot and nonzero removed-column NULL bits. These
opaque bytes are not decoded as surviving values or assigned invented types.

Schema inspection sorts surviving fields by native number. A TD with block order
`Id(0), Score(4), Label(2)` exposes `Id, Label, Score`: selected native column four
has dense position two. The writer uses the actual native number for its mask
bit, stored fixed offset for payload and original logical ROW for INDEX keys.
It does not renumber metadata to match a dense output vector.

Expected images preserve removed payloads, variable offsets, unrelated NULL bits,
TD bytes, row headers and slot words. An ordinary UNIQUE key and logical alias
share one physical owner, rewritten once; its historical counter remains 29.
ASC/DESC key and ROW ordering are independently authored from literal values.
These fixtures do not establish original-file or native deleted-column editing
qualification.

## Complete proof before staging

The [historical source qualifier](../../crates/engines/tablecraft-ace/src/table/mutation/gates/historical.rs)
runs only for gapped `FixedUpdate` admission. It pays for the complete original
row selection and reuses the actual
[logical-root owner](../../crates/engines/tablecraft-ace/src/table/mutation/page_update/roots.rs).
That owner verifies declared/live counts, DATA ownership and exclusive resolved
final-record identities before returning records. Existing forwarding, aliases,
wrong-owner and cycle checks remain authoritative.

Every original record must have the full physical count and complete valid
fixed/variable framing. This happens before selected callbacks, even for no-op or
empty requests. Existing masked staging then changes only selected payload and
presence bytes. Normal before/final constraints and complete-byte preservation
checks remain. Allocations, copies, work and cancellation use the caller budget.

## Independent evidence and failure history

The [literal model](../../crates/engines/tablecraft-ace/src/table/mutation/fixed/historical_columns/model.rs)
defines 32 pairs: two headers, ASC/DESC keys, removed fixed-only or fixed-plus-
variable storage, present/NULL input and selected native number two/four. All 64
source/expected images passed row, INDEX and field-rule checks before mutation.

The first source gate found nullable Score rows under `>=0 And <=100`: the result
is NULL, which the current rule auditor reports unsupported. A diagnostic
confirmed Required and AllowZeroLength were NotDeclared while Validation was
Unsupported. Before any mutation API, nullable fixtures acquired the explicit
supported rule `[Score] Is Null Or ([Score]>=0 And [Score]<=100)`; non-NULL cases
retained their original rule. Active range validation remained; assertions and
engine gates were not weakened.

The next gate caught authored row facts assuming TD block order. Those facts
were corrected to native-number order; source and complete expected byte
construction remained unchanged. The corrected source gate then passed before
production activation or API execution. Failed programs, models, logs and
derivative freezes are preserved privately.

The [API controls](../../crates/engines/tablecraft-ace/src/table/mutation/fixed/historical_columns/preview.rs)
cover both exact routes, byte-identical no-op/empty results, readable shorter
records refused even for empty requests, unknown flags, missing ordinals,
overflowing carriers and unchanged other admissions. Measured exact/one-below
Bytes, Rows and Steps limits and final-poll cancellation prove failure returns no
candidate. No expectations changed after observing producer results.

The executable [historical SQL UPDATE target](../../apps/tablecraft-cli/tests/query_preview_historical_updates.rs)
reuses this same model through an import-only test seam. Both target functions
pass, including eight actual complete-copy comparisons. The source-only function
passed separately before those commands. The existing pure SQL
`Score=Id*8+9 WHERE Id<=2` produces the already frozen values 17 and 25; expected
images were not adapted to the frontend. This is a fresh engineering copy through
the common publisher, not a normal native save.
