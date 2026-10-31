# Preserved eight-byte scalar engineering updates

The existing `NativeImage::preview_ace16_fixed_updates` and
`preview_ace16_batch_updates` services admit one ordinary, unindexed,
calculation-independent eight-byte scalar target in a preservation-qualified
mixed schema. Requests use existing `FixedScalarUpdate` and `FixedColumnUpdates`
types with exact expected and replacement carriers. The result remains a
non-journalable `Ace16WritePreview`; it grants no normal ACE save, required-macro
absence or execution permission.

This extends the closed preserved fixed profile alongside
[Boolean](ACE16_PRESERVED_BOOLEAN_UPDATES.md) and
[Int32](ACE16_PRESERVED_INT32_UPDATES.md). It introduces no public setter family,
implicit coercion or retry through another writer. Existing healthy Boolean and
Int32 branches retain their established work and resource controls.

## Native target and request boundaries

Three scalar families are supported. The authored corpus exercises four actual
fields from the unchanged rich source:

| Field | Native type | Carrier | Fixed offset | Width |
| --- | ---: | --- | ---: | ---: |
| Large_Number | 19 | Int64 | 8 | 8 |
| Date_Field | 8 | DateTime | 16 | 8 |
| Time_Field | 8 | DateTime | 24 | 8 |
| Currency_Field | 5 | Currency | 116 | 8 |

Offsets describe these fixtures, not universal field positions. Admission uses
the actual native declaration: fixed and updatable, exact scalar/native type,
width 8, no extended flags and no overlap with another fixed payload. Date and
Time are the same legacy DATE storage family. Single, Double, GUID and Date/Time
Extended do not enter this new profile merely because the encoder can represent
other types.

Only one target group is admitted. Indexed, generated, calculated or
calculation-dependent targets, multiple scalar targets and mixed Boolean/scalar
groups remain refused. Expected and replacement values must be NULL or the exact
target carrier. Caller-supplied ordinals and original expectations are checked;
duplicate, absent and stale rows do not silently change the selected operation.

NULL and empty requests require actual metadata selection. An expected NULL
cannot select Int32 merely because that older profile also supports NULL.
Selection occurs before row staging, using the existing paid name/field binding;
no conflict, cancellation, resource or staging error triggers another writer.

## Exact payload and NULL preservation

Int64 stores its exact little-endian signed eight-byte payload. Currency stores
the exact scaled integer; no floating-point conversion occurs. Legacy DateTime
stores its exact carrier bits. Signed-zero and temporal aliases are not
normalized or compared with SQL equality. A representational change counts as
a change even where a different comparison policy could treat values as equal.

Present-to-present writes change only the selected eight-byte field. NULL-to-value
also sets only that field's presence bit. Value-to-NULL clears only that bit and
retains the now-inactive payload bytes. NULL-to-NULL preserves the complete
original representation, including inactive bytes that would not decode as a
valid active DATE value. Other mask bits, padding, record lengths, roots, page
layout, indexes, allocation maps and EOF remain unchanged.

The stage uses bounded, prepaid payload/presence buffers and checks cancellation
and cumulative resource pools. Independent proof recaptures the original source
and rebuilds the permitted payload/mask overlays from original requests rather
than trusting staged patches or counts. It compares the entire image and changed
row count. The independently reconstructed expected image and the actual final
candidate retain the existing field/table constraint checks, including Required.

## Complete preservation policy

The unchanged complete preserved-source owner still qualifies native catalog
origins, names/GUIDs/properties, records, indexes, GLOBAL/maps, LVAL ownership,
calculated dependencies and attachment associations. Participating relationships
and unknown or unsupported semantic payloads retain their refusals. The selected
field metadata alone is not authority to edit the database.

The rich authored header 5 sources retain two Date/Time Extended sibling fields
whose scalar metadata is unsupported under that header. They are explicit
whole-preservation refusal controls; their bytes are not retargeted or removed.
This does not establish a general restriction against the three selected scalar
families in every header 5 database.

## Observed authored evidence

The [result record](results/ace16-preserved-fixed-scalars-2026-10-31.json)
separates source qualification, producer comparisons, causal proof and regressions.

SOURCE qualification was committed separately as `70a1f0c4` before producer calls.
It covers 78 original views and 70 expected views: 74 header 6 views pass direct
complete source admission, and 74 header 5 views retain their typed Column
refusal. The corpus includes signed integer and Currency endpoints, exact DATE
bits, NULL transitions with inactive sentinels, no-ops, multirow requests and
healthy Required refusal sources.

The first native API execution passes six functions, including 70 complete-image
comparisons through the fixed and batch services over 35 admitted expected
images. The expanded full preserved contract passes 48 functions with two
existing ignored helpers; ACE strict checks across all targets/features and the
minimal build also pass. The mutation-library batch passes 601 tests with seven
existing ignored helpers. This includes five direct proof tests that reject
wrong counts, DATE aliases, inactive NULL normalization, unrelated byte changes,
wrong carriers, foreign metadata and substituted requests. A forged observed
Required-property disable still fails validation of the proof's independently
reconstructed image against original properties.

The [frontend result](results/ace16-preserved-fixed-scalar-frontends-2026-10-31.json)
records the separate consumer runs and retained first failure.
The shared Access editor now admits the same target families in Default and
Fixed(Fitted) modes. Variable remains Short Text-only for projected rich schemas.
Seventy typed and eight SQL complete-image comparisons pass through the adapter;
35 typed and four SQL images pass through the CLI. The complete preserved
frontend targets pass 25 adapter and 23 CLI functions. Required, header-profile,
mode, handle, mixed-target, budget, cancellation and output-conflict checks retain
their refusals. The first CLI run retained a test that incorrectly used excluded
`CDate`; its correction selects admitted date literals or NULL from the existing
frozen corpus without changing expected bytes or function admission.

Genuine-file qualification for this new profile remains separate. Native
Microsoft Access save/reopen, required-macro discovery and normal ACE writes
remain unqualified.
