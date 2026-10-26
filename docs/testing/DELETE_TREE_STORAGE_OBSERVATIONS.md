# Original multi-level DELETE storage observations

[DeleteTreeFixture.java](DeleteTreeFixture.java) creates fresh inert V2016/V2019
originals with public Jackcess builders and deletes selected IDs only on separate
copies using `Table.deleteRow`. The
[read-only capture](capture_delete_trees.py) reopens all ten files without write
access. Expression evaluation is off, linked targets throw, and UTC is explicit.
No stored/reference SQL, imported active content or Microsoft Access execution is
used. These original repository recipes use the reviewed Jackcess 5.0.4 JAR; its
provenance and all ten exact database hashes are in
[the fixture ledger](FIXTURES.md#original-ace16-multi-level-delete-reference-inputs).

The [compact reference summary](results/delete-tree-reference-2026-10-26.json)
records ten getter receipts, no failures and thirteen unchanged pins: producer,
capture, JAR, two originals and eight reference copies. Its SHA-256 is
`81c1f64f10014ea938fcc0badbe375dc8d8273917a62e70cb3902c24eb9999ef`.
It pins the unchanged full capture at
`.cache/tablecraft-fixtures/delete-trees-20261026/reference-verified.json`,
SHA-256 `bee3f4909d6bbf495d3fa82fdbe0df8a44cd9bf560a1608a66514ec8c2e5bb22`.
Per-row/entry arrays and full getter stdout remain ignored; public records keep
command statuses, transcript hashes/counts, boundary samples and structured checks.
The generation/deletion pass used eighteen Java commands and remains privately
preserved in `reference-initial.json` with its exact capture source snapshot.
Subsequent captures only read the same files; no original/reference was regenerated.
Fresh generation is not claimed byte-identical.

## Original topology and positive reference batches

Each Tree table has 1,000 rows in permuted physical order, a primary ID, nullable
and tied GroupKey, a separately permuted unique Score and a reversed/gapped
Part/GroupKey composite. GroupAsc and GroupTwin actually share physical index 1.
Three rows contain long Unicode Memo and 12 KiB OLE data; remaining rows have small
payloads. Unindexed and Sentinel tables remain separate preservation controls.

Both profiles have the same observed index topology and leaf populations, at
different physical page addresses. Each physical index has one node root of
height 1:

| Logical indexes | V2016 root page | V2019 root page | Leaf populations |
| --- | ---: | ---: | --- |
| PrimaryKey | 91 | 108 | 504, 496 |
| GroupAsc, GroupTwin | 92 | 109 | 244, 255, 501 |
| GroupDesc | 93 | 110 | 450, 273, 277 |
| Compound | 94 | 111 | 249, 250, 256, 245 |
| UniqueScore | 95 | 112 | 501, 499 |

Public index cursors establish the full row-reference encounter orders. The
independent decoder scans table-owned node/leaf headers, reconstructs each entry
from the shared prefix and end bitmap, follows explicit child/tail links, and
requires every stored separator to equal the complete final key **and row
reference** of its child subtree. It checks equal subtree heights and the full
leaf sibling chain. Complete root encounter orders match one physical declaration
each; aliases match that same declaration. Getter owned-page counts equal the
reconstructed graphs. Full raw entries and leaf partitions remain in the pinned
ignored capture. The public summary retains exact roots/heights/page counts,
node separator samples, per-leaf key/order digests and first/last boundaries;
it does not substitute an implementation-generated model.

The maxima batch removes IDs
`169,244,500,504,508,511,816,852,882,885,890,921,1000`. The tie-boundary batch removes
`244,508,511,852,882,885,921`. Both leave at least one entry in **every original
leaf across every physical index**, including indexes unrelated to the motivating
boundary. Reference reopens contain 987 and 993 Tree rows respectively. They retain
all original leaf partitions with exactly the surviving full key/reference bytes,
all original index pages, root/child/tail/sibling identities, heights and header
ownership/stamps. All surviving row/index encounter orders and Memo/OLE getter
values remain unchanged apart from removed rows. This establishes original
reference behavior; it does not itself execute or qualify a TableCraft candidate.

## Contraction observations remain separate

Deleting the 244 IDs in the smallest original GroupAsc leaf reduces that physical
index's owned-page count from 4 to 3; its logical alias reports the same reduction.
Other physical indexes retain their observed page counts. Delete-all reduces each
physical index to one owned empty leaf at its original root page: 91–95 for
V2016 and 108–112 for V2019. The raw scan finds no remaining table-owned
child/node headers.

All empty cursor orders coincide after delete-all, so the ordinary root-to-index
assignment by unique encounter order becomes ambiguous. The report preserves this
explicit diagnostic; it does not invent root identity from that order. Raw pages
at the original root numbers and the separate getter counts are observations.
This evidence does not qualify ownership/free-map retirement, branch contraction,
root-height reduction or an allocator. Those states remain separate from a
nonempty-original-leaf candidate profile. Native Access save/reopen, data macros,
ordinary write eligibility and durable apply remain unqualified by this work.

## Reproduction

Use a fresh existing directory and relative paths from the repository root:

```sh
for profile in V2016 V2019; do
  java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
    docs/testing/DeleteTreeFixture.java generate "FRESH_DIRECTORY/Tree-$profile-before.accdb" "$profile"
done
python3 docs/testing/capture_delete_trees.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs FRESH_DIRECTORY --report FRESH_GENERATION_REPORT.json
```

The capture derives the two safe batches and the smallest-leaf exhaustion batch
from the actual original partitions, creates four public-delete reference copies
per profile, then reopens them. No output name may already exist. The report
records actual commands and hashes. A later read-only recapture uses:

```sh
python3 docs/testing/capture_delete_trees.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs ORIGINAL_DIRECTORY --report FRESH_READ_ONLY_REPORT.json \
  --existing-references
```

Each Java command has a 120-second timeout. Generated files and raw captures remain
ignored; no third-party fixture or binary is bundled in this reference chunk.

## TableCraft candidate comparison

The [frozen comparison example](results/ace16-delete-trees-example-build-2026-10-26.json)
from committed revision `3b1cb8c7696312fccd95e635c5b21ed6993abf49` passes all
24 [candidate replay controls](results/ace16-delete-trees-originals-2026-10-26.json):
fourteen successful independent reopens and ten typed refusals with empty stdout.
The positives include both safe batches, individual non-tail/tail and equal-key
boundaries, and new boundary deletions from each existing safe reference history.
Leaf-exhaustion, delete-all, stale/duplicate and existing-output requests refuse.

The [runner](compare_delete_trees.py) verifies exact original leaf partitions,
surviving physical keys/references and every node's full separator maximum. All
root/child/tail/sibling pointers, heights, maps, historical counters, record bodies
and Memo/OLE data remain preserved under narrow patch spans. All registered input,
tool and reference hashes remain unchanged. Public records retain outcome and
command receipts; full getter transcripts and raw topology stay ignored and
hash-pinned. These checks qualify the explicit engineering profile, not native
Access semantics, tree contraction, ordinary write eligibility or durable apply.
