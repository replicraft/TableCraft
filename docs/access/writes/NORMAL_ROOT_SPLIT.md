# Normal Int32 primary root splits

Normal complete-row INSERT can promote a qualified manual Int32 primary root
leaf into a two-child NODE when the existing root cannot fit the new membership.
The actual retained-session consumer uses existing TCNA4 requests and V2 checked
recovery. Native Access save/reopen and Windows qualification remain unrun.

## Original-source authority

Selection uses a sufficient overflow bound from original metadata and requested
row count. It does not catch a failed fitting plan and retry with allocation.
The selected path freshly validates the native catalog, required-program absence,
constraints, complete source, canonical physical ownership and original key/ROW
membership. The existing no-split and older fixed-intent paths remain separate.

New records must fit existing owned DATA capacity. Exactly two INDEX pages append
at the original EOF, with right and left roles derived from the source. Each page
requires explicitly represented GLOBAL_FREE=1 and INDEX-owned=0 cells. Zero bytes,
EOF position, a creator receipt or an absent map cell grant no allocation authority.

Map storage is checked against all catalog, DATA, INDEX, definition and long-value
roles. Overlapping records/bitmaps, conflicting owned/candidate claims or missing
windows refuse. Shared-byte masks are checked against original bytes before any
change. No map expansion or recursive split is attempted.

## Staging and proof

The existing numeric key preparation and leaf/root serializers construct the
split. The original root is retained, membership divides into the independently
specified children, and map claims cover the two actual EOF pages. Original
inactive root residue and all protected source bytes remain exact.

Independent proof separately derives record placement, original compressed key
membership, final sorted keys/ROWs, partition, root fence/children, histories and
map bits. It compares the complete image and freshly recaptures the final source;
it consumes no producer reservations, key arrays or patches.

TCNA4/V2 recovery retains original complete rows and BEFORE binding, reruns this
same admission/planning/proof and compares complete images before effects.
Pending mixtures roll back and truncate through the existing resizable journal.
Committed mismatches and forged intents/AFTERs refuse without effects.

## Observed checks and remaining work

The [source gate](../../testing/ACE16_NORMAL_ROOT_SPLIT_SOURCES.md) passed four
original/independent AFTER views before producer calls. Two unit functions pass
literal plans, direct proof and forgery controls. Actual V2 commits, mixed-target
recovery, exact public resource limits and measured cancellation checks pass.
Retained sessions also pass split INSERT followed by an empty DELETE, with fresh
snapshots, stale-handle refusal, primary cursor checks and namespace cleanup.

The supplied row includes present-empty Binary. No SQL/CLI split save is claimed
from a substituted NULL or an invented Binary literal. Compound DATA+INDEX
allocation, multiple physical splits, recursive growth, Binary splits and map
expansion remain work. Broader validation is recorded after the combined batch.
