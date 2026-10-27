# Alternate-page and EOF ACE 16 update previews

With `ace16-write-preview`,
`NativeImage::preview_ace16_variable_updates(table, column, updates, budget, cancelled)`
consumes existing `SlotScalarUpdate` requests and returns a non-journalable
`Ace16WritePreview`. This separate engineering service adds alternate owned-page
placement and bounded EOF allocation. The
[root-page-only service](ACE16_PAGE_UPDATES.md) and exact-span slot service keep
their existing contracts. Normal backend/CLI writes, journal admission and
unqualified data macros remain blocked.

## Placement and storage

Expected decoded values are checked before recognizing semantic no-ops. No-ops
preserve every source byte. Fitted replacements keep the original final physical
record capacity, slot and page free count. The existing variable record codec
retains other field payloads, opaque gaps, unused mask bits and padding, with an
independent record audit. It proves Text decoder roundtrip and rejects leading
FEFF aliases rather than silently changing the requested value.

For growth, the service selects the highest owned data page with a true table
candidate-hint bit. It checks actual free bytes, slot capacity, signature, owner
and global allocation. A fitting candidate receives a new hidden last slot. If
that single highest candidate does not fit, its hint is cleared and a new EOF
page is allocated; lower candidates are not searched. With no hinted candidate,
a new EOF page is allocated directly. This finite policy follows the reviewed
independent writer's maintenance control flow and the
[actual public UPDATE observations](CROSS_PAGE_UPDATE_OBSERVATIONS.md).

EOF allocation requires existing represented owned, hint and global cells for
the exact appended page: original owned/hint bits must be clear and the global
free bit set. The service sets owned/hint and clears global free, merging edits
sharing a bitmap byte. It initializes a complete zero-filled data page with the
selected table owner and exact count/free/hidden-slot framing. Missing bitmap
representation refuses; map growth, window shifts, new bitmap storage and reuse
of arbitrary in-file free pages are not implemented. Candidate size retains the
64 MiB captured-image ceiling.

The original logical root and every index row reference stay stable. A growing
ordinary root becomes a forwarding stub; an already-forwarded root is repointed
while its stub residue and old hidden targets remain exact. Physical target
exclusivity is proved by the complete logical-root census before staging.
Multiple changes follow original row traversal order, independently of request
slice order. Original unselected bodies and table/historical counters remain
unchanged. Native pages retain the qualified maximum of 255 slots.

## Independent candidate checks

The service retains the existing column, index and constraint admission gates,
including unsupported indexed French and ignored-NULL profiles. Genuine
single/mixed-tuple key replacement and complete ancestor/index checks remain
mandatory. Required, field rules, uniqueness and relationship checks apply to
the complete candidate before publication.

A separate append-aware finalizer owns original/candidate byte vectors directly;
it never creates an unequal-length `NativeEditPlan`. Normal equal-length apply,
preservation and journal paths are unchanged. It validates source identity,
length alignment/cap and sequential EOF extents before allocating output.
Original-prefix preservation admits only staged data pages, exact map bytes and
qualified index spans. New pages and root/destination edits have independent
header, free-count, slot, pointer, residual-body and zero-padding audits.

An independent source-and-event replay reconstructs highest-candidate selection
and actual prior slot/free geometry for every append. Only a genuine failed fit
can clear a hint. Added ownership/global allocation bits must correspond exactly
to actual EOF pages. Its expected role-specific byte edits are derived without
trusting the planner's writable masks; extra or contradictory map effects
refuse. A causal test preserves the initial auditor gap where a forged clear of
a fitting page's hint was accepted, then demonstrates the corrected refusal.

The finalizer reparses the actual grown header/catalog, performs complete table
and constraint checks, and verifies final owned/global/candidate membership.
Budgets and cancellation cover captured roots, serialization, placement, map
composition, key replacement and independent audits. Failures publish no
candidate; no source file is modified.

## Validation boundaries

The [scoped implementation record](results/ace16-variable-updates-native-2026-10-26.json)
records exact source/log hashes, failures and focused regression commands.
Author tests reuse repository-owned encoded fixture mechanics and literal key
expectations; the independent contract target authors its own storage/map model.
Private causal tests forge final metadata, bodies, unauthorized map bits and
length/placement facts. These checks establish bounded engineering behavior,
not Microsoft Access native save/reopen compatibility.

The eight public-writer UPDATE observations establish independent storage facts.
The subsequent [original-file comparison](VARIABLE_UPDATE_OBSERVATIONS.md)
passes all eight candidates on its first run with 32 commands. Each shrink
starts from the corresponding generated growth candidate. Full typed values,
index orders and exact image/map/EOF/padding proofs agree. These later results
remain separate from the original core test record and native Access qualification.

The same committed consumer separately passes the
[eight Binary allocation controls](BINARY_VARIABLE_UPDATE_OBSERVATIONS.md)
against genuine space-exhaustion checkpoints. All eight preserve complete
typed/index results and independently reconstructed image/map changes. Four
shrinks consume their corresponding generated growth candidates and retain
268-byte capacity and opaque padding; the public writer's zeroed padding is
not substituted as an engineering golden.
