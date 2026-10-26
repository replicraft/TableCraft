# Original deeper DELETE contraction observations

These fresh originals and public Jackcess delete references establish observed
page-retirement behavior for the reviewed dependency. They do not execute a
TableCraft candidate, qualify native Access save/reopen, or enable ordinary writes.
The [compact reference summary](results/delete-contraction-reference-2026-10-26.json)
keeps hashes, receipts, boundaries and checks; full getter stdout and raw key/map
arrays remain in its hash-pinned ignored captures.

## Original recipes and provenance

[DeepDeleteFixture.java](DeepDeleteFixture.java) creates 500 rows with ten
SHA-256-derived GUID components, an Int32 primary key, a unique composite Wide
index and a real WideAlias sharing the physical index. Physical insertion order
is permuted. A separate Preserved table contains Unicode Memo and 12 KiB OLE.
[IndirectDeleteFixture.java](IndirectDeleteFixture.java) adds ten nonindexed
240-byte Binary values to each row to force a larger actual page population.
Both are original repository code under the root license; no upstream source or
third-party fixture was copied.

The reviewed Jackcess 5.0.4 JAR has SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Its provenance is already in [the fixture ledger](FIXTURES.md). The new ledger
entry lists all sixteen original/reference hashes. Originals and references remain
ignored under `.cache/delete-deep-contraction-next-20261026/`. They were never
regenerated for publication. Promoted Java recipes are byte-identical to the
actual private producers/probes. Historical generation receipts retain those
actual private command paths and hashes; fresh reproduction uses public paths.

Expressions are disabled and linked targets throw. Public `Table.deleteRow` runs
only on new copies. All getter reopens are read-only. No stored/reference SQL,
lookup expression or active imported content runs. Public rows/index cursors
establish populations, order and payloads. The separate
[DeepUsageMapProbe.java](DeepUsageMapProbe.java) and
[IndirectUsageMapProbe.java](IndirectUsageMapProbe.java) read protected root/map
metadata reflectively against this exact JAR; that introspection is not a stable
native format API. The public global membership method is intentionally unsupported
by the reviewed handler, so its protected BitSet is checked independently against
raw bitmap bytes. Ordinary map membership is additionally cross-checked through
the public getter.

## Actual topology and contrary height evidence

| Family/profile | Bytes | Physical pages | Primary/Wide roots | Wide height | Wide pages/leaves |
| --- | ---: | ---: | --- | ---: | --- |
| Deep V2016 | 704,512 | 172 | 91 / 92 | 2 | 45 / 40 |
| Deep V2019 | 774,144 | 189 | 108 / 109 | 2 | 45 / 40 |
| Indirect V2016 | 2,674,688 | 653 | 91 / 92 | 2 | 45 / 40 |
| Indirect V2019 | 2,744,320 | 670 | 108 / 109 | 2 | 45 / 40 |

Actual height was checked rather than inferred from row count. The Wide tree has
one root, four internal child nodes and forty leaves. Independent raw decoding
reconstructs complete prefix-compressed entries, checks each stored separator
against its child's full key **and row reference** maximum, and matches complete
encounter orders to the public index cursors. Reflection separately identifies
roots when empty orders coincide.

Each original has three reference copies:

| Selection | Removed/remaining rows | Retired index pages | Resulting Wide form |
| --- | --- | ---: | --- |
| Whole first root-child subtree | 127 / 373 | 11 | Height 2, three root children |
| Retain one original leaf | 491 / 9 | 42 | Height 2, one bounded child at each node |
| Delete all | 500 / 0 | 44 | Same root becomes empty height-0 leaf |

The nonempty nine-row tree **keeps its height**. It retains a root, one internal
node and one leaf; both nodes store one bounded-child separator entry and have
tail=0. It is not a tail-only form, and no nonempty root promotion is inferred.
The whole-subtree reference retires one internal node and its ten leaves. Delete-all
retires every original nonroot page and preserves the original physical root.
The alias continues sharing that root and ownership map.

## Existing inline and indirect maps

The unpadded Deep originals have inline global and Wide-owned maps. Both padded
originals already have indirect global maps at page 1/slot 0, using bitmap page
513. Their Wide-owned maps are also already indirect, using bitmap page 618
(V2016) or 635 (V2019). These cases exercise retirement inside existing map windows,
not creating or extending them.

Across all twelve reference deletions, global newly set in-file bits equal exactly
the removed index-owned pages. No index-owned page is added. Map representations,
pointer rows and bitmap headers remain unchanged. All table-owned and table
free-space memberships remain unchanged. Eighty-four complete inline payload or
indirect bitmap-page checks verify that only the exact identified membership bits
change. Counts of set bits beyond EOF remain separate from physical file members;
they do not establish allocation permission or file cardinality.

All retired pages begin with four zero bytes, but residual owner, sibling and
body bytes can remain. Deallocation is not erasure. Every complete original
record body, including GUIDs, Binary padding, column-count headers and masks,
remains identical; only selected slot words become tombstones. Historical index
counters and surviving getter payload/order remain unchanged. File lengths stay
fixed. This does not qualify allocation, page reuse, truncation, creating map
windows, data macros or durable apply.

