# Variable DATA and encoded index splitting

The feature-gated `NativeImage::preview_ace16_variable_split_updates` returns a
separate non-journalable candidate for ordinary variable Short Text/Binary changes
and qualified index growth in one operation. The existing slot, page, variable
and numeric split APIs retain their own contracts. Normal CLI/backend writes and
native Access saves are not enabled.

The selected field uses the existing ordinary variable storage profile, including
NULL, empty values and exact expected-value preconditions. Affected indexes must
be single-key General legacy, General modern or Binary indexes already qualified
by the encoded split service. Affected tuples, French/unknown collation, Memo/OLE,
ignore-NULL and other unqualified overflow/constraint profiles remain refused.
Required/unique rules, active-content blockers, relationships and complete final
constraints remain in force. Unchanged stored NULL peers retain their membership
and original bytes under the existing qualified index policy.

The operation first stages ordinary DATA changes, then transfers its single owned
append state into the index planner. One cursor, typed reservation inventory and
original-before/current-after map-byte ledger serve both phases. No cloned state,
rebased image, dummy numeric keys or synthetic widths are used. Owned key facts
outlive the DATA plan move while the builder prepares real replacement keys.
The independent final audit derives its keys again from original requests.

DATA placement retains the [ordinary-variable policy](ACE16_VARIABLE_UPDATES.md):
fit the original physical record when possible; otherwise examine the highest
hinted owned candidate once, then allocate represented EOF DATA when necessary.
Original logical ROW references stay stable. First forwarding clears the ordinary
root body and writes its pointer without reclaiming its allocated range.
Already-forwarded stub remainders and retired hidden records remain preserved.
Preferred record serialization, historical offsets, masks and stationary padding
follow the [request-derived DATA proof](ACE16_VARIABLE_DATA_PROOF.md).
The combined service additionally supports qualified
[inline hint/index window transitions](ACE16_ALLOCATION_TRANSITIONS.md), including
new ordinary candidate-map bitmaps. Ordinary owned/global allocation must still
be represented; older variable wrappers keep their original limits.

The new combined service also requires a complete original metadata census.
Canonical mutable owners include global free space, selected ordinary ownership
and hints, and each affected physical index owner; aliases are deduplicated by
actual physical identity. Missing, colliding or incomplete ownership declarations
refuse. All original table-definition chain pages and map storage/container pages
are protected from DATA writes. This additional gate belongs to the combined API;
it does not strengthen existing represented-only wrappers.

That immutable metadata guard applies to selected roots, final targets and noops.
It also checks the actual highest candidate before inspecting fit or clearing a
hint. A forbidden metadata candidate refuses rather than selecting a lower page
or inventing a failed fit. This guard does not consult original free/hint bits,
so legitimate staged EOF DATA can be reused. It grants no writable map-cell
permission. The checked definition-page visitor also remains a working ordinary
reader boundary with unchanged default grammar.

A shared composer checks original image length, exact-before patch overlap and
contiguous EOF rules. It copies DATA pages before index pages and writes the
composed map vector once. Ordinary ownership contains DATA only; index ownership
contains index pages only; candidate-map and index-map bitmap metadata belong to neither payload
set. Existing represented allocation and qualified index-map promotion continue
from the DATA phase boundary. Recursive index splitting retains its established
logical ROW, prefix, sibling, root and original/fresh residue policies.

Independent certification rebinds the original table, column, requests and full
metadata census. It reconstructs requested record bytes and derives DATA
placements, failed hints, page purposes and exact byte effects without staged
length authority. An opaque certificate borrows the exact original image and
candidate slice. Index replay starts from that certified EOF and map ledger,
rederives final keys from the original requests, and certifies the complete suffix
and final union. DATA reservations cannot become index membership during late
promotion. Source/candidate identity controls reject reuse of the certificate
against byte-identical but separately captured objects.

The [scoped implementation result](results/ace16-variable-index-splits-native-2026-10-27.json)
records 168 distinct runtime functions, three compile-fail doctests and scoped
strict/minimal/rustdoc/format checks. Twelve functions are added by the combined
feature; two checked-page visitor functions belong to the separately committed
helper. Literal Binary255 controls compare entire candidate images for both
header codes/directions, shared map bytes, promotion and late promotion. Private
forgeries cover DATA/index/bitmap signatures, correlated membership, phase-end
and certificate identity. The real engineering consumer also publishes and reads
a fresh Binary255 candidate while preserving its source.

The dedicated `preview_ace16_variable_split` example uses clap `Parser` and five
positionals: `SOURCE OUTPUT TABLE COLUMN TSV`. It reuses the immutable headerless
five-field quoted TSV parser and fresh synchronized publication helper. Existing
4096-request, 16 MiB input, 1024-byte value and shared 256 MiB/100k-row/100m-step
limits remain explicit. It captures all requests before opening the database,
never overwrites the source or an existing output, and cleans up only its own
failed partial output.

Authentic [source preparation](VARIABLE_ENCODED_SPLIT_SOURCE_OBSERVATIONS.md),
[old-variable baselines](VARIABLE_ENCODED_SPLIT_BASELINES.md) and
[public update references](VARIABLE_ENCODED_SPLIT_UPDATE_OBSERVATIONS.md) are
separate evidence. The [first six original-file max255 candidates](VARIABLE_INDEX_SPLIT_CANDIDATE_OBSERVATIONS.md)
now match the complete images frozen before this consumer existed, then pass six
fresh independent getter/root/map/tree proofs. All twelve first-attempt commands
succeed with original inputs, requests and budgets unchanged. Each adds one DATA
page and six index leaves; these originals do not exercise map promotion. The
implementation result above remains its earlier compiled-commit snapshot; the
original-file receipt records this separate replay.
Public scratch bytes are not engineering goldens. Additional proof work/storage
can tighten resource and cancellation boundaries. This is neither a performance
claim, global database-health certification nor native Access save qualification.
