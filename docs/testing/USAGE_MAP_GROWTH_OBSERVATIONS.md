# Usage-map storage growth observations

Fresh public-builder ACE 16 sources demonstrate two storage transitions: a
previously empty indirect ordinary-owned map gains its first bitmap, and an
inline index-owned map promotes to indirect during a root-leaf split. The
[receipt](results/usage-map-growth-observations-2026-10-27.json) preserves the
sources, exact executed helpers, failure and successful read-only classifications.
These are independent-writer observations, not native Access qualification or
authority for normal saves.

## Sources and finite geometry

Both V2016 and V2019 have `GrowthKeys` with ID LONG, KeyValue LONG and Marker
LONG. ID has a primary index. The two required nonunique ascending KeyValue
declarations, `Ascending` and `Twin`, actually share one physical index. Exactly
900 rows contain IDs 1..900, KeyValue zero and Marker `ID * 17`.

A separate unindexed `Padding` table contains 700 rows, each ID plus seven
uncompressed Short Text255 fields of U+0100. All 1,600 user rows are generated
through public APIs. No headers, native flags, map bits or descriptors are
patched. Expression evaluation is disabled, FK enforcement remains enabled,
timezone is UTC and free-page reuse is explicitly disabled. The link resolver
refuses every open. Only passive Required properties are authored.

Closed-file checkpoints capture empty schemas, the 900 target rows and the final
source after padding. Both complete source prerequisites pass before either
reference UPDATE. The actual selected leaf prefix is five bytes: its 900 entries
occupy 3,605 of 3,616 available bytes. Changing only ID1.KeyValue from zero to
Int32 MAX would require 7,201 bytes, while the two 450-entry halves require
1,805 and 3,601 bytes. All 700 padding records are 3,595 bytes on distinct pages.
There was no population or shape adjustment after observation.

All JVM/compiler processes have 512 MiB heaps and 30-second timeouts. The existing
repository-owned [file bound](parent_index_split_originals/BoundedFileChannel.java)
prevents writes beyond 8 MiB. Engine scalar, row and image ceilings are unchanged.

## The failed action remains a failure

The actual sequence contains **18 fixture commands**, with these distinct phases:

| Phase | Commands | Actual outcome |
| --- | --- | --- |
| Compile, construct and inspect both sources | 13 | All successful |
| First V2016 public UPDATE helper | 1 | Exit 1 after storage mutation |
| Separately approved read-only V2016 classification | 1 | Getter and raw proof pass |
| Compile corrected helper, first V2019 UPDATE, original getter | 3 | All successful |

The V2016 helper passed `Map.of` to `updateCurrentRowFromMap`. That API writes
returned row values into the supplied map. It raised
`UnsupportedOperationException` in `TableImpl.returnRowValues` after the update
path had written storage. The failed copy, stderr, source, class and first report
were preserved without repair. This was not an atomic no-write refusal and is
not counted as a successful reference action.

One separately authorized read-only call used the exact original compiled helper
to inspect that copy. Complete getters, map/index decoding and the frozen
protected-byte proof pass; the result is **validated partial mutation evidence**
with its original failed exit status. V2016 was never rerun.

The separate [ProbeV2 helper](usage_map_growth/UsageMapGrowthProbeV2.java) changes
only the class name and the update argument to a mutable `HashMap`. Its first
V2019 action uses the unchanged original source and a fresh output. The exact
original compiled helper performs the subsequent read-only inspection. Sources
were not regenerated and selectors, values, budgets and expectations did not
change. The original seventeen-command plan is not reported as an all-successful
run. Any frozen-engine baseline calls are separate from these eighteen commands.

## Actual storage transitions

An empty ordinary table-owned map is already type 1 with zero pointers. Inserting
the 900 target rows creates its first bitmap at page 96 or 114. Its owned set is
exactly the four ordinary data pages; its metadata bitmap is not a data member.
This is first-pointer creation, not an observed ordinary inline-map promotion.

The selected physical index initially owns only its low root through a type-0
map. After the padding gap, its root-leaf split produces:

| Observation | V2016 partial mutation | V2019 successful reference |
| --- | --- | --- |
| Original EOF F | 804 | 822 |
| Retained root | 92 | 109 |
| New nested/right index page | 804 | 822 |
| New bitmap metadata page | 805 | 823 |
| New left index page | 806 | 824 |
| File growth | 12,288 bytes | 12,288 bytes |

The original root becomes height one; both new leaves contain 450 entries.
The exact same 69-byte map record changes from inline to indirect, retaining its
packed reference and row extent. Its first pointer names the new bitmap and its
unused pointer bytes are zero. The bitmap has prefix `05 01 00 00` and exactly
three member bits: the retained root and two children. **The bitmap itself is
not an index-owned member.** The already indirect global map clears three free
bits, accounting for both children and their separate metadata page.

## Independent saved-byte proof

The [model](usage_map_growth/model.py) preserves the executed raw proof functions
without their process runner. The [verifier](usage_map_growth/verify.py) performs
no Java, native candidate or database write calls. Its first run verifies 88
unique live pins, six source snapshots, 5,000 source-row comparisons and 3,200
post-update row comparisons. These are repeated checkpoint comparisons, not
8,200 distinct stored rows.

The model rebuilds all numeric key/row-reference streams and index topology from
raw bytes and checks public getter order. It reconstructs the entire new bitmap.
Across the complete original file prefix, only ID1's four-byte value, the retained
index root, the exact owned-map row and three global bit cells may change.
Every other original byte, including unselected rows, unaffected indexes,
definition/counter bytes and adjacent map records, remains exact. Original and
partial hashes remain unchanged by inspection.

The maintained Java file is byte-identical to the executed V2 source. The original
failed helper and all classes remain separately pinned; the three-hunk helper
diff is retained. Public proof function bodies match the frozen executed capture;
only their module composition/import location changes. No private planning file
is imported by the public verifier.

To verify the retained artifacts without repeating fixture commands:

```sh
python3 docs/testing/usage_map_growth/verify.py \
  --first-report .cache/ace16-usage-map-growth-first-20261027/report.json \
  --partial-classification .cache/ace16-usage-map-growth-partial-classification-20261027/report.json \
  --v2019-report .cache/ace16-usage-map-growth-v2019-first-20261027/report.json
```

New reproduction identities can use ProbeV2 with the existing bounded channel
source and the same create/fill/pad/reference/inspect modes. That does not rewrite
the historical helper identity or qualify a regenerated database as an original.
The external reference is the already reviewed Jackcess 5.0.4 binary/source
release; exact artifact hashes and source inspection pins are retained in the
receipt's freezes. No third-party implementation or fixtures are copied into
the new helper or engine.

This corpus does not establish generic map rewrite authority, indirect candidate
growth, a second bitmap span, global-map bootstrap during the selected UPDATE,
in-file reuse or reclamation. Future engineering promotion must independently
prove all current/historical role, record-range and bitmap-storage authority;
late reader validity alone is insufficient. No map-growth implementation or
normal native save is supplied by these observations.
