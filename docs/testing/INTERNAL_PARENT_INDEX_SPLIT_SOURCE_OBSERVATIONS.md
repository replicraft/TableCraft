# Non-root internal-parent overflow sources

Both bounded public-builder experiments found a genuine **non-root internal
parent overflow witness** within the unchanged generation cap. Session 90573
completed on its first execution with status 0 and exactly nine source processes:
one Java compilation, two generations, two public sparse-copy prune histories
and four read-only descriptions. No MAX replacement, native admission or internal
split action was executed. The [compact receipt](results/ace16-internal-parent-split-sources-2026-10-27.json)
pins sources, classes, commands, selections, raw proofs and immutable database
histories. These observations do not qualify Microsoft Access behavior.

## Measured source geometry

Each profile naturally reached its first gate at **149,184 target rows plus one
sentinel**, below the unchanged 150,000 aggregate generation cap. The selected
rightmost leaf contains 470 rows. Keeping that entire leaf and every other
leaf's maximum on a fresh public-writer copy leaves **963 target rows plus the
sentinel**. The larger generation is not an engine-admitted mutation source.
The sparse source remains within the existing 16,384-row per-table ceiling;
actual mutation admission has not been attempted.

| Source fact | V2016 | V2019 |
| --- | ---: | ---: |
| Root ID / height | 91 / 2 | 108 / 2 |
| Distinct overflowing parent ID / height | 820 / 1 | 837 / 1 |
| Selected rightmost leaf | 100 | 117 |
| Selected key / native ordinal | 149184 / 962 | 149184 / 962 |
| Selected page:slot | 1180:8 | 1197:8 |
| Leaf / height-one / root page counts | 494 / 2 / 1 | 494 / 2 / 1 |
| Complete index-owned / data-owned pages | 497 / 586 | 497 / 586 |

For the fixed future `149184 → Int32MAX` replacement, independent complete
key-plus-ROW reconstruction gives a best-prefix leaf size of **3,761 bytes**,
exceeding the 3,616-byte entry area. Balanced 235/235 halves require 1,413 and
1,881 bytes. The distinct height-one parent would require **3,621 bytes**, also
exceeding capacity. This parent is not the root; no internal-page retention,
allocation order or root change is inferred from earlier leaf/root observations.

Public pruning retained all 497 original index pages, every internal child and
tail, every full key/ROW bound, all subtree maxima and the complete ordered
sibling chains at heights zero, one and two. The target leaf's full entry stream,
all kept physical row references/markers and the separate sentinel are unchanged.
No merge, redistribution, page retirement or header/flag forcing was used.
Future map cells are recorded as geometry only, not allocation qualification.

## Frozen bounds and provenance

The new repository-owned helper adapts the existing sparse-source producer with
a separate class and output paths. It uses the reviewed Jackcess 5.0.4 public
builders and cursor deletion, with expressions disabled, explicit UTC and linked
opens refused. Raw access only inspects bytes. Bounds stayed at 64 MiB per file,
512 MiB heap, depth 16, checkpoints every 32 inserts and 1,200 seconds per child.
A no-witness outcome would have preserved partial files and failed explicitly;
no alternate witness, additional rows or adjusted replacement was allowed.

The first static manifest construction accidentally applied a text newline check
to the binary JAR. That pre-action authoring failure and correction are pinned;
it caused no compilation or database action and is separate from the first
actual nine-process execution.

## Read-only checks and fresh reproduction

The [verifier](internal_parent_split_originals/verify_existing.py) recomputes both
larger/sparse certificates and complete map/child/sibling/retained-row facts from
cached descriptions, without JVM or native calls:

```sh
python3 docs/testing/internal_parent_split_originals/verify_existing.py \
  --output FRESH_OUTPUT.json
```

The Java helper is byte-identical to the executed private source. The certificate
has only its repository-root adjustment; the maintained capture wrapper and new
verifier are explicitly distinct from historical execution. Fresh reproduction
requires a current-source manifest and fresh outputs:

```sh
python3 docs/testing/internal_parent_split_originals/capture.py \
  --freeze-current .cache/fresh-internal-parent-tools.json
python3 docs/testing/internal_parent_split_originals/capture.py \
  --freeze .cache/fresh-internal-parent-tools.json --execute \
  --workdir .cache/fresh-internal-parent-sources
```

These commands describe new source generation, not historical hash restoration
or permission to perform a MAX update. Public commands are portable renderings;
raw expanded getter/tree arrays and exact executed command receipts stay ignored
and pinned. All accepted originals and earlier root-growth evidence remain intact.
