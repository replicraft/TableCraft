# Preserved fitting Short Text and fixed updates

The existing `NativeImage::preview_ace16_fitted_updates` engineering API routes
preservation-qualified mixed schemas to one original-image record composer. It
accepts exactly one ordinary Short Text `FittedColumnUpdates::Variable` group
alongside ordinary Boolean, Int32, Int64, Currency or legacy DateTime `Fixed`
groups. All selected fields must be unindexed and outside calculated
field dependencies. This route reuses the existing borrowed fitted request
shape without a new setter or request type.

Selection uses original native metadata before ordinary full-row admission;
there is no retry after a failed staging or preservation check. A single rich
Short Text target retains the existing preserved Text service. Ordinary fitted
requests retain their own table, index and full-row checks. This joint route
requires Standard capture and introduces no larger-image policy. Additional
variable targets, Short Binary, Memo/OLE, attachment, calculated and other fixed
scalar families are outside this profile, including empty target groups.

See [preserved Short Text](ACE16_PRESERVED_SHORT_TEXT_UPDATES.md),
[joint fixed updates](ACE16_PRESERVED_JOINT_FIXED.md) and
[engineering previews](ACE16_WRITE_PREVIEWS.md) for the adjoining contracts.

## One original record and one final image

Every request expected value refers to the same original image. All targets,
carriers and per-field ordinals are qualified before record composition, even
for empty or no-op groups. Duplicate target fields or duplicate ordinals within
one field refuse; different fields may select the same original row.

Each affected row produces at most one complete physical-record replacement.
Changed Text is encoded using its declared policy and actual original capacity,
then reconstructed from the original record. Fixed payloads and individual mask
bits are applied afterward to that buffer. This order prevents the Text
serializer's copied original mask from erasing numeric or Boolean changes.
Fixed stale checks still use original bytes, and mask updates accumulate only
their own selected bits.

A changed record must retain its exact original physical length and location.
Sibling variable byte spans can move *inside* that record when Text length
changes; their contents, including opaque calculation capsules and LVAL
references, remain exact. Existing unused slack follows the fitting serializer's
preservation policy. Raw Text and inactive NULL Text no-ops are not re-encoded
when a fixed sibling changes. Numeric NULL clears only its presence bit and
retains inactive payload bytes; Int32 retains neighboring upper padding. Legacy
DATE expectations and replacements compare exact bits, including signed zero.
Boolean values occupy their native value bits and do not admit NULL.

There is no record relocation, forwarding, page or index allocation, map rewrite
or EOF growth. An oversized Text replacement refuses instead of entering the
separate [Text growth service](ACE16_PRESERVED_TEXT_GROWTH.md). Changed-row counts
are the union of original logical roots: a row changed by Text and several fixed
fields counts once. An entirely empty or effective no-op statement keeps the
whole original image exact after target metadata qualification.

## Independent preservation and rules

The independent proof recaptures its own complete original Source and binds the
original request targets itself. It reconstructs changed Text through the
separate record auditor, checks fixed expected carriers against original masks
and payloads, and applies its own explicit little-endian fixed encodings. Stage
bindings, cached projections and record patches are not proof authority.

The proof checks the final selected Text values, recomputes the changed-root
union, validates its independently reconstructed complete ordinary projection
against original qualified properties, and compares every observed byte and
EOF. A self-consistent physical image cannot bypass Required or
AllowZeroLength rules. The coordinator then recaptures the candidate under the
original image policy and repeats complete Source qualification. Shared Bytes,
Rows and Steps budgets and cancellation apply throughout.

## Qualification evidence

The authored Source tranche was committed as `a356f1d5` before any new candidate
execution. Its two read-only gates passed: 27 original images and 18 complete
expected images per header, totaling 90 views. The 45 header-6 views passed
complete private Source admission; the 45 header-5 views retained the typed
unsupported-column refusal caused by their existing Extended Date siblings.
This is an authored-profile distinction, not a general header-5 write refusal.

The corpus has 18 admitted request cases and nine request-refusal controls.
It includes raw and inactive Text no-ops, NULL/empty transitions, shared masks,
opaque sibling movement, inactive DATE payloads, the existing Int32 declaration
and padding derivative, overlapping row sets, empty groups and original
Required/AllowZeroLength properties. Original and complete expected bytes were
frozen independently of the producer.

All six focused native API/proof functions pass on their first execution.
They include 72 complete-image comparisons across group and row orders,
typed/profile refusals, exact and one-below resource pools, final cancellation
and causal changes to payloads, masks, siblings, footer, padding, count, EOF and
request authority. Final-rule controls independently reconstruct physically
consistent rule-invalid images and retain Required/AllowZeroLength refusals.
The broader mutation batch passes 616 functions with ten existing ignored
helpers; the public preserved contract passes 54 with two ignored helpers.
Strict all-target/all-feature ACE, Access and CLI checks and minimal ACE/Access
builds pass. The initial strict run found an assertion-only range loop; its
iterator rewrite changes no original or expected construction bytes.

The [native result](results/ace16-preserved-joint-text-fixed-2026-10-31.json)
records focused checks, broader regression and the initial strict diagnostic.
Frontend consumer qualification is recorded separately.

These engineering candidates grant no normal ACE save or journal permission,
no data-macro absence or execution authority, and no Microsoft Access save/reopen
qualification. Genuine-file, independent-reader and platform evidence remains
separate from this authored corpus.
