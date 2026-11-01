# Unchanged numeric-index source qualification

The unchanged V2016/V2019
[integral split originals](results/ace16-integral-index-split-sources-2026-10-27.json)
contain `ByteKeys`, `Int16Keys`, `Int64Keys` and `CurrencyKeys`. Each table has
an Int32 primary ID, the named numeric key and an unindexed Int32 `Marker`.
Ascending and descending numeric indexes plus a logical alias remain unchanged.
These are existing source files, not regenerated or previously mutated outputs.

The independent [original facts](retained_numeric_originals/original-observations.json)
and [model](retained_numeric_originals/model.py) qualify exact native descriptors,
the first DATA record and saved SDK observations. Each literal request changes
`Marker` from 17 to 42 on ID 1 in one table. Eight complete expected images alter
only that four-byte field; all INDEX pages, maps, counters, properties, unrelated
tables, system objects and preserved Memo/OLE content remain exact. Models invoke
no TableCraft writer, native serializer or child process.

Ten native views (two originals and eight outcomes) pass forty complete table
snapshots and index checks, totaling 26,000 row observations. Eight fresh bounded
reads through the existing pinned Jackcess 5.0.4 fixture match every precomputed
inventory line. Source and expected inputs remain unchanged. This is independent
read evidence, separate from normal write admission and Microsoft Access testing.

Explicit source replay uses the retained original directory and expected files:

```sh
TABLECRAFT_RETAINED_NUMERIC_ORIGINALS_DIR=/path/to/originals \
TABLECRAFT_RETAINED_NUMERIC_EXPECTED_DIR=/path/to/expected \
cargo test --locked -p tablecraft-ace --all-features --lib \
  genuine_numeric_indexes_and_literal_marker_overlays -- --ignored
```

The shared testkit loader verifies both original hashes and all eight expected
hashes before returning any case to write tests. No database, class or JAR binary
is committed by this tranche. See the
[source record](results/ace16-retained-numeric-sources-2026-11-01.json).
