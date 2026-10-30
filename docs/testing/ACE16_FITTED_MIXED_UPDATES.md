# ACE 16 fitted mixed UPDATE previews

The engine can apply Short Text/Binary fields and optional fixed scalar
assignments jointly to one original table; see the [multiple-variable
extension](ACE16_MULTIPLE_VARIABLE_UPDATES.md). The original one-variable
feature passed all eight focused mutation functions,
including 83 complete-image comparisons. Both source-only gates passed before
their respective mutation cases. Three existing independent record-proof tests,
36 existing variable-update functions across five integration targets and two
joint-key regression functions pass. Strict ACE all-target/all-feature Clippy and
minimal ACE checks pass. This is engineering preview evidence; native Access
save and original-file mixed-update qualification are not claimed.

The subsequent [body-ownership correction](ACE16_FITTING_BODY_ROLES.md)
adds complete source-role protection to this nonallocating route. Historical
fixture constructors remain unchanged; positive regression controls now use
explicitly framed ownership-list derivatives qualified before their mutation
calls. The earlier counts above describe the original feature validation.

## Working API and admission

`NativeImage::preview_ace16_fitted_updates` takes borrowed
`FittedColumnUpdates::Fixed` and `FittedColumnUpdates::Variable` groups. Requests
must contain at least one ordinary Short Text/Binary group; fixed groups are
optional. Multiple variable fields are composed jointly. Every cell carries an original row ordinal, exact expected stored value
and replacement. Duplicate columns or repeated assignments within a column
refuse. Expected values are representational preconditions, including exact
Text/Binary bytes; they are not collation-based SQL comparisons.

The route retains existing slot-update source qualification. Fixed fields use
the existing exact scalar encoder and ordinary field flags. Variable fields use
the qualified ordinary Short Text/Binary profile. Memo/OLE, generated fields,
unqualified properties, historical column layouts and unsupported indexed
profiles remain refused. Affected IgnoreNULL indexes are not admitted by this
route. Independent fixed-update capabilities do not implicitly widen it.

The result is the separate non-journalable `Ace16WritePreview`. Normal ACE
writes, required data macros, journal activation and native save permission stay
behind their existing gates. This API does not convert a preview into a normal
write plan for callers. See [engineering preview boundaries](ACE16_WRITE_PREVIEWS.md)
and the separate [variable-record allocation service](ACE16_VARIABLE_UPDATES.md).

## One original row and one joint key transform

The [mixed stage](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/stage.rs)
resolves selected columns and affected key siblings from the original schema.
It captures original logical roots through the existing owned/exclusive
[record resolver](../../crates/engines/tablecraft-ace/src/table/mutation/page_update/roots.rs).
Row identity remains the logical root even when the final payload resides in an
existing hidden forwarded record. Wrong ownership, shared final targets and
invalid forwarding retain their checks.

Each changed row is rebuilt once from its original bytes. The variable field
replacement and all fixed payload/presence changes produce one final record.
Fixed NULL assignments retain their original opaque payload bytes. Unselected
variable payloads, unrelated mask bits and unconsumed padding remain exact.
There is no intermediate candidate or modified-source value override.

The [joint replacement owner](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/replacement/mixed.rs)
qualifies every affected alias before deduplicating physical owners. It checks
the original encoded key and derives the complete final key, applying all
selected components together. Each physical owner is transformed once. This
allows the corpus's UNIQUE tuple permutation without accepting a sequential
intermediate collision as the final state. Logical aliases do not multiply
physical changes or historical counters.

## Existing physical capacity and text representation

No DATA or INDEX pages are allocated. The final record must fit its existing
physical capacity, and each INDEX replacement must fit the existing writer
profile. Existing exclusive forwarded targets can be updated; creating a new
forwarding placement is outside this operation.

The additive [mixed DATA-growth service](ACE16_MIXED_GROWING_UPDATES.md) now
handles qualified new physical targets under represented-map limits. It uses
the existing minimum-size variable serializer; this fitting route retains its
own declared-form preference and capacity contract.

Available selected payload capacity is derived from original checked framing:
`data_end - (payload_end - selected_span)`. It preserves every unselected
payload extent and permits consuming only genuine internal padding. Padding is
not interpreted as part of the selected string.

The mixed engineering serializer prefers the declared Unicode compression form
when all UTF16 units are nonzero Latin1 and that form fits. This includes the
marker-only empty representation. If the marker would exceed capacity, legal raw
UTF16 is selected when it fits. If neither form fits, the operation returns
`Unsupported(RowLayout)` without a candidate. The choice happens before payload
emission within one plan; it is not a preview retry. Ignored bytes under a NULL
mask are never inspected to infer a prior encoding form.

