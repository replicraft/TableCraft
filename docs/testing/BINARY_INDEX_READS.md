# Single-column native Binary indexes

Short Binary native columns now have bounded full ascending/descending index
reads, explicit projected reads and read-only table-check coverage in the four
current read profiles. This uses the existing checked tree traversal: ownership,
depth/cycles, separators, leaf sibling links, prefix reconstruction, reference
sets, row counts and exact key/row agreement. Required/unique declarations use
the existing tuple checks. Scalar query sources may explicitly require the index
without sequential fallback; query ordering/filters remain in the executor.

Binary keys have a NULL/value marker followed by eight-byte padded segments and
length/continuation tags. Descending encoding complements payload, final padding
and final length, while preserving continuation tags. Invalid markers, padding,
lengths, dangling continuations and lengths beyond the column's declared maximum
fail. The decoder uses a fixed bounded canonical value, with no per-key heap
allocation. Entry vectors charge the actual larger key type to caller budgets.
The current field-carrier ceiling is 510 bytes; independent native fixtures below
exercise values through 255 bytes. This is not a format-wide maximum-length claim.

Empty Binary is a non-NULL value marker alone, a strict encoded prefix of nonempty
values even in descending order. It therefore precedes nonempty values in both
directions; descending NULL remains after values. This is physical index ordering,
not an implicit SQL comparison policy. Native entry decoding now excludes row and
node-child trailers before passing key bytes to every key profile, preventing empty
Binary from being mistaken for trailer data. Existing typed profiles retain their
complete entry-length checks.

Ignore-NULL declarations and binary compound keys remain unsupported. Explicit
[typed Binary range reads](BINARY_INDEX_RANGES.md) now have separate evidence.
OLE is a separate native type despite sharing the Binary value
carrier and is not eligible. Text collations/keys remain separate work. Native
write gates explicitly reject binary-indexed tables; read/check coverage does
not qualify key mutation, unrelated edits or ACE save behavior. Stable/offline
sources, existing structural limits, cumulative budgets and cancellation remain.

## Reference and evidence

The already reviewed/pinned Apache-2.0 [Jackcess IndexData](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
describes the general binary segment encoding used by its BinaryColumnDescriptor;
GUID uses the same encoding with a fixed payload. Production Rust and fixture
recipes are original; no upstream implementation or binary fixture was copied.

[BinaryIndexFixture.java](BinaryIndexFixture.java) creates 1,000-row ordinary
ascending/descending indexes, a unique index, an empty table and explicit ignored/
compound refusals. Values cover NULL, empty, duplicates, zero/high bytes, partial
and full segment boundaries and 254/255-byte fields. Both ordinary roots are
verified as node pages rather than single leaves. [IndexRowsOracle](IndexRowsOracle.java)
opens read-only with expression evaluation disabled and now formats byte arrays
as exact hexadecimal values. [The runner](compare_binary_indexes.py) checks tagged
indexed rows, complete/projected CLI queries and table-check coverage, plus explicit
refusals: eighteen controls per profile, seventy-two total. Hashes remain unchanged;
[fixture records](../FIXTURES.md#native-binary-index-inputs) retain their identities.

Synthetic tests cover all four profiles, typed seek refusal, padding damage,
key/row disagreement and the write gate. Codec goldens and fixed-seed generated
properties cover every supported byte length, exact row keys, empty ordering and
malformed segments. Native Access SQL/file behavior, binary primary/foreign-key
fixtures, Windows, shared-file operation and native saves remain unqualified.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-cli --release --locked --offline
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example table_data --release --locked --offline
python3 docs/testing/compare_binary_indexes.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --rows target/release/examples/table_data --directory /tmp/new-binary-index-inputs --generate
```

Generation refuses an existing directory. Omit --generate for immutable comparisons.
