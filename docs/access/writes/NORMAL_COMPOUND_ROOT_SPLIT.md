# Compound DATA allocation and primary root splitting

Normal complete-row INSERT can append one DATA page and two INDEX pages in one
original-bound plan when a qualified manual Int32-primary source exhausts both
record capacity and its root leaf. The actual retained-session consumer uses
existing TCNA4 requests and V2 checked recovery. Native Access save/reopen remains
unrun.

## Admission and physical scope

The original root-overflow selection, native17 catalog, required-program absence,
constraints, ownership and complete source checks remain mandatory. Existing
record reservation selects real EOF growth before staging; the planner does not
catch a failed fitting plan and retry with allocation.

This first compound profile accepts one complete row and one exhausted original
DATA hint. The new DATA page is exactly original EOF; right and left INDEX pages
follow it. All three pages require original represented GLOBAL_FREE=1 and separate
DATA-owned, hint and INDEX-owned=0 authority. Missing windows, conflicting roles,
aliasing storage or map expansion refuse. Neither zero bytes nor EOF grants
permission.

DATA placement, INDEX claims and exhausted-hint changes compose shared map bytes
against the original source, preserving earlier claims. The new row's physical
ROW follows its actual DATA reservation; no caller chooses a page or native slot.
Existing fitting-only splits and DATA-only insertion keep their own paths.

## Proof, recovery and consumers

Independent proof encodes the requested record, proves original capacity
exhaustion and derives its new ROW, DATA framing and map/count changes. It then
independently reconstructs root fences, child populations, histories and the
complete final image. Fresh final Source checks remain mandatory. Proof consumes
no producer reservations or staged page arrays.

TCNA4/V2 retains the original row and BEFORE binding. Recovery freshly admits,
replans and compares complete images before effects. Interrupted DATA/INDEX tails
and causal prefix mixtures roll back and truncate through the existing journal;
committed mismatches and forged requests/AFTERs refuse without effects.

Actual typed sessions pass the exact supplied Binary84 row, then refresh snapshots
and cursors. Stale handles refuse while retaining the completed receipt and lease.
A same-lease empty DELETE preserves the complete split image and uses its own
TCND/V1 lifecycle. No SQL/CLI save is claimed from a substituted NULL or invented
Binary literal.

## Observed checks and limits

The [four-view source gate](../../../tests/reports/indexes/ACE16_COMPOUND_ROOT_SPLIT_SOURCES.md)
passed before producers. Direct literal proofs/forgeries, two native V2 commits,
twenty pending recovery contexts, exact public resource pools/cancellation and
four retained-session commits pass. Earlier fitting root-split native/session
regressions also pass. Strict all-feature/all-target, minimal, journal-only and
preview-only checks pass. Full-suite verification remains distinct from these
focused checks.

Multiple-row compound placement, multiple exhausted hints, recursive/multiple
splits, wider index codecs and map expansion remain work. These checks do not
qualify native Access saves or Windows durability.

The [execution record](../../../tests/results/indexes/ace16-normal-compound-root-split-writes-2026-11-01.json)
retains successful checks, the first source-audit lint failure and corrections.
The preceding full-suite batch is linked separately; it was not rerun per edit.
