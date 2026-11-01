# Column identities in normal ACE actions

ACE column metadata carries three separate identities. Normal INSERT and DELETE resolve each according to its physical meaning; a gap in one identity does not create a visible value.

| Identity | Meaning | Use |
| --- | --- | --- |
| Visible position | Position in the complete schema/value sequence exposed to a caller | Complete INSERT values, decoded row cells and projected snapshots |
| Native column number | Number declared in the native field descriptor | Index key declarations, record presence bits and native record column extent |
| Stored column ID | Stable identifier in the table definition | Raw declaration identity and preservation; never a value-vector index or mask bit |

A table with three visible fields may declare native numbers 0, 8 and 9. Its complete INSERT request still supplies three values. Its record declares a native extent of ten columns and owns two presence-mask bytes. The first visible value uses bit 0, the second bit 8 and the third bit 9. Unused native positions remain opaque gaps; the action does not synthesize NULL cells for them. Stored IDs may instead be 0, 3 and 4 while native numbers remain 0, 1 and 2; that changes neither value positions nor the one-byte mask.

## Admission

The native 17-field target-table action path freshly checks the complete catalog record, required-program absence profile, selected table definition, all-local LVAL declarations and full preserved Source. Native numbers must be unique, strictly increasing and below the declared native extent. Stored IDs must be unique, strictly increasing and distinct from the reserved FFFF value. Index declarations resolve their actual native field before inspecting its type, physical width, flags and dependencies. Logical aliases share one physical owner only when their declarations agree.

The initial sparse-native profile permits qualified fixed fields. It does not infer missing variable-slot declarations from gaps. Fixed spans must be bounded and nonoverlapping, and their maximum extent must agree with the original definition. The independent record proof derives the mask width from the native extent and reconstructs bits from native numbers. All unused mask bits, inactive field payloads and unrelated record/page bytes remain subject to complete-image preservation.

This action profile does not broaden canonical global content closure, positive BeforeChange processing or existing UPDATE target policy. Those consumers retain their dense-column admission. Normal omitted/default INSERT resolution also retains its existing canonical Source boundary. Unsupported shapes remain explicit refusals rather than alternate-route retries.

## Requests and recovery

Existing request wire formats are unchanged. A complete row retains its original visible value order; DELETE retains the original ordinal and complete raw witness. No native gap values, stored-ID remapping or permission certificate are serialized. Native recovery freshly captures the original image and table, resolves declarations again, reconstructs the action from its original request, and compares both complete images before effects. Foreign same-byte image instances do not satisfy the borrowed Source identity seal.

## Qualified source families and acceptance

Independent source observations distinguish three cases:

- SDK-created stable IDs: native numbers 0/1/2, stored IDs 0/3/4, native extent 3.
- Authored native gaps: native numbers and stored IDs 0/3/4, native extent 5; three visible values and one presence byte.
- Authored crossing-byte gaps: native numbers and stored IDs 0/8/9, native extent 10; three visible values and two presence bytes. The qualified masks are raw 01 03 for present values and 01 02 when the middle field is NULL, with its inactive payload retained.

The authored gap files are format witnesses, not claims about an Access application's dropped-column workflow. Source native/read-only SDK qualification precedes mutation qualification. Two focused independent proof functions, six native action/recovery/admission/resource functions, two borrowed-certificate guard functions and one typed/SQL host function pass. The first direct runs exposed two dense ordinal assumptions in declaration and INSERT qualification; their receipts and the unchanged expected bytes are retained. Broader regression results are recorded separately in the [batch execution record](../../testing/results/ace16-sparse-actions-and-creation-properties-2026-11-01.json). Native Access save/reopen remains unrun.


The combined local batch passes 1,148 ACE units, 79 adapter units, 102 CLI units
and 90 default recovery functions. Strict all-feature, journal-only, preview-only
and minimal checks pass. Fixture-dependent ignored functions are covered only by
the separately executed focused receipts. Native Access and Windows qualification
remain unrun.
