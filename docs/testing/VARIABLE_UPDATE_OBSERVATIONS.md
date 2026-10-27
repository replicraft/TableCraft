# Variable-record allocation preview: eight original controls

All eight engineering candidates completed on their first run, with zero
model disagreements. The [compact receipt](results/ace16-variable-originals-2026-10-26.json)
records eight native candidate calls, twelve read-only getters and twelve read-only
map probes: 32 commands. This is a separate eight-case supplement, not another
execution of Slot162, its batch supplement, or PAGE180.

The candidates use the four unchanged sources and eight public UPDATE references
from [cross-page observations](CROSS_PAGE_UPDATE_OBSERVATIONS.md). They compare
actual typed rows and complete logical index orders against those references,
while independently certifying engineering byte preservation. The reference
writer is Jackcess 5.0.4; Microsoft Access execution and native saves remain
unqualified.

## Fixed cases and results

The selected field is uncompressed, unindexed Short Text `RawLegacy.Label` on
row ID1. Both ACE 16 public profiles are covered. Each growth replaces NULL
with 255 U+0100 units; each shrink replaces that value with `B`.

| Profile / source history | Growth target | File growth | Following shrink |
| --- | --- | ---: | --- |
| V2016 / true root hint, after four wide appends | new EOF255:0 | 4096 bytes | same target, 523-byte capacity |
| V2016 / false root hint, after fifth wide append | existing owned255:1 | 0 | same target, 523-byte capacity |
| V2019 / true root hint, after four wide appends | new EOF272:0 | 4096 bytes | same target, 523-byte capacity |
| V2019 / false root hint, after fifth wide append | existing owned272:1 | 0 | same target, 523-byte capacity |

Each shrink starts from its corresponding **candidate-grown output**, distinct
from the public writer's grown reference. Four independent histories therefore
exercise both real allocation and reuse, then fitted updates of the resulting
hidden targets. Root identities remain 97:0 or 114:0. All eight native stderr
streams retain the explicit engineering-candidate notice.

The true-hint growths clear the insufficient root-page hint, set the represented
new owned/hint bits, and clear the new page's global-free bit. The false-hint
growths reuse the highest existing hinted owned page without changing maps.
All four shrinks change neither file length nor maps. No map is grown and no
arbitrary globally free in-file page is reused.

## Independent byte and semantic checks

The [proof](variable_preview_originals/proof.py) constructs an entire expected
candidate from its actual source, with these narrowly derived changes:

* The original root slot receives its forwarding flag and exact destination
  pointer; an ordinary root's remaining old record bytes are zeroed.
* A new hidden slot and record occupy the exact contiguous destination space.
  Destination count/free bytes and directory words are checked literally.
* Every appended EOF page has an independently constructed zero-initialized
  header, owner, count, free space, hidden directory word and raw UTF16 record.
* Only the exact independently captured owned/hint/global bitmap bytes change.
  The global future-bit clear is checked even when the in-file free-membership
  difference is empty.
* Fitted shrinks retain all 523 physical bytes and old padding after the new `B`
  payload; the variable end offset changes. Public reference canonicalization
  is not used as the candidate's raw-byte golden.

The resulting expected image must equal the whole output byte-for-byte. This
also protects every original byte outside those regions. All preexisting raw
index-signature pages remain unchanged: 100 per V2016 source and 102 per V2019
source. Full typed rows, all other raw row bodies, complete logical index
metadata/orders, table counts and the selected table's sole physical index
historical counter are checked. Counts remain 21 or 22 for the two source histories.
No selected Text key is re-encoded because Label is unindexed.

## Frozen provenance and reproduction

The executed artifact was compiled from committed revision
`87c35d435450251d69f305cb247741ccbf7be7e2`; its [build receipt](results/ace16-variable-example-build-2026-10-26.json)
pins the 17,512,496-byte binary and 107 selected historical Git source objects.
Its SHA256 is `2d2b173617e115a3bca3b10018086dab1c5d8f3d72b536fe3825d21e19cb4a5f`.
The source inventory is selected, not an exhaustive compiler-input declaration.

The [plan](variable_preview_originals/plan.json) is byte-identical to the frozen
pre-call plan (SHA256 `24e0bc6eb9af67b39976a4a8eb731402e6291216c0015c50687ffb578cc9a022`).
Exact executed private runner/proof and imported helper bytes were archived
before the first candidate call. All requests were written and pinned at that
same boundary. Original images, references, requests and tools remain unchanged.

The current public [runner](variable_preview_originals/replay.py) corrects package
paths and adds read-only verification; it was not the historically executed
runner. The public proof has readability changes. The receipt keeps both source
versions distinct. Current public verification redecoded all preserved getters,
maps and images and repeated all eight proofs without another native/getter/map
process:

```sh
python3 docs/testing/variable_preview_originals/replay.py --verify-existing \
  .cache/ace16-variable-original-candidates-initial-20261026/report.json
```

A fresh reproduction requires preserved registered inputs, the reviewed JAR and
compiled repository-owned helpers. It also requires explicit `--execute`, a fresh
ignored `--workdir`, and `--artifact`, `--artifact-sha256`, `--revision`,
`--build-receipt`, `--jar` and `--classes`. The runner verifies the artifact and
historical Git source pins before calls. Running it without `--execute` only
checks the frozen plan and source/reference pins.

This eight-case proof does not qualify arbitrary map growth, indexed key splits,
Memo/OLE allocation, other record layouts, Binary allocation, macros, journalable
edits or normal CLI/backend writes. The existing exact-span and same-page preview
services keep their separate contracts.
