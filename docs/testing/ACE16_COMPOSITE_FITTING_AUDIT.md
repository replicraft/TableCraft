# Composite existing-fit framing certification

The [fixed numeric split preview](ACE16_FIXED_INDEX_SPLITS.md) now independently
certifies framing for affected, already-admitted composite indexes that retain
their original topology. This closes the composite fallback gap explicitly left
outside the [retained-planner correction](ACE16_RETAINED_SPLIT_PLANNER.md).
It adds no composite allocation, codec admission, public API or normal write path.

The audit resolves the actual numeric, mixed GUID or Decimal tuple profile from
immutable original schema/index metadata and prepares keys from the original
request-bound updates. It walks the original checked tree's leaf counts and
child/tail forms without allocating pages. Complete key plus ROW maxima determine
each original child separator. Exact expected encoded key/ROW/child bytes are
compared with the candidate's active representation; comparator equality alone
does not authorize different wire bytes.

Audit-owned measurement derives each page's prefix, bitmap and free space using
its immutable original prefix seed. The audit preserves original header fields,
links/tail and the suffix beyond the independently measured active end. It does
not trust the supplied patch inventory or candidate free space to enlarge opaque
byte authority, and does not call builder repacking or capacity helpers. Original
row-request authority, constraints, complete owned-page membership and map checks
remain in force. The extra preparation, encoding and reads are paid; identical
resource boundaries are not claimed.

The first causal run reproduced the gap on 200 equal Int32/Int32 tuples. The
original prefix was 13; changing Score from zero to one required prefix 9. A
forged prefix 8 page with correct keys, ROWs, bitmap and free space passed ordinary
row/index/constraint checks and was accepted by the old finalizer. Its expected
Preservation assertion failed. The preserved before run is a private forged-stage
certification counterexample, not an observed public-input corruption. The same
healthy control still succeeds, and the forged page now returns Preservation.

Four new private functions cover that counterexample; numeric, GUID and Decimal
companions under both header versions and directions; original bounded/tail
nodes, full ROW maxima, links and old residue; and stale/duplicate requests,
resource limits, immediate/measured late cancellation and exact retry. Fixture
bytes and existing numeric/composite goldens are unchanged. Shared tuple fixtures
are registered once for private read and mutation tests.

The [scoped result](results/ace16-composite-fitting-audit-native-2026-10-27.json)
records 62 distinct runtime functions, four new, and three compile-fail doctests.
Feature/default strict checks, minimal build and scoped formatting pass. It retains
the causal before result, test-only compilation errors and duplicate fixture lint
diagnostic separately. No database generation, JVM/public reference action or
original-file native candidate process ran in this chunk. Text/Binary slot-split
admission remains separate work; these checks do not establish native Access save
compatibility or general composite splitting.
