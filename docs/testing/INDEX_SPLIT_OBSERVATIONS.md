# Int32 root-leaf split observations

Two frozen old-service requests refused with `Unsupported(IndexedColumn)` and
created no candidate. Two matching public Jackcess UPDATE calls succeeded.
The [receipt](results/ace16-index-split-originals-2026-10-26.json) records all
12 commands and preserves the initial verifier's exit 1. Its failed hypothesis
was byte identity of unaffected indexes, not failure of either public UPDATE.
A separate read-only diagnosis completed successfully without repeating actions.

These are observations for the next engineering split implementation. No new
TableCraft split API is implemented by this evidence, and the public Java writer
is not a Microsoft Access or SQL statement-timing oracle.

## Original inputs and exact request

The two unchanged registered BatchDelete originals use ACE16 V2016 and V2019.
The selected record is `Children.ID=1`; its independent `ParentID=1` and all
GUID fields remain unchanged. The sole enforced relationship is
`Parents.ID -> Children.ParentID`, with RESTRICT updates/deletes. This request
therefore changes neither endpoint's relationship key value or association.

The [frozen plan](index_split_originals/plan.json) requests exactly one fixed
Int32 replacement per profile: original ordinal 0, expected 1, new 2147483647.
It is byte-identical to the pre-action plan, SHA256
`bffbf7831ae1911fdfb3b3b5b583f27c59bd08757e8b48bce0c6b99585a25006`.
The public helper copies each original to a fresh output, disables expressions,
enforces foreign keys, selects UTC and rejects linked opens. It checks actual
empty table/column property maps and the required unique primary metadata before
the update. Read-only full getters independently check all rows and index orders.

The existing [fixed Int32 consumer](../../crates/engines/tablecraft-ace/examples/preview_ace16_update.rs)
was frozen from revision `2fc75f6f147df1ae7e715a79ef400968185ab304`.
Its [build receipt](results/ace16-index-split-example-build-2026-10-26.json)
pins the 18,074,152-byte artifact, SHA256
`28bfe8b1db749eebf1da2ed2ca266aed1bc0d317393d567504478006fc8bd7b5`,
and 108 selected historical Git source objects. Both actual old-service failures
occurred before any output file existed; source hashes remained unchanged.
No gate or eligibility rule was weakened to force the request through.

## Capacity and actual public growth

Each original Children primary index is a root leaf with 500 keys, a three-byte
common key prefix and 613 free entry bytes. Independent decoding matched every
literal nullable Int32 key and original physical row reference.
Replacing ID1 with Int32MAX changes the common prefix from three bytes to one:

| Entry budget | Bytes |
| --- | ---: |
| Original `500*9 - 499*3` | 3003 |
| Replacement `500*9 - 499*1` | 4001 |
| Page entry capacity | 3616 |

Thus valid common-prefix compression cannot retain the final keys in one page.
The current replacement planner retains original leaf cardinalities/topology and
its serializer explicitly refuses a required split.

The actual public outputs keep the original root ID and convert it from a leaf
at height 0 to a node at height 1. Two new EOF leaves each contain 250 entries:

| Profile | Retained root | Left leaf | Right leaf | File pages |
| --- | ---: | ---: | ---: | --- |
| V2016 | 95 | 242 | 241 | 241 -> 243 |
| V2019 | 112 | 259 | 258 | 258 -> 260 |

The right leaf was allocated first. Full separator keys include the physical row
reference and were checked against actual child maxima. The final primary
sequence is IDs 2..500, then 2147483647, using the same 500 original row references.
No live row count or historical index counter changes.

Only primary-index ownership and global allocation map bytes change:

| Profile | Index-owned byte | Global-free byte |
| --- | --- | --- |
| V2016 | offset 388936: `00 -> 0c` | offset 8158: `fe -> f8` |
| V2019 | offset 458568: `00 -> 0c` | offset 8160: `fc -> f0` |

All other map memberships and bytes remain unchanged, including table data-owned
and free-space hints. The selected record changes exactly its four fixed ID bytes;
all other raw row bodies, including the Memo/OLE sentinel, remain unchanged.

## Preserved initial hypothesis failure

The first capture's 12 commands completed, including both native refusals and
both successful public actions. Verification then failed on its expectation that
unaffected index pages would remain byte-identical. The initial report, source,
assertion and exit 1 remain immutable; they are not relabeled a passing capture.

The full `Children.Wide` key-plus-row stream is unchanged, as are its page set,
root and height. Nevertheless, the public writer redistributes one existing entry
between two leaves and updates their parent separator. V2016 pages 105/197 change
from 12/13 entries to 13/12; parent 166 changes. V2019 has the corresponding
pages 122/214 and parent 183. The other three physical indexes remain byte-identical.
These physical rewrites do not change GUID values, association keys or row references.

The separate read-only diagnosis redecodes every raw getter, map and tree, verifies
both final primary sequences and all child maxima, and classifies these rewrites
explicitly. A future kernel may preserve unaffected Wide bytes more strictly;
the public writer's redistribution is not its raw-byte golden.

## Tools and provenance

Exact executed private sources are archived and pinned separately from the
current public [capture](index_split_originals/capture.py),
[action helper](index_split_originals/IndexSplitReference.java) and
[analysis](index_split_originals/analyze.py). The public capture records physical
rewrites rather than repeating the false byte-identity expectation. That refactor
was not the historically executed program. The public analysis was exercised
read-only on the preserved initial artifacts, with no new native or Java calls:

```sh
python3 docs/testing/index_split_originals/analyze.py \
  --report .cache/ace16-index-split-original-actions-initial-20261026/report.json \
  --output .cache/index-split-fresh-read-only-diagnosis.json
```

Running the public capture without `--execute` verifies plan/input pins only.
Fresh action reproduction requires explicit artifact/hash/revision/build receipt,
a fresh work directory and the reviewed JAR. It never regenerates an original.
