# ACE 16 fixed Int32 index split previews

The optional `ace16-write-preview` feature supplies
`NativeImage::preview_ace16_split_int32_updates(table, column, updates, budget, cancelled)`.
It returns the existing separate `Ace16WritePreview`: original bytes, complete
candidate bytes and changed-row count. It does not produce a journalable edit
plan, execute required cascades or enable normal CLI/backend saves.

This is a working bounded allocation service, not general index splitting or
Microsoft Access/data-macro qualification. Existing fixed, nullable, slot, page
and variable preview APIs retain their contracts. ACE 16 compatibility remains the overall target; this exact codec and allocation
profile does not qualify every index split.

## Admission and exact operation

Only the existing plain, unencoded, subversion-zero ACE 16 2016/2019 preview
profiles are admitted. The selected field must be a qualified fixed non-NULL
Int32 target under the original expected-value/row-mask/storage checks. The
`Int32Update` slice addresses original live-row ordinals in this snapshot;
ordinals are not durable identities. Duplicate, stale, missing, wrong-carrier,
field-rule, unknown-property, opaque-content and unqualified index/relationship
cases remain blocked. All requests share one cumulative caller budget.

Existing logical/physical index declarations, constraints and foreign checks
must qualify completely before planning. Every physical declaration has a
logical owner; real aliases share one physical edit. Original row references
remain stable. Final uniqueness is checked on the complete final key set rather
than publishing successive intermediate changes.

A single-Int32 original root leaf can be recompressed to fit. If the best valid
common prefix still overflows, its final sorted key-plus-row sequence is divided
at the middle entry count. Both nonempty halves must fit independently. The
service allocates exactly two represented EOF index pages, right first and left
second, and retains the original root ID as a height-one node. For 500 entries,
each leaf contains 250 entries. Leaf siblings are reciprocal with zero external
end links; the root stores the complete left maximum **key and row reference**,
left child pointer and right tail.

For an existing non-root leaf, the service first consumes the globally sorted
final key sequence using each original leaf's entry count. An overflowing leaf
is divided at the middle entry count: one new **left** leaf is allocated at EOF,
and the original page retains the right half. This preserves the observed
new-left/retained-right geometry. Neighbor links are repaired across parent
boundaries; every parent separator uses the complete resulting subtree maximum
key and row reference. Existing bounded-child versus right-tail node forms remain unchanged when the
node fits. Multiple leaf allocations follow source leaf traversal and
physical-index order; logical aliases do not duplicate physical work.

If complete updated child maxima cannot fit an internal node even with its best
valid prefix, its final child sequence is divided at the middle child count.
For a non-root node, one new left node is allocated and the original node retains
the right half and its original bounded/tail form. The new left node uses a right
tail. The separate [below-root observations](INTERNAL_PARENT_INDEX_SPLIT_OBSERVATIONS.md)
retain the public writer's matching new-left/retained-right geometry at height
one below a height-two root. That reference behavior and the literal engineering
model remain distinct from a candidate replay or Microsoft Access qualification.

For an overflowing internal root, the service allocates the right node and then
the left node, after descendant allocations. Both new lower nodes use right
tails. The original root ID remains fixed at height plus one, with **two bounded
maxima and no tail**. Child maxima include full row references, even when numeric
keys tie. Reciprocal sibling chains are rederived for every final height.
The unchanged root-leaf split described above still uses its original single
bound/right-tail form. An empty non-root leaf, depth beyond the existing index
ceiling, or a half that still cannot fit refuses atomically.

Other nonoverflowing qualified indexes use existing replacement machinery.
An unqualified codec/composite refuses atomically. UPDATE ignore-NULL blockers are
unchanged. Physical read support does not grant new mutation-codec authority.

## Source authority and preservation

A private immutable operation binding retains the original table, selected
column and requests through staging and finalization. The finalizer rebinds
metadata from the frozen original, replays actual row/field mask/span/expected
value checks, and compares the complete physical patch inventory. Extra,
missing, wrong-row and wrong-value patches cannot grant themselves permission.
Audit keys derive solely from this replay, not a supplied mutable key vector.

Physical split logic belongs to the index subsystem. Its independent audit
reconstructs required splits from original keys and actual replacement capacity,
then checks the exact root/node/leaf key streams, row-inclusive bounds, order,
per-height links, height, endpoint bitmap and free bytes. Recursive allocation
authority is rederived from original leaf counts, final child counts and actual
replacement capacity, independently of the builder arena and planner-supplied
page patches. Newly appended pages have initialized
headers and zero unused residue. The original root keeps opaque bytes outside
its explicitly authorized header/bitmap/new entry regions.

Allocation requires existing represented cells for every new index page. Only the
selected physical index-owned bits are set and global future-free bits cleared;
table data-owned and candidate-hint maps remain unchanged. Map-role/storage
ownership is checked. Expected byte effects are rederived from source descriptors
and split facts, including multiple bits in one byte; planner masks do not grant
authority. Map growth and arbitrary in-file free-page reuse remain unsupported.

All unaffected index pages remain byte-identical. Existing affected pages retain
protected framing and old unused residue. The source prefix is checked outside
qualified row/index/map changes; appended coverage must be contiguous and exact.
The grown header/catalog, complete table/index membership, field rules and
relationships are checked again before returning a candidate. Counters and
original physical row references remain unchanged. Cancellation and all resource
pools can prevent publication, including at the final boundary.

