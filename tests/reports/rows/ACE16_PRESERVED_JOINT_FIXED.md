# Joint fixed updates in preserved mixed schemas

`NativeImage::preview_ace16_batch_updates` now prepares one same-table candidate
for multiple ordinary Boolean, Int32, Int64, Currency and legacy DateTime
targets in a preservation-qualified mixed schema. Existing `FixedColumnUpdates`
and `FixedScalarUpdate` requests retain original live-row ordinals and exact
expected values. There is no new public request family or CLI flag.

This composes the existing [Boolean](ACE16_PRESERVED_BOOLEAN_UPDATES.md),
[Int32](ACE16_PRESERVED_INT32_UPDATES.md) and
[eight-byte scalar](ACE16_PRESERVED_FIXED_SCALARS.md) preservation profiles.

The result remains an `Ace16WritePreview`, separate from a normal edit plan.
Joint preparation grants no normal ACE save, journal permission, required-macro
absence or native Microsoft Access qualification. Existing capture-policy checks
and image ceilings remain in force.

## Qualified targets and atomic preparation

All selected groups refer to the same immutable source image and table. Every
target, including an empty group, must pass exact native type, width, flags,
fixed-span overlap, index and calculated-dependency checks. Supported payload
widths are 4 for Int32 and 8 for Int64, Currency and legacy DateTime. Boolean
uses its native value bit rather than a payload or nullable presence state.
Boolean NULL requests remain refused.

Indexed, generated, calculated and calculation-dependent targets remain outside
this profile. Text, Binary, LVAL, Decimal, GUID, floating-point and Date/Time
Extended targets do not enter it implicitly. Different tables, duplicate column
identities, duplicate row ordinals within a group, absent rows, stale expected
values and wrong carriers refuse. Different groups may select the same original
row or different row subsets.

Every group's metadata and carriers are qualified before joint record visits.
The producer collects bounded borrowed assignments, orders them by original row
and resolves each selected original record once. It stages disjoint field spans
and selected mask bits without applying one group's changes before another
group checks its original expectation. A late failure returns no candidate.
Existing Required and field/table validation rules still apply to the complete
expected and candidate images.

The changed-row count is the union of changed logical roots. Three changed
fields on one original row count as one changed row. Group or request reversal
does not change the expected image or that union count.

## Shared masks, NULLs and exact bytes

Each mask contribution retains the same original before byte. The existing
span owner merges only disjoint selected bits and rejects overlapping payloads
or repeated bits. A nullable field update therefore cannot overwrite an earlier
Boolean or numeric contribution sharing its mask byte.

NULL-to-value writes the selected payload and sets only its presence bit.
Value-to-NULL clears that bit while preserving inactive payload bytes. NULL
no-ops preserve every original byte, including inactive DATE bytes that would
not decode as an active value. Boolean changes affect only their value bits.
Int32 changes remain four bytes and retain adjacent upper-word padding in the
qualified declaration derivative. Currency uses scaled integer bytes and legacy
DateTime uses exact bits; signed zero and temporal aliases are not normalized.

An independent auditor captures its own original source and reconstructs all
permitted changes without producer patches, encoded values or returned counts.
Stale checks read original record masks; selected bit writes accumulate in its
separate expected image. It independently deduplicates changed roots, validates
the complete expected scalar projection against original properties and compares
every candidate byte and EOF. Rich siblings, stored caches, indexes, allocation
maps, attachments, LVALs, physical record sizes and unrelated bytes remain exact.

## Routing and existing consumers

Existing single-target and healthy Boolean paths remain intact. Typed numeric
or NULL interest selects the new joint owner before Boolean staging. Markerless
empty numeric groups use actual metadata selection in the existing paid lookup.
That newly admitted path may discard an already paid Boolean prefix and prepare
all groups afresh from the original source. No staging error is caught to retry
another writer; no intermediate candidate is chained into a later edit.