## Request-file discriminator

The unpadded records are exactly 168 bytes; all-delete argv strings occupy
174,890 bytes including NULs. Padded records are exactly 2,593 bytes; their
corresponding strings occupy 2,599,890 bytes. Every row remains below the current
4,096-byte expected-record ceiling and every file below 64 MiB.

The measured host argument limit was 1,048,576 bytes. A dummy `/usr/bin/true`
consumer accepted the unpadded arguments and returned E2BIG for padded ones; no
candidate executable was used. Full padded batches therefore require the bounded
[request-file consumer](DELETE_REQUEST_FILES.md). Dropping requests to evade the
host limit would remove the intended acceptance case.

## Reproduction and capture chronology

From the repository root, use a fresh existing directory and relative paths:

```sh
for profile in V2016 V2019; do
  java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
    docs/testing/DeepDeleteFixture.java generate "FRESH_DIRECTORY/Deep-$profile-before.accdb" "$profile"
  java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
    docs/testing/IndirectDeleteFixture.java generate "FRESH_DIRECTORY/Indirect-$profile-before.accdb" "$profile"
done
python3 docs/testing/capture_delete_contraction.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar --family Deep \
  --inputs FRESH_DIRECTORY --report .cache/FRESH_DEEP_CAPTURE.json
python3 docs/testing/capture_delete_contraction.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar --family Indirect \
  --inputs FRESH_DIRECTORY --report .cache/FRESH_INDIRECT_CAPTURE.json
```

The capture derives selections from actual original topology, creates separate
reference copies, then reopens them. Output files/reports must be fresh. For an
existing original/reference corpus, add `--existing-references`; this performs
only reads. Each Java subprocess has a 120-second timeout. Expanded reports must
remain under ignored `.cache`; fresh databases are not promised byte-identical.

Historical private captures each recorded 22 commands, including reference
creation. Final publication captures each record sixteen read-only commands using
the promoted public sources and independently rehash all existing files. The compact
summary distinguishes these source/capture versions and pins the retained raw
reports rather than rewriting them. No candidate replay is included in this
reference chunk.

## Current candidate baseline: sixteen positive controls remain unmet

The separate [candidate baseline](results/ace16-delete-contraction-baseline-2026-10-26.json)
runs the committed frozen engineering example from revision
`a19cda688f6ea7660bfc8ca53852f5846fa3fd24`, SHA-256
`c807a655bce1ea49a6b4c5308bc287293e01d6039279589110f7cf8db85deda4`.
It is **partial evidence, not full contraction acceptance**. Of twenty required
positive controls, four succeed: deleting the remaining nine live rows from each
existing one-leaf reference history. Twelve expected stale/duplicate/existing-output
refusals also succeed. Sixteen larger positive cases return the typed
`Unsupported(IndexedColumn)` refusal. All source/input/request pins stay unchanged.

All 32 controls run; the actual 64 commands include only successful candidate
reopens. A fully successful run would have 96 commands because each additional
candidate requires two independent getter/probe reopens. Native/query allowances
and expected populations were not changed. Full padded batches use request files;
there is no argv workaround that omits selected rows.

The concrete frozen-source gate refuses any internal node with nonzero previous
or next links during contraction. The original Wide trees have complete reciprocal
internal chains, not just linked leaves:

| Family/profile | Wide root | Original height-1 node chain |
| --- | ---: | --- |
| Deep V2016 | 92 | `152 → 126 → 158 → 125` |
| Deep V2019 | 109 | `169 → 143 → 175 → 142` |
| Indirect V2016 | 92 | `584 → 370 → 622 → 369` |
| Indirect V2019 | 109 | `601 → 387 → 639 → 386` |

The reference subtree deletion removes the first node and repairs the next node's
previous pointer to zero; the remaining per-height chain stays reciprocal. Roots
have zero siblings. The nine-row reference has one node at each internal height,
so both sibling links are zero. These complete chains were independently checked
for all sixteen original/reference graph states.

Original Wide roots use bounded-final-child entries; their height-1 internals
store bounded entries plus a final tail child. The selected nine-row history
retains a non-tail original leaf, yielding a bounded singleton at each node with
tail zero. An initial verifier incorrectly assumed bounded source nodes everywhere.
That source and its raw run remain archived. The corrected verifier derives the
surviving original child sequence and full key/row maxima independently, accepts
either all bounded children or all-but-last bounded with the **exact last child**
as tail, and derives sibling chains separately per physical root and height.
All sixteen reference graph cases pass those model checks. It does not accept
arbitrary tail pointers or infer byte-identical node re-encoding.

A fresh same-binary rerun reproduces the sixteen typed production refusals and
verifies the four candidates and twelve negative controls again. Initial and
corrected raw reports, command logs, output files and exact driver snapshots stay
ignored with immutable hashes and source-path/archive mappings in the baseline.
The published driver subsequently only names header offsets/widths; its sixteen
model checks pass without claiming another native execution. Any future repaired
implementation requires a new frozen artifact and a separate acceptance record.
This baseline will not be rewritten into a passing result.
