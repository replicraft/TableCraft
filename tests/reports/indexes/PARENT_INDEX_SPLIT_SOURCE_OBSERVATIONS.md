# Sparse Int32 sources with measured parent overflow

Two fresh ACE 16 public-builder histories produced **782-row sparse target tables**
whose measured Int32 leaf and parent would both overflow for the planned
`Key=99424 → Int32MAX` replacement. The frozen engineering consumer accepted
both **K→K no-op** requests and returned byte-identical fresh outputs. No
K→MAX request, public UPDATE, parent split or root-growth action is part of this
source phase. These observations do not qualify Microsoft Access behavior.

The [compact receipt](results/ace16-parent-index-split-sources-2026-10-27.json)
records the exact first 11 fixture-phase processes: one Java compilation and,
for each profile, generation, larger-source description, fresh-copy pruning,
sparse-source description and native no-op admission. All succeeded on the
first attempt. Read-only source hashing/Git verification is separate. Larger and
sparse hashes, selection files, complete getter/map outputs, raw tree proofs,
class files and actual executed source archives remain pinned. No accepted
original or prior generator was overwritten.

## Larger generation and bounded sparse qualification

The own producer uses the reviewed Jackcess 5.0.4 public builder and ordinary
cursor deletion. A delegating file channel bounds writes to 64 MiB and refuses
writable mappings. Target generation is capped at 149,999 rows plus one sentinel,
so user rows per generated database never exceed 150,000. Rightmost-path geometry is checked
every 32 inserts; the first qualifying geometry is selected. Full-tree scans
are bounded phase checks, not repeated after every insertion. No engine cap,
header, flag or raw database byte is patched.

Each profile reached the first gate at **99,424 target rows**. Those larger
files exceed the existing 16,384-live-row mutation ceiling and are producer
intermediates, not qualified engine mutation inputs. The fixed pruning rule
keeps every other leaf's maximum and all 454 rows in the rightmost target leaf.
It removes 98,642 rows on a fresh copy, leaving 782 target rows and the separate
sentinel. All 329 leaves, the root, complete child/ROW bounds, subtree maxima,
per-height sibling links and height remain intact. No merge, redistribution or
pruning of index pages was observed.

| Measured sparse fact | V2016 | V2019 |
| --- | ---: | ---: |
| File bytes, before and after pruning | 3,350,528 | 3,420,160 |
| Root and overflowing parent | 91 | 108 |
| Rightmost leaf | 100 | 117 |
| Root height | 1 | 1 |
| Bounded parent entries / right tail | 328 / 100 | 328 / 117 |
| Parent prefix / free bytes | 2 / 6 | 2 / 6 |
| Target leaf entries | 454 | 454 |
| Other leaf populations | 328 singletons | 328 singletons |
| Native selected ordinal | 781 | 781 |
| Physical selected page:slot | 816:228 | 833:228 |
| Data-owned / index-owned pages | 390 / 330 | 390 / 330 |

Both fields are ordinary native LONG: a required unique primary Key and an
unindexed Marker. Complete public rows, native field metadata and all primary
key-plus-physical-row entries are captured and checked. The Marker remains
three times the original Key; the sentinel remains separate. Native ordinal
781 is independently derived from raw data-owned page/slot encounter order,
skipping tombstones, and agrees with the public cursor. The two original sparse
files, their larger ancestors and all selection files remain unchanged.

## Measured capacity gate, not an observed write outcome

Replacing the global maximum with Int32MAX leaves other original leaf-count
partitions unchanged. Independent integer/key/ROW reconstruction gives:

- Updated unsplit leaf: **3,633 bytes**, exceeding its 3,616-byte entry area.
- Balanced 227/227 halves: best-prefix sizes 1,365 and 1,817 bytes, both fitting.
- Parent after adding the new left bound: **3,621 bytes**, exceeding capacity
  even with the best shared key/ROW prefix. The original parent occupies 3,610
  bytes, leaving six free bytes.

The calculation includes every complete row reference and excludes child-pointer
bytes from the shared key/ROW prefix. This is genuine measured parent-capacity
pressure, rather than a large row count or a leaf-only overflow. Because that
parent is currently the root, root growth is a **prediction awaiting an actual
UPDATE observation**. No successful parent-overflow kernel candidate is claimed.

Existing indirect global and index-owned maps already represent the next three
EOF pages: 818–820 or 835–837. Those cells are globally free and unowned. Their
existing bitmap pages are 513 globally and 605/622 for the selected index; map
containers retain the observed global owner 1 and owned/hint owner 0. The raw
reports retain exact cells and unrelated future bits. These facts do not grant
map-growth or allocator authority beyond a separately qualified implementation.

## No-op admission and reproduction

The no-op consumer is pinned to revision
`0c0dc6d904bd112236a383c74b04192ee6c5f90f`, with its frozen artifact and 143
selected historical Git source objects verified. Its actual complete preview
checks accept the final sparse sources under unchanged allowances. Each fresh
output equals its source byte for byte. No tombstone flag was changed to force
admission; historical counters remain distinct from live populations.

The [public verifier](parent_index_split_originals/verify_existing.py) recomputes
the saved complete source/tree/map/selection proofs and checks both byte-identical
no-op outputs without a native or JVM call:

```sh
python3 docs/testing/parent_index_split_originals/verify_existing.py
python3 docs/testing/parent_index_split_originals/selfcheck.py
```

The two Java sources, wire model and arithmetic selfcheck are byte-identical
promotions. The [capture wrapper](parent_index_split_originals/capture.py) adjusts
repository paths and can create a current-source manifest. It is distinct from
the archived wrapper actually executed in the initial history. A current public
reproduction requires fresh outputs and explicit execution:

```sh
python3 docs/testing/parent_index_split_originals/capture.py \
  --freeze-current .cache/fresh-parent-tools.json
python3 docs/testing/parent_index_split_originals/capture.py \
  --execute --freeze .cache/fresh-parent-tools.json \
  --workdir .cache/fresh-parent-sources
```

This reproduces new files; it does not restore historical hashes or authorize
any K→MAX/public UPDATE. The immutable first-generation sources and command
outputs remain the evidence for the accepted history. The public saved-proof
verifier initially needed JSON integer-key normalization for bitmap dictionaries;
that wrapper-only failure/source is preserved, and its corrected read-only check
passes without changing any byte model or repeating a database action.