The [public reference observations](INDEX_SPLIT_OBSERVATIONS.md) establish the
500-key prefix-loss refusal and matching public splits. Their public writer also
redistributed an unchanged Wide index. That physical rewrite is not this
service's golden: the engineering contract deliberately protects original
unaffected Wide bytes. Those observations remain separate from candidate replay
against a committed build of the new service.

The separate [non-root observations](NONROOT_INDEX_SPLIT_OBSERVATIONS.md) retain
two earlier atomic refusals and two public update successes. Their first verifier
exited 1 because the public writer also rewrote unchanged Memo/OLE references;
read-only diagnosis verifies the complete values and actual split geometry.
The engineering service preserves those original descriptors/chains and adds
only the required index page. Public reserialization is not its byte golden.

The [parent source observations](PARENT_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
separate bounded large generation from the sparse, fully qualified 782-row
source. The [parent update observations](PARENT_INDEX_SPLIT_OBSERVATIONS.md)
retain two old-service refusals and two public root-growth successes. Their
new-left leaf and right-then-left internal allocation, two-bound root and complete
subtree maxima inform this profile. Public scratch rewrites remain observations;
the engineering candidate preserves original residue and zeroes fresh padding.
No parent-growth candidate comparison is established by those public actions.

## Engineering consumer

The feature-gated [example](../../crates/engines/tablecraft-ace/examples/preview_ace16_split.rs)
accepts seven positional arguments. It buffers the complete candidate before
creating a fresh output, synchronizes publication and cleans up only its own
failed partial file. It never writes the source or replaces an existing output.
Its operation allowance is 256 MiB, 100,000 rows and 100 million steps.

```sh
cargo run --locked --offline -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_split -- \
  before.accdb fresh-candidate.accdb Children ID 0 1 2147483647
```

The warning and return type remain explicit engineering boundaries. A fresh copy
is not a qualified native save, locking/recovery guarantee or data-macro result.

## Verification

The original root-leaf slice passed 173 distinct runtime functions (153 existing,
15 primary and five private), three compile-fail docs, scoped strict lint,
standalone minimal compilation, warnings-denied all-feature rustdoc and formatting.
The [scoped result record](results/ace16-index-splits-native-2026-10-27.json)
separates final focused checks from earlier diagnostics and mechanical cleanup.
It includes independent literal source/key goldens, both admitted ACE codes and
index directions, real aliases, row-tie boundaries, two physical owners sharing
one global bitmap byte, nonoverflowing NULL peers, field/unique constraints,
future-cell refusal, quotas/cancellation and retry. The consumer exercises real
candidate creation and fresh publication.

A private forged-stage probe demonstrated an extra unindexed-field patch passing
before the request-bound proof. It was a certification gap, not a demonstrated
public-input corruption. Exact before sources/logs remain preserved; corrected
extra/missing/wrong-row/wrong-value controls pass. Separate harness compile
failures and the mode-specific malformed-versus-unsupported map expectation are
retained and excluded from final passing counts. No model bytes were changed to
match a candidate.

The [non-root implementation record](results/ace16-nonroot-index-splits-native-2026-10-27.json)
reports 100 distinct runtime functions: 70 existing regressions, 23 primary
functions and seven private controls. Ten functions are new in this slice.
Three compile-fail docs, scoped strict lint, standalone minimal compilation,
warnings-denied rustdoc and formatting also pass. The original root-leaf result
record remains unchanged. The new literal tests cover
first/middle/last leaf insertions, both directions and node forms, full row-tie
bounds, aliases and physical owners sharing map bytes, nonoverflowing NULL
peers, explicit parent-capacity refusal, quotas/cancellation and retry. Private
forgeries independently check neighbor links, row-inclusive parent bounds,
original residual bytes, fresh-page residue and unused future-free bits.

The first authored non-root split returned `Preservation`: its builder had
changed temporary header links but used the existing repack patch emitter, which
intentionally excludes those header fields. The independent after-audit caught
the missing emitted changes. Explicit paid four-byte link patches fixed the
production draft; source/log evidence is retained. This was not an oracle
correction or a published corrupt candidate.


The [recursive implementation record](results/ace16-parent-index-splits-native-2026-10-27.json)
reports 108 distinct runtime functions: 70 existing regressions, 29 primary
functions and nine private controls. Eight functions are newly added. Three
compile-fail docs, scoped strict lint, standalone minimal compilation,
warnings-denied all-feature rustdoc and formatting also pass. Earlier result
records remain unchanged; the exact old parent-capacity refusal now has a
positive retained-root growth control without changing its source image.

Independent whole-image cases cover both ACE codes and index directions,
retained bounded/tail forms, full row-tie bounds, every final sibling height,
NULL peers, alias deduplication, exact future-map refusal, original opaque
residue, fresh zero padding, stale/duplicate requests, cumulative limits and
late cancellation/retry. Separate author cases check root growth and complete
images for both headers. Private healthy-stage/forgery controls additionally
exercise parent-only overflow without a leaf split and reject supplied changes
to root bounds, height, tail, old residue, fresh internal links/padding and
unused future-free bits.

The first independent tie case passed its full-image assertion but its intended
equal-key boundary was not actually paired: the input produced 150/151. The
fixture owner corrected only that input pairing to produce 150/150 and retained
the original source and failing log. A subsequent test-only clone lint was also
preserved and corrected. Neither diagnostic required a production or candidate
byte-golden change. Actual original-file candidate comparison requires its own
committed build and separate replay receipt; these implementation tests do not
claim one.
