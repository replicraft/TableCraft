# Indirect column-LVAL map source observations

Two fresh public-builder ACE 16 originals now demonstrate an indirect Memo owned
map with an inline candidate map. The fixed construction and read-only capture
completed all thirteen fixture commands on their first attempt. No native
candidate or normal save was attempted. The
[compact receipt](results/indirect-long-value-map-observations-2026-10-27.json)
pins the actual sources, snapshots, raw report and saved-proof verification.

These are independent-writer storage observations. They do not qualify Microsoft
Access behavior, indirect candidate maps, map growth or replacement previews.
Existing raw60 and compressed148 candidate acceptance remain separate evidence.

## Fixed public construction

The repository-authored
[producer and getter](indirect_lval_observations/IndirectLongValueProbe.java)
uses the previously reviewed Jackcess 5.0.4 binary. Both V2016 and V2019 have
`IndirectLongValues` with ID LONG, uncompressed Memo and opaque OLE. The primary
ID index and two ordinary ascending declarations yield two physical roots;
`IDAsc` and `IDTwin` actually share physical index 1.

Each profile follows the same immutable recipe:

1. Insert ID1 with 8,192 U+0100 Memo units and OLE bytes `00..ff`.
2. Insert exactly 700 rows in a separate unindexed padding table, each ID plus
   seven uncompressed Short Text fields containing 255 U+0100 units.
3. Insert ID2 with 8,192 U+0101 Memo units and OLE bytes `ff..00`.

Each writer phase closes before its checkpoint is copied and opened read-only.
One compiler command plus three construction and three getter commands per
profile produced thirteen commands. Every command used a 512 MiB JVM/compiler
heap and a 30-second timeout. Files remained below the fixed 8 MiB ceiling;
there are exactly 702 user rows and each Memo payload is 16,384 bytes. Expression
evaluation is disabled and a refusing link resolver prevents linked opens.

No fields, headers, map bits or descriptors were patched. There were no retries,
geometry adjustments, changed selectors or failed commands. All earlier
checkpoints and frozen input pins remain unchanged.

## Actual stored geometry

| Observation | V2016 | V2019 |
| --- | --- | --- |
| Final file pages | 811 | 828 |
| Memo owned map container/slot | 90:4 | 107:4 |
| Memo owned indirect bitmap | 806 | 823 |
| Memo owned pages | 10 | 10 |
| Lowest-to-highest owned span | 716 pages | 716 pages |
| Memo candidate map container/slot | 90:5 | 107:5 |
| Candidate inline base | 800 | 816 |
| Sole candidate page | 810 | 827 |
| Global free-map bitmap | 513 | 513 |

All 700 padding rows occupy distinct ordinary pages, and every physical padding
record is exactly 3,595 bytes. The first and second Memo values each use five
chained rows. Their decoded public getter values and raw descriptors agree.
The sibling OLE fields retain their exact independent getter bytes.

The Memo owned map starts inline and becomes indirect after the second Memo is
stored beyond the padding gap. Its packed reference and row extent stay stable.
The candidate map instead shifts its inline window and remains type 0. This
mixed outcome was anticipated from the pinned writer source and was preserved
without adaptation. There is no observed indirect candidate layout in this run.

The selected ordinary table-owned map is also indirect, with one actual ordinary
data member, 101 or 118. A map's representation does not imply its current member
count. Bitmap pages have the exact four-byte prefix `05 01 00 00`; bytes after
that prefix are bitmap data, not an ordinary owner integer.

## Saved preservation checks

The import-safe [capture wrapper](indirect_lval_observations/capture.py) combines
public getters with independently decoded raw map rows, bitmap bits, descriptors
and chunks. It captures native column blocks, current and historical map pairs,
ordinary/index/column/global roles, exact map-row ranges, bitmap storage, complete
bit counts and future bits. All captured catalog declaration tails are complete.
Logical index aliases canonicalize to their actual physical identities.

The separate [read-only verifier](indirect_lval_observations/verify.py) repeats no
fixture commands. Its first run verifies 48 unique live artifact pins and rebuilds
all six saved snapshots. It checks 2,808 raw row bodies, 48 live chunk observations,
672 map-role observations and 3,240 mutable-role storage comparisons. Repeated
checkpoint observations are counted as comparisons, not distinct stored rows or
chunks. Exact references, overlapping map rows and bitmap/container aliases are
checked while distinct nonoverlapping rows may share a container.

The verifier also proves that all previously present user record locations,
bodies, long-value descriptors and saved chunk bodies survive subsequent phases.
Actual ID cursor order is `[1]` before the second row and `[1, 2]` afterward.
Complete originals retain raw index pages; cursor order alone is not a new
physical-index qualification.

The public Java helper is byte-identical to the executed source. The maintained
public Python wrapper differs from the executed ignored driver only in its
module description and repository-root calculation for its public location.
Both identities are pinned separately. The exact executed driver, input freeze,
compiled class, outputs and original six snapshots remain preserved unchanged.

To verify retained private artifacts without starting Java or the engine:

```sh
python3 docs/testing/indirect_lval_observations/verify.py \
  --report .cache/ace16-indirect-lval-observations-first-20261027/report.json
```

## Admission remains separate

The current default storage checker still reports indirect column-map profiles
explicitly, and the replacement preview still requires qualified inline selected
column maps. No engine admission was expanded for this experiment. A future
mixed-map implementation must keep strict writable provenance, map-row/bitmap
exclusivity, global allocation checks, existing represented EOF cells, independent
map replay and full value/preservation checks. Unknown or missing pointers, map
growth, indirect candidate maps, multiple bitmap spans, in-file free reuse and
old-chain reclamation remain unqualified by this source corpus.
