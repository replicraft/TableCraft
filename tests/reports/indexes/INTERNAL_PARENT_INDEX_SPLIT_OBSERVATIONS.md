# Measured non-root internal-parent split

The two unchanged [963-row sparse sources](INTERNAL_PARENT_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
now have **two successful public-writer fixed MAX updates**, each showing a
non-root internal-parent split while retaining the height-two root. The frozen
prior engineering service refused both requests atomically with
`Unsupported(IndexedColumn)`. These are separate reference-action and refusal
observations, not passing new-kernel candidates or Microsoft Access execution.

Session 74237 completed with status 0 and exactly nine processes: one Java
compilation, two prior-service attempts, two public UPDATEs and four read-only
descriptions. No input was regenerated or retried. The
[compact receipt](results/ace16-internal-parent-split-actions-2026-10-27.json)
pins the original sources, copied public results, executed sources/classes,
getter outputs, raw report and command statuses. Requests retain ordinal 962,
`Key=149184 → Int32MAX`, and original references 1180:8 / 1197:8.

## Actual framing and preservation

| Measured result | V2016 | V2019 |
| --- | ---: | ---: |
| Retained root / height | 91 / 2 | 108 / 2 |
| Retained original internal RIGHT | 820 | 837 |
| Retained original leaf RIGHT | 100 | 117 |
| First new page: leaf LEFT | 1181 | 1198 |
| Second new page: internal LEFT | 1182 | 1199 |
| New internal LEFT tail | 823 | 840 |
| Original internal RIGHT tail | 100 | 117 |

The leaf divides 235/235. The overflowing internal parent's resulting 330 children
divide 165/165, with 164 complete key/ROW bounds and one tail per half. The
original internal page remains RIGHT; the new LEFT is inserted between its
original predecessor and that RIGHT. Both sibling directions are repaired at
each affected height. The root retains its ID, increases from two to three
bounded children and keeps tail zero. No index page retires and root height does
not grow. This is observed below-root behavior, not an inference from leaf splits.

All 963 original row references and markers remain unchanged. Every original
record body is identical except the intended selected four-byte LONG replacement;
the complete key-plus-ROW stream agrees with the public getter. The separate
sentinel remains unchanged and historical primary count stays 149,184.
Global-free/index-owned cells change for exactly two new index pages; ordinary
table-owned/free-space map membership does not change. Exact cells and full
key/ROW/separator/topology arrays remain pinned in the ignored raw report.

Public rewriting of page padding is an observation, not a future kernel golden.
A kernel candidate must independently preserve original root/RIGHT/predecessor
residual bytes and zero-initialize new unused bytes under its explicit contract.
No native save, SQL timing, map-growth or general mutation qualification follows
from these public `updateRow` actions.

## Read-only verification and source identity

The Java helper and [action model](internal_parent_split_originals/action_model.py)
are byte-identical promotions of the executed private sources. The
[new verifier](internal_parent_split_originals/verify_actions.py) redecodes both
cached before/after getters, maps and complete trees and repeats the row/record
comparison without calling JVM/native tools:

```sh
python3 docs/testing/internal_parent_split_originals/verify_actions.py \
  --output FRESH_OUTPUT.json
```

The raw first command receipts and source archives remain unchanged and ignored.
Public command examples are portable repository-relative renderings. The
prior-service artifact is the committed non-root-leaf consumer at revision
`d8bdf6c3ec015afdb70ffa4668ac10974cf7dbb6`; its exact admission refusal is retained
without borrowing earlier DELETE or no-op qualification.
