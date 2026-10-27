# ACE 16 repeated-overflow index split previews

The optional engineering [Int32 and exact fixed-scalar split services](ACE16_FIXED_INDEX_SPLITS.md)
now split a still-oversized half again. Their APIs, original-row request authority,
closed scalar carriers and complete before/final gates are unchanged. This is a
working allocation extension, not a normal save path or native Access/data-macro
qualification. The [implementation result](results/ace16-repeated-index-splits-native-2026-10-27.json)
retains exact scoped source/log pins and initial diagnostics.

## Declared deterministic policy

Original index nodes are processed in postorder. Each original leaf consumes
exactly its original entry count from the final sorted key-plus-ROW sequence;
repetition subdivides that range without consuming its neighbor's keys. Parent
construction concatenates every child's complete resulting sibling group and
rederives full row-inclusive maxima.

An overflowing page is divided using the existing floor(count/2) balanced rule.
The existing right page keeps its ID and a new left sibling is allocated at the
next represented EOF page. Normalization uses a stable page/allocation-order
worklist with at most one split of each page per pass. A new left item joins the
end and is visited in the same pass; an already visited retained right waits for
the next paid pass. Passes repeat until every page fits. Logical sibling order is
tracked separately from allocation/worklist order.

A root is nested by allocating its right page before its first left sibling.
An original leaf root retains its ID at height one with all non-tail child
maxima and a right tail, including when it ends with three or more leaves.
An overflowing internal root retains its ID at height plus one with bounded
children and no tail. New lower internal left nodes use the existing tail form;
a retained original right node keeps its original bounded/tail form. Source
neighbor links and complete parent key-plus-ROW bounds are maintained.

These rules retain all accepted ordinary two-leaf complete images. Newly admitted
multi-offender/bulk scheduling is an explicit engineering policy; the measured
single-offender public outcomes do not establish its general public/native
allocation-order parity. The [900-key reference](REPEATED_INDEX_SPLIT_OBSERVATIONS.md)
uses even partitions. Odd ceil(encoded-entry-count/2) versus floor(child-count/2)
and deeper fresh internal-tail/peer conventions remain separate qualification
questions, not silently inferred from those four copies.

## Origin, framing and map authority

A page's source origin stays fixed throughout normalization. Retained original
pages use their original prefix/framing and protect their original opaque residue.
An appended page remains fresh even when split again: its final prefix starts
from zero and its final unused bytes must be zero. Intermediate oversized payload
or earlier compression never becomes original-image residue authority.

Capacity and final framing use actual full key/ROW bytes and the owning codec.
The conservative key-only source capacity model proves overflow but is not an
exact fresh-prefix golden. A fresh raw page that already fits retains prefix zero;
otherwise actual common-prefix repair can include shared ROW bytes. Public
scratch/prefix choices remain distinct from the engineering byte policy.

The builder's arena, worklist and touched-bit list grant no permission. A separate
postorder audit independently reconstructs page passes, ranges/child groups,
exact EOF identities and map effects from original counts and request-derived
keys. Its own full key/ROW measurement computes capacity from original/fresh
prefix seeds; it does not call the builder's capacity predicate. The wire codec
is shared, while growth/topology decisions remain separate. It decodes final full streams, children, tails, heights, masks, prefixes,
free bytes and sibling links. Generic leaf-root certification explicitly checks
the original leaf-to-node kind transition. Every original unaffected index/page,
protected source prefix and unrelated global bit stays unchanged.

Allocation still requires existing represented index-owned/global cells. No map
growth, arbitrary in-file free-page reuse or retirement is added. Same-byte effects
from multiple physical owners are composed and rederived independently. Original
row references and historical counters stay fixed; real aliases do not allocate
twice. Immutable request/field/span/value replay and complete fresh catalog,
index membership, field and relationship checks remain additional final gates.

All new worklist/sibling storage, copies, capacity scans, splits, passes and audit
work are paid under the caller's cumulative allowances. Every split strictly
reduces a nonempty offending range. Empty/unsplittable inputs, depth/image ceilings,
unrepresented/nonfree future cells, allocation failures, exhausted pools or
cancellation prevent publication. Existing request/format/metadata preflight order
is preserved; no identical post-planning resource boundary is claimed.

## Focused acceptance

Independent complete-image literals cover repeated wide root, non-root and
internal-parent splits, both ACE codes/directions, stored NULL peers, aliases,
full ROW ties, old/fresh residue and exact map bits. The exact original 600-child
Int64/Currency MAX source now passes its independent repeated-parent image; its
historical refusal and the same-source value-1 positive remain documented.
Initial and late future-cell refusals, exact carriers/stale/duplicate requests,
all three budget pools and measured near-final cancellation/retry are included.

An authored 1,600-row two-offender case distinguishes same-pass append scheduling:
for each physical index, relative page order is `[4,3,1,2,5,0]`, with populations
`[200,200,400,400,200,200]`. Separate Byte/Int16/Int32 repeated cases measure each
width, including equality of old Int32 and fixed-facade complete output. These
synthetic full-ROW-compressed inputs are literal engineering controls, not public
fixture/native Access qualifications.

Private healthy-stage/forgery controls first establish that ordinary complete
index/constraint checks accept each altered image, then require operation
certification to reject it. Cases cover a fresh page's stale padding, an unused
future global bit and a correlated leaf-body/root-pointer/link permutation that
preserves complete semantic streams but violates the declared schedule. A fourth
control correlates a fitting value-1 request, field bytes and leaf wires while
retaining an unnecessary four-page growth; complete ordinary checks still pass,
but operation certification refuses it. Thus structural/final membership checks
do not replace byte/event authority.

The first feature library compile had an incorrect new NODE import path. A new
private fixture initially needed a test-only self-crate alias to reuse immutable
integration helpers; that alias is not an external dependency or runtime export.
The first authored collapsed fixture omitted future-free bits for discarded
alignment pages and correctly refused `Inspect(Malformed)`; only that authored
mask was corrected. A constant-chunk lint was fixed separately. Exact initial
sources/logs are preserved; none required an algorithm or byte-golden correction.

Root review identified that the first draft's topology replay still called the
builder's capacity predicate. A bounded synthetic fault experiment forced that
predicate false for a fitting 400-entry/value-1 operation. The correlated draft
certified an unnecessary four-page growth that passed ordinary full table/index
and constraint checks. The corrected audit-owned measurement rejected the same
fault with `Preservation`. The exact before/after source, logs and synthetic
images are preserved; all temporary fault/probe code was removed. This concerns
independence of required-growth certification, not observed row corruption or a
normal public-input exploit. A retained literal matrix checks full ROW compression,
original versus fresh seeds and exact fitting/overflow capacities.

The source/reference package records nine source and thirteen reference processes,
including four old typed-service refusals and four public repeated split successes.
Those actions, and the historical typed8 candidate receipt, are separate from this
implementation's runtime test totals. The subsequent
[first four-original replay](REPEATED_INDEX_SPLIT_UPDATE_OBSERVATIONS.md) now passes
all four exact candidate images and fresh typed/index/map inspections through a
new committed consumer, with eight successful first-run commands. That separate
receipt establishes candidate evidence for these 900-key originals; unit tests
and public writer observations alone do not establish it.