The existing Access engineering editor already composes fixed groups from one
snapshot and invokes one native finish. SQL batch planning evaluates every SET
right-hand side against the original snapshot, retaining original row handles.
Typed CLI `--set` and TSV transport likewise preserve original ordinals and
expected/replacement carriers. These shared paths need no new business logic in
the CLI. Their focused joint acceptance runs are tracked separately from native
API acceptance; no frontend result is inferred from engine tests alone.

## Observed authored evidence

SOURCE qualification was committed separately as `931e2972` before producer
installation. The corpus contains 27 recipes per header: 19 positive requests
with complete expected images and eight request-refusal controls. Its 54
original and 38 expected views yield 46 complete header 6 admissions and 46
header 5 Column refusals. Header 5 controls retain unsupported Date/Time
Extended siblings; this is a preservation-profile refusal, not a universal
header 5 joint-update restriction.

The base rich source witnesses Boolean and the eight-byte families. Ordinary
Int32 combinations reuse the already-qualified declaration derivative that
replaces Large_Number's Int64 declaration with width-4 Int32 and retains its
upper four bytes as padding. These are separate source profiles; the corpus
does not claim Int32 and Int64 coexist in that one field.

The focused native target passes five functions, including 76 complete-image comparisons
over both group and row orders. Independent proof passes seven functions covering
root-union counts, shared masks, inactive bytes, DATE aliases, adjacent padding
and caches, alternative requests, original Required rules and Boolean-prefix
reapplication. Initial unit-test compilation stopped on accidental extra arguments
in existing Int32 proof tests; restoring those calls to their original bytes
left every source and expected image unchanged.

The first full preserved contract recorded 50 passes and three obsolete blanket
joint-refusal assertions, with two existing ignored helpers. Those assertions
now compare complete independent positive images while retaining all unrelated
refusals. The corrected full contract passes 53 functions with the same two
ignored helpers. The mutation-library batch passes 609 functions with nine
existing ignored helpers. Strict checks across ACE, Access and CLI targets and
features, minimal ACE/Access builds and formatting pass. The
[native result](results/ace16-preserved-joint-fixed-2026-10-31.json) retains the
initial compilation and obsolete-refusal failures separately from passing runs.
The [frontend record](results/ace16-preserved-joint-fixed-frontends-2026-10-31.json)
separates 76 typed and 24 SQL complete-image adapter comparisons from 32 typed
and four SQL CLI comparisons. Full preserved frontend targets pass 30 adapter
and 27 CLI functions. SQL controls prove original-row RHS evaluation and genuine
empty multi-target selection. Late Required, wrong modes, stale handles,
budgets, cancellation and output conflicts retain their refusals.

Typed CLI transport requires assignments and merges repeated same-name groups.
It cannot literally represent native empty groups or duplicate column-group
requests. Those shapes remain covered by native/shared services; CLI counts do
not substitute fabricated no-ops. A false SQL selection exercises true empty
groups through the existing SQL host.

## Generated-file joint qualification

The [genuine corpus](preserved_joint_fixed_genuine/README.md) reuses the exact
unchanged repository-authored V2016/V2019 originals and read-only getter from
the eight-byte scalar qualification. Original-only requests and expectations
were frozen separately; all twelve expected images passed complete native
Source admission before any producer call. The expectation tranche was committed
as `2f436c5a`.

A fresh isolated build of committed producer `74b6b112` matches all twelve whole
images. Ten typed batches and two SQL calls cover joint same-row changes,
NULL promotion/clearing, overlapping rows, exact no-ops and true empty groups.
Eight images change and four remain byte-identical to their originals. The
changed-root union is twelve across the corpus; each overlapping case counts
three roots rather than the number of changed cells.

All twelve fresh unchanged read-only SDK getter processes match the complete
expected inventories, including calculated caches, Memo/OLE, attachments and
Sentinel. The first saved-only verifier replay passes without child processes.
The [genuine result](results/ace16-preserved-joint-fixed-genuine-images-2026-10-31.json)
retains the actual compilation, source, independent expectation, producer,
reader and replay evidence separately. These are generated-file engineering
observations; Microsoft Access save/reopen, required-macro coverage and normal
ACE save permission remain unqualified.
