# Retained-NODE DELETE source qualification

Four literal source views now qualify a topology-preserving DELETE expectation
over the [800-row genuine NODE originals](GENUINE_NODE_ORIGINALS.md). Both
originals and both independently modeled AFTERs passed complete native reads,
ordinary Source qualification, retained owned-tree membership, all four aliases,
historical counters and full key+ROW fence checks before matching producers.
Producer acceptance was pending at source freeze; matching saves now pass in
the separate [write record](../../results/rows/ace16-normal-retained-node-delete-writes-2026-11-01.json).

The maintained [model](../../../scripts/compatibility/genuine_node_delete/model.py)
and [requests](../../../scripts/compatibility/genuine_node_delete/requests.json)
derive expectations exclusively from the pinned original bytes. They consume no
prior writer output or preview candidate. Input/request laws were frozen before
the model. Delete original IDs 302 and 354 (ordinals 301 and 353), in either written
order. The expected image is identical for both orders; TCND intent must still
retain the actual written request order.

| Physical index | Original child counts | Retained counts | Full left fence |
| --- | --- | --- | --- |
| Primary | 302 / 498 | 301 / 497 | Id302/ROW becomes Id301/ROW |
| Guard | 395 / 405 | 393 / 405 | Value3 stays; ROW354 becomes ROW347 |

Child order comes from the original graph, not ascending page numbers. Both
roots remain NODEs, all four leaves remain nonempty, and every original leaf
retains its own surviving membership. The model manually builds signed Int32,
ROW and child wires, prefix/bitmap/free counters and active payload. It copies
the complete original image and proves the unaffected byte complement. Only
selected logical directory words, the TD live count800→798 and fitting INDEX
framing/payload change. Historical counters800/7, raw records, inactive/vacated
tail, children, links, tail pointers, height, maps and EOF remain byte-exact.

The independent [report predictions](../../../scripts/compatibility/genuine_node_delete/reports.py)
retain the original complete inventory and derive changed PAGE/TD/DATA/physical
slot frames plus target row/cell ordinals from frozen bytes and literal requests.
Cursor predictions use original native ROW identities for ties. Twelve original
SDK observations are reused unchanged; twelve new AFTER observations passed on
their first attempts, followed by a zero-child saved verifier. The unchanged
complete observer, metadata oracle and four alias cursors use read-only opens;
no new compiler or Java tool change was needed. Closure pins remained unchanged.

The native source test checked all four views in one passing function (0.24s),
without invoking a planner. The [result](../../results/rows/ace16-genuine-retained-node-delete-source-2026-11-01.json)
pins complete images, preconstruction laws/code, twelve new child receipts and
the native log. It distributes no class/JAR binaries or machine paths. The
original-builder record is preserved separately at commit c60cdc4d.

For explicit source reproduction, supply the exact pinned originals:

```sh
python3 scripts/compatibility/genuine_node_delete/create_expected.py --originals ORIGINAL_DIRECTORY --output FRESH_SOURCE_DIRECTORY
```

The maintained wrapper reproduces all six historical image/proof artifacts
byte-for-byte without SDK or native calls. The qualified read reports remain
from the frozen private controller and exact model/request/report bytes.
The wrapper creates only independent testing source images and refuses existing
output directories. Its output is not a journal plan or save permission.
The ignored native source gate uses `TABLECRAFT_RETAINED_NODE_DELETE_SOURCE_DIR`
and literal pins in `normal_delete/node_delete_sources.rs`.

This record establishes source/read evidence. It does not establish producer
commit/recovery, empty-child retirement, index allocation, root growth, broader
tree forms or native Access save/reopen conformance. Those remain separate
acceptance work; no normal admission is inferred solely from this model.
