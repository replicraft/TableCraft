# ACE 16 multiple-variable UPDATE copies

One statement can jointly replace multiple admitted Short Text/Binary fields and
optional fixed siblings. The existing `FittedColumnUpdates` requests and three
native entry points remain: fitted records, DATA growth with fitting INDEX
pages, or one shared DATA/INDEX allocation continuation. No intermediate
candidate becomes the source of another assignment.

SQL default routing uses the fitted service for multiple variable targets.
Explicit `--variable` permits DATA placement; adding `--index-growth` selects the
joint continuation. Typed `write preview-update --variable` groups multiple
Text/Binary targets into the same service. A single variable target retains its
existing route, and Memo/OLE retains its explicit single-column profile.
Fixed-only and unsupported mode refusals remain separate.

Every original expected cell is checked before composing a row. Changed variable
slots are serialized together, in native slot order, with one final offset table
and presence mask. Unselected and no-op spans, opaque NULL payloads and remaining
padding stay exact. Fitting chooses declared compression forms if their aggregate
payload fits; deterministic raw fallbacks use native slot order. Growing profiles
retain their existing minimum-size encoding policy.

The independent proof reacquires source qualification and body-role protection,
resolves original requests again, and reconstructs complete final records with
its separate encoder. Joint INDEX projections derive from those records and
original requests, rather than producer caches. Alias qualification, final tuple
constraints, shared maps, resource quotas and cancellation remain active. This
extends the existing record composer; it does not grant normal ACE saves,
journalability, required macro execution or native Access qualification.

The independent corpus contains 36 two-variable pairs and eight additive
three-variable pairs. Native controls pass six tests and 176 complete-image
comparisons across group and row-order reversals. The corpus covers Text/Text,
Text/Binary, Binary/Binary, optional fixed siblings, overlapping/disjoint rows,
NULL/empty, UNIQUE permutations, aliases, forwarding, capacity fallback and both
growth profiles. Stale/duplicate intents, exact budgets, final cancellation and
correlated record/cache/INDEX forgeries have focused controls.

Both frontend SOURCE gates passed all 44 pairs before mutation registration.
Access passes three API tests: 88 typed and eight SQL image comparisons, plus
mode/zero-match controls. CLI passes its SOURCE gate and three API tests: eight
SQL and 12 typed complete copies, plus late-stale/mode refusals. The selected
combined regression passes 540 native mutation functions, 19 Extra functions and
20 preserved scalar/UI functions (three existing ignored exports). Access passes
63 library and two preserved-Boolean integration tests. The CLI passes 40 unit
and 45 integration tests across five targets. Minimal ACE/Access, formatting,
local links and dependency boundaries pass. Strict all-target/all-feature checks
for ACE, Access and CLI pass.

SOURCE framing corrections preceded writers. Test-only type/import/borrow
corrections and bracket quoting of reserved SQL names are retained in the
[execution record](results/ace16-multiple-variable-updates-2026-10-30.json). Obsolete one-variable
refusal assertions were migrated explicitly; fixed-only, capacity, cross-table,
unknown-kind and mixed-LVAL refusals remain. No expected bytes changed after
writers ran. The existing [fitted contract](ACE16_FITTED_MIXED_UPDATES.md)
and [body-ownership guard](ACE16_FITTING_BODY_ROLES.md) remain active.
