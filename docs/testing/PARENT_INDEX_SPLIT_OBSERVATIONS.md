# Int32 parent overflow and retained-root growth observations

The first frozen action phase completed **nine fixture processes**: two atomic
`Unsupported(IndexedColumn)` refusals from the current engineering consumer and
two successful public Jackcess UPDATEs with actual root growth from height 1 to
2. Both original root IDs remain unchanged. These are public-writer/native-file
observations, not Microsoft Access qualification or a successful parent-splitting
kernel implementation.

The [compact receipt](results/ace16-parent-index-split-originals-2026-10-27.json)
pins the exact first execution, sources, commands, complete raw reports and
subsequent read-only framing checks. The fixed inputs are the two unchanged
782-row [sparse sources](PARENT_INDEX_SPLIT_SOURCE_OBSERVATIONS.md), not their
99,424-row producer ancestors. The request is `SparseInt32.Key`, live ordinal
781, expected 99,424, replacement Int32MAX. The physical row references remain
816:228 and 833:228; Marker remains 298,272.

The current consumer is pinned to committed revision
`d8bdf6c3ec015afdb70ffa4668ac10974cf7dbb6` and artifact SHA256
`d9facf656f8cb51e5e235223b5532ee64624695e0f1a165fee3b8c6a4ec614c3`.
Its 153 selected historical Git source objects and exact binary were checked
before execution. Both attempts emit the exact typed refusal, empty stdout and
no output candidate. There are no retries, changed caps or alternate selectors.

## Actual active geometry

The independent source gate measured the updated unsplit leaf at 3,633 bytes and
the parent at 3,621 bytes, each exceeding the 3,616-byte entry area even with the
best complete key/ROW prefix. The successful public calls establish the resulting
physical growth, rather than inferring it from those predictions.

| Measured result | V2016 | V2019 |
| --- | ---: | ---: |
| Retained root, height 1 → 2 | 91 | 108 |
| Retained right leaf, 227 entries | 100 | 117 |
| First allocation: new left leaf, 227 entries | 818 | 835 |
| Second allocation: new right internal node | 819 | 836 |
| Third allocation: new left internal node | 820 | 837 |
| Left internal tail | 459 | 476 |
| Right internal tail | 100 | 117 |
| Original / resulting file pages | 818 / 821 | 835 / 838 |

The original parent had 328 bounded entries and one tail, hence 329 children.
Leaf insertion makes 330 children. The two new height-one nodes each have 165
children: 164 bounded complete key/ROW maxima and one tail. Their sibling chain
is new-left → new-right, with reciprocal links and zero outer endpoints. Both
new nodes use prefix zero and have 1,484 free bytes.

The retained root becomes a height-two node with **two bounded maxima and
`tail=0`**, not the one-bound/right-tail form of the earlier root-leaf split.
Its left bound is Key 49,830 plus that row's full physical reference; its right
bound is Int32MAX plus the original selected row reference. Root prefix is zero,
free space is 3,590 bytes, owner/stamp remain qualified, and root siblings remain
zero. The existing predecessor leaf links to the new left leaf.

All 782 typed rows and the full final key/physical-row stream match the exact
replacement. All live record bodies and references remain unchanged apart from
the selected four Key bytes. A separate whole-page check proves that the selected
data page has exactly those four changed bytes. Marker, sentinel, declared row
count and historical index counter 99,424 remain unchanged.

Exactly three EOF pages are added; none is retired. The global future-free and
index-owned changes compose in one byte per map:

| Map byte | V2016 | V2019 |
| --- | --- | --- |
| Global future-free | offset 2,101,354: `fc → e0` | offset 2,101,356: `f8 → c0` |
| Index-owned | offset 2,478,186: `02 → 1e` | offset 2,547,820: `05 → 3d` |

Ordinary data-owned and candidate-hint maps remain unchanged. Raw captures retain
all node/leaf entries, links, prefixes, masks, free counts, old/new roots, complete
map payloads and counters. The source files and their larger/selection/no-op
ancestors remain immutable.

## Scratch bytes are separate from active structure

The public writer rewrites unused storage. After the new root's active end at
506, 3,351/3,353 old bytes differ. After the retained leaf's active end at 2,523,
1,444/1,445 bytes differ. Even the predecessor's unchanged one-key stream rewrites
1,780 page bytes: its next link and unrelated unused residue. Newly allocated
pages also contain nonzero unused suffixes: 1,272 bytes in the new leaf and 1,203
in each lower node.

These scratch bytes are observations, **not kernel byte goldens**. A future
engineering implementation retains original root/right/predecessor opaque
residue and initializes fresh unused bytes to zero. It must separately prove
active key/ROW/child geometry, sibling chains, represented map events and field
patch authority. This stronger preservation policy does not claim byte-for-byte
parity with public canonicalization or normal native save behavior.

## Source identity and reproduction

The [public verifier](parent_index_split_originals/actions/verify_existing.py)
and framing tools recheck saved artifacts without launching a JVM or consumer:

```sh
python3 docs/testing/parent_index_split_originals/actions/verify_existing.py
python3 docs/testing/parent_index_split_originals/actions/framing.py \
  --verify-existing \
  --output .cache/ace16-parent-split-actions-next-20261027/framing.json
python3 docs/testing/parent_index_split_originals/actions/fresh_framing.py \
  --verify-existing \
  --output .cache/ace16-parent-split-actions-next-20261027/fresh-framing.json
```

The action helper, comparison model and plan are byte-identical promotions.
The public capture/framing wrappers adjust paths and expose explicit current-tool
manifests and saved-proof arguments; they are distinct from the ignored sources
actually executed. Their extra current-getter class checks are new guard checks,
not falsely attributed to the historical run. The accepted getter classes and
reviewed JAR remain pinned.

Explicit fresh reproduction uses the fixed accepted inputs and a current manifest;
it does not regenerate sources or authorize normal database saves:

```sh
python3 docs/testing/parent_index_split_originals/actions/capture.py \
  --freeze-current .cache/fresh-parent-action-tools.json
python3 docs/testing/parent_index_split_originals/actions/capture.py \
  --execute --freeze .cache/fresh-parent-action-tools.json \
  --workdir .cache/fresh-parent-actions
```

Every public action uses a fresh bounded copy, actual immutable source hash,
selected ordinal/reference/Marker, required unique metadata, enabled foreign-key
enforcement, disabled expressions, UTC and a throwing link resolver. The setter
is one public `updateRow` call, not an observation of native SQL statement timing.
