# Public cross-page UPDATE observations

Eight bounded public Jackcess 5.0.4 `updateRow` calls completed on fresh copies of
four existing allocation-boundary descendants. The [compact receipt](results/ace16-cross-page-update-originals-2026-10-26.json)
records all eight outcomes and 32 commands: eight updates, twelve read-only full
getters and twelve read-only map probes. No INSERT, DELETE or engineering candidate
was called in this capture. The actual executed private source/selectors and full
raw report are archived and hash-pinned; the new public wrapper was not the
historically executed program.

The unchanged [original producer](SlotUpdateFixture.java) supplies ordinary
`RawLegacy(ID LONG primary, Label ShortText255)` rows. `Label` is uncompressed and
unindexed; the table still has its ID primary index. Expressions are disabled,
linked opens throw and each output must be fresh. The selected ID is always 1.
Existing Slot162 and predeclared PAGE180 evidence is unchanged.

## Actual source and destination geometry

Each true-hint source has one owned data page, 21 live/physical rows and 139 free
bytes. ID1 has a 13-byte NULL record. Replacing its Label with 255 U+0100 units
requires a 523-byte record, plus a two-byte target slot, and cannot fit there.
The false-hint sources already contain a second owned page with a true candidate
hint and 3557 free bytes. The false root hint still accompanies 139 real free
bytes; it does not mean the page is physically full.

| Profile / source | Root | Growth target | File growth | After target page free / slots |
| --- | --- | --- | ---: | --- |
| V2016 / after four wide appends | 97:0 | 255:0 | 4096 bytes | 3557 / 1 |
| V2016 / after fifth wide append | 97:0 | 255:1 | 0 | 3032 / 2 |
| V2019 / after four wide appends | 114:0 | 272:0 | 4096 bytes | 3557 / 1 |
| V2019 / after fifth wide append | 114:0 | 272:1 | 0 | 3032 / 2 |

All growth targets are hidden slots with a full 523-byte physical record. Root
page/slot identity remains fixed and the root becomes a forwarding stub. Each
subsequent update to `B` retains that same hidden target and its full 523-byte
capacity. Those four shrinks change neither file length nor maps.

## Map and preservation facts

The two fresh EOF allocations clear the new page's global-free bit, set its
existing represented owned-map and hint bits, and clear the insufficient old
page's hint. No map container or bitmap page is allocated. The source owned map
is indirect and the global/hint maps are inline; the compact receipt retains the
exact existing-byte offsets, masks through before/after bytes, and memberships.

The false-hint sources reuse the already owned alternate page. Their growths
change no global, owned or hint bits. Root-page physical count and 139 free bytes
remain unchanged in all four growths: the extra slot/body belongs to the target
page. No old physical slot is reused or moved.

All eight operations change only the selected typed Label value and retain all
other raw row bodies, logical index declarations/orders, and every preexisting
raw index-signature page byte. Table counts and historical index counters stay at 21 for the first source
pair and 22 for the second pair. Primary row references remain rooted at 97:0 or
114:0. The receipt records all changed old-file pages and all eight output hashes.

The originals also contain globally free in-file pages. These public calls chose
EOF rather than reusing those pages. This is observed behavior, not permission to
reuse arbitrary residual pages or a universal allocation-order contract.

## Reproduction and limits

The parameterized [capture tool](capture_cross_page_updates.py) performs only this
eight-action matrix in a fresh ignored directory. Its current source has syntax
and help checks; it has not been rerun to replace the historical execution. The
promoted [read-only map probe](SlotMapProbe.java) is byte-identical to the executed
repository-owned helper. It inspects reviewed Jackcess metadata by reflection;
those internal names are scoped to the pinned 5.0.4 JAR.

From the repository root, compile the two helpers into a fresh class directory,
then provide the preserved boundary capture, reviewed JAR and classes explicitly:

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/cross-page-fresh-classes \
  docs/testing/SlotUpdateFixture.java docs/testing/SlotMapProbe.java
python3 docs/testing/capture_cross_page_updates.py \
  --boundary-report .cache/ace16-slot-boundary-next-20261026/report.json \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --classes .cache/cross-page-fresh-classes \
  --workdir .cache/cross-page-fresh-capture
```

The boundary sources and original producer provenance are already recorded in the
[Slot reference receipt](results/ace16-slot-original-references-2026-10-26.json).
The new evidence establishes actual public UPDATE behavior, separately from the
earlier INSERT allocation observations. It does not qualify Microsoft Access
execution/native saves, data macros, arbitrary free-page reuse, map growth,
compaction, or Binary allocation. The existing unindexed Binary source has enough
space for its maximum admitted replacement, so it is not an allocation witness.
