# Maximum-width local table histories

Microsoft's [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
list 255 fields per table. The original
[WideTableHistoryFixture.java](WideTableHistoryFixture.java) recipe reaches that
width in Jet 4, ACE 2007 and both ACE 16 layout families using the reviewed
Jackcess 5.0.4 artifact. This is independent engineering evidence; it does not
qualify native Access DDL, compaction, save/reopen or broader historical layouts.

Each fresh database contains two tables. WideVariable has an Int32 ID and 254
Short Text fields, exercising unsigned variable counts/offsets above 127 and a
32-byte null mask. WideMixed combines Boolean, Byte, Int16, Int32 and Short Text,
including Boolean bits and absent fields above the same logical boundaries.
Both have an ascending ID index and definitions spanning multiple pages.

The recipe inserts rows after schema widths 8, 9, 16, 17, 127, 128, 129, 254 and
255. It then grows the first old row to the full current shape, deletes two rows
and appends two. Each final table retains 18 rows. Values include NULL, empty
Text, ASCII, accented/BMP Unicode and supplementary characters. Missing newer
Boolean fields read FALSE; other missing fields read NULL, without defaults or
active expressions. All mutations occur only while creating fresh originals.

The [runner](compare_wide_table_histories.py) separately models values and row
history in Python, then checks read-only Jackcess getters. It compares complete
native reads, one/seven-row cursors, reversed requested projections at logical
boundaries, direct independent/native index order, buffered/progressive CLI
rows, wildcard/projected sequential and selected-index SQL, aggregate counts
and an atomic tiny-budget refusal. TableCraft alone executes SQL.

The generation and original 104-control reuse run pass. Adding direct index
order checks yields 112 controls in the final registered reuse run. Original
file hashes remain unchanged. [Fixture records](../FIXTURES.md#maximum-width-table-history-inputs)
retain the generation identity; downloaded JARs, generated databases, manifests
and logs remain in ignored .cache/tablecraft-fixtures/ and are not CI inputs.

Two focused Rust unit tests provide portable framing coverage independently of
the external corpus. They cover the same width boundaries, literal 255-column
mask/offset goldens, malformed high-slot/end offsets and variable counts, and
an insufficient allocation budget. Existing production admission and decoding
are unchanged; successful wide reads do not qualify historical writes.

```sh
CARGO_INCREMENTAL=0 cargo build --locked --offline -p tablecraft-ace --example table_data
CARGO_INCREMENTAL=0 cargo build --locked --offline -p tablecraft-cli
python3 docs/testing/compare_wide_table_histories.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --rows target/debug/examples/table_data \
  --directory .cache/tablecraft-fixtures/wide-table-histories
```

--generate requires a fresh directory and never overwrites an input. A new
generation can change metadata timestamps and hashes; review/register it before
reusing it without that flag. Opening the databases through either reader does
not evaluate expressions, load VBA or alter their bytes.