This is an explicit mixed serialization policy, not a claim of native Access
packing parity. Existing single-variable updates retain their minimum-size
serializer, including the observed raw UTF16 tie policy.

## Independent authority before broad record patches

The [record proof](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/record_proof.rs)
starts again from the immutable image and original request groups. It rechecks
source admission, resolves columns and roots, verifies original expected values,
and independently reconstructs the complete final record. Its variable framing
and payload rendering use the existing independent record-proof owner, with its
own capacity and encoding selection.

Only exact full-record equality authorizes a record patch. The proof compares
logical reference, physical offset, original bytes, complete final bytes and
exact original/final value projections. It then rebuilds joint key inputs from
those independently proven values and compares complete expected INDEX patches.
Producer key caches and agreement with an INDEX page are not authority for
changed record bytes. Final table/constraint checks and complete-image
preservation still run before publishing the candidate.

Metadata comparisons, deep Text/Binary comparisons, copied records, projections
and key work use the caller budget and cancellation callback. Work bounds use
actual index-column counts. Errors return no partial candidate.

## Independent fixtures and retained failures

The [original literal model](../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/model.rs)
defines 39 source/expected pairs: both headers, Legacy/Modern text and Binary,
ASC/DESC keys, joint UNIQUE permutations, shrinking payloads, shared NULL masks
and existing forwarded targets. Sources include nonzero NULL payloads and eight
bytes of genuine internal record padding. Both complete images of every pair
passed row, INDEX and constraint checks before mutation. The API checks cover
36 inline cases in both group orders plus three forwarded cases: 75 comparisons.

The first source gate found a fixture declaration error: compressed text used
extended flag 2, which the actual scalar decoder does not qualify. A declaration
diagnostic established UnicodeCompression flag 1. Only the authored compressed
declaration was corrected before any mixed producer ran; source assertions and
engine admission were not weakened. The corrected 39-pair gate passed.

The first mutation run passed five functions and failed the whole-image function:
the existing minimum-size serializer used two raw UTF16 bytes for one-character
text, while the qualified expected images used the declared three-byte
compressed form. INDEX pages agreed. That failure and its program are retained.
The producer and independent proof acquired the explicit mixed encoding policy;
the qualified expected images were not changed.

A capacity review then identified a real fitting case: a one-character raw field
with compression declared and no spare byte. The separate
[zero-slack literal owner](../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/capacity.rs)
defines four additional source/expected pairs. Its first source-only run caught
an authored descriptor offset that omitted three physical counter records.
Correcting that metadata offset preceded every new-case mutation. The corrected
eight complete images passed source checks, and the original 39-pair constructor
remained unchanged.

The [focused mutation controls](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/tests.rs)
and [capacity controls](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/tests/capacity.rs)
then passed all eight functions, including the original 75 images and eight
zero-slack comparisons in both group orders. They also cover stale original
expectations, neither-form-fits refusal, exact/one-below Bytes/Rows/Steps limits,
final-poll cancellation, and correlated forgeries of selected fixed bytes,
unselected fields and collation-equivalent value caches. Complete-file equality
remains exact; diagnostics report lengths and differing offsets instead of
dumping full images.

The two separately focused joint-key functions passed their first run. They
verify all frozen physical owners and reject altered original keys or missing
logical ROW associations. No original application or native Access execution is
implied by these independently authored fixtures.

After the capacity-aware rerun, the existing encoded-split, page-update,
slot-update, variable-update and variable-update-contract targets passed
9, 7, 9, 4 and 7 functions respectively. These regressions exercise the existing
single-variable consumers whose serialization policy remains unchanged. They
do not establish growing-record support for the mixed route.

## Record-growth source qualification

A separate [literal growth corpus](../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/growing.rs)
now qualifies 36 source/expected pairs before any growing mixed-update producer
runs. Both header codes, three key families, both index directions and three
placement destinations are represented: a hidden slot on the original page, an
existing owned page, and a new DATA page at EOF. Reads, indexes, constraints,
source-role metadata and original logical-root resolution pass for all 72 images.
The growing text expectations use the existing minimum-size variable serializer;
the fitted service retains its distinct capacity-aware encoding policy.

The first source gate stopped at source-role admission because three independently
authored table definitions lacked their empty declaration terminators. The new
corpus alone acquired explicit terminators at its known definition ends before
the successful rerun. Existing fitted fixtures, engine guards and expected values
were unchanged. The [source record](results/ace16-mixed-growth-sources-2026-10-30.json)
preserves both attempts. This is source qualification, not a passing growth
producer, CLI capability, native Access comparison or save qualification.
