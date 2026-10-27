# ACE 16 fixed Int32 root-leaf split previews

The optional `ace16-write-preview` feature supplies
`NativeImage::preview_ace16_split_int32_updates(table, column, updates, budget, cancelled)`.
It returns the existing separate `Ace16WritePreview`: original bytes, complete
candidate bytes and changed-row count. It does not produce a journalable edit
plan, execute required cascades or enable normal CLI/backend saves.

This is a working bounded allocation service, not general index splitting or
Microsoft Access/data-macro qualification. Existing fixed, nullable, slot, page
and variable preview APIs retain their contracts. ACE 16 compatibility remains
the overall target; internal/non-root split propagation is still implementation
work, not a permanent exclusion.

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

Other nonoverflowing qualified indexes use existing replacement machinery.
An overflowing internal node, non-root leaf, other codec/composite, or half that
still requires another split refuses atomically. UPDATE ignore-NULL blockers are
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
then checks the exact root/leaf key streams, row-inclusive bounds, order, links,
height, endpoint bitmap and free bytes. Newly appended pages have initialized
headers and zero unused residue. The original root keeps opaque bytes outside
its explicitly authorized header/bitmap/new entry regions.

Allocation requires existing represented cells for both new pages. Only the
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

Final selected verification passes 173 distinct runtime functions (153 existing,
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
