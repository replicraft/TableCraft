# Compound native Binary index reads

`read_table_index` and `read_table_index_projection` now accept qualified
text-free compound indexes containing Short Binary fields. The existing explicit
CLI `--index TABLE INDEX` path uses these APIs. There are two through ten key
components, with per-component directions and mixed Boolean, numeric, legacy
DateTime, GUID and Decimal carriers. Text-bearing tuples use the same owned
physical-key machinery and retain their existing General collation restrictions.
OLE/Long Binary keys remain unsupported.

Binary values retain their exact bytes. NULL and empty Binary are distinct;
eight-byte segments retain the native continuation/final-length tags and descending
inversion rules. Components are fully encoded before the complete entry receives
the native 510-byte cap and overflow digest. This includes Binary/Binary tuples
whose components each fit but whose concatenation overflows. A single native
format constant now owns this entry ceiling, separately from Short Text widths.

Every referenced live row must reproduce the complete key, including fields
omitted from output. The existing walker retains ownership, bitmap/prefix,
parent-bound, sibling, row-reference and full-count checks. Keys shorter than
one marker per component reject before allocation. NULL constraint facts
come from full rows rather than opaque separators. A unique index containing a
full-width/digested key remains explicitly unqualified: digest equality does not
prove full-value equality. Unknown flags/types, ignore-NULL indexes and OLE keys
fail explicitly; there is no sequential fallback. Buffers, byte scans and digest
work retain caller-owned budgets and cancellation.

This adds complete/projection reads and explicit consistency checks. Typed Binary
ranges still require a single-column index, including All/Null/empty selections.
No compound seek, optimizer selection, Binary predicate pushdown, Binary-key
mutation or native Microsoft Access save qualification is introduced. Primary and
foreign Binary fixture qualification remains pending.

## Independent evidence

`BinaryTupleFixture.java` creates original inert Jet 4 and ACE 2007/2016/2019
inputs through the reviewed Jackcess 5.0.4 engineering artifact. The comparison
recipe passes 176 controls across four files, including:

- Complete tagged rows and two CLI projection shapes through 1,000-row node-root
  Binary/Int32 pairs, reversed and mixed directions.
- Ten-column mixed tuples with two Binary fields, GUID, Decimal, Boolean, Byte,
  Int16, Currency, Double and legacy DateTime, including legacy Decimal sign bytes.
- NULL/empty and 1/2/7/8/9/15/16/17/254/255-byte values with zero/high/mixed bytes.
- Unique/all-NULL/empty checks and an explicitly unqualified digested unique key.
- Mandatory corrupt-root and ignore-NULL refusals, without partial stdout.
- Unchanged original and damaged input hashes; mutations use temporary copies.

The prior single-Binary corpus passes 84 controls after its existing one-row
compound case becomes a positive complete/projected/check control. Original bytes
are unchanged. Authored Rust goldens cover NULL/empty, segment framing, descending
bytes, width/type rejection, whole-entry suffix-sensitive digests and explicit
uniqueness limits. These observations establish independent-reader evidence,
not native Access behavior, SQL binary comparison or concurrent-file safety.

```sh
python3 docs/testing/compare_binary_tuples.py \
  --jar path/to/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --rows target/debug/examples/table_data \
  --directory path/to/fresh-binary-tuples --generate
```

The recipe refuses an existing output directory. The reviewed JAR and generated
databases remain local/untracked; [fixture records](../FIXTURES.md#compound-binary-index-inputs)
retain artifact provenance, identities and regeneration limits.
