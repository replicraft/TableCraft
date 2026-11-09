# Wider numeric keys with ordinary siblings: originals

Two freshly created independent ACE 16-family originals now pass thirty inert SDK
observations and native read/canonical Source checks for four table contexts.
Each header has Int64Companions and CurrencyCompanions with Id, KeyValue, Marker,
Enabled, Label and Opaque. Three rows retain MIN/NULL/MAX numeric keys, true/false,
nonempty Text and Binary payload/empty/NULL. Currency values use exact signed
scaled integers through BigDecimal with scale four, without floating conversion.

Each table has three physical numeric indexes and six aliases. Observed records
are 39/39/35 bytes at offsets 4057/4018/3983; DATA free bytes are 3963. Masks
3f/35/1f include the Boolean value bit. NULL KeyValue retains the original MIN
payload at record-relative bytes 6 through 13. Primary leaves have 27 active bytes,
wide leaves 31, original prefix zero and histories 3/3/3. These are recorded
original observations; they are not predictions from a TableCraft writer.

The creation recipe and semantic requests were frozen before one bounded compile,
two CREATE_NEW SDK creations and thirty read-only observations. Strict saved
verification starts no child; copied-terminal controls reject eleven invalid
receipts. Original bytes remain immutable. Native qualification checks complete
fields, rows/raw records, ROW identities, six cursors, counters, properties,
constraints and fresh canonical Source against the independently frozen geometry.
It stages no edit and invokes no mutation planner.

A separate diagnostic records fixed-table capability success in all four contexts,
while named-empty KeyValue facts refuse Unsupported(IndexedColumn). This identifies
the current whole-schema numeric-width gate despite an eligible unindexed field.
Neither a temporary refusal nor read admission is a permanent format invariant or
permission to write. Boolean's raw declared length one is distinct from its
zero-payload native storage and is checked accordingly.

See the [original qualification record](results/ace16-wide-sibling-originals-2026-11-02.json).
Independent AFTER construction, further reader observations, actual mutations,
recovery and typed/SQL/CLI save qualification remain separate work. Microsoft
Access save/reopen and native compatibility remain unqualified.

```sh
TABLECRAFT_WIDE_SIBLING_ORIGINAL_DIR=/absolute/path/to/original-first \
  cargo test -p tablecraft-ace --all-features --lib \
  table::mutation::normal_ordinary_sources::wide_sibling_originals::wider_numeric_sibling_originals_have_complete_raw_read_and_canonical_source \
  -- --ignored --exact
```

The later [independent expected-update qualification](ACE16_WIDE_SIBLING_AFTER_SOURCES.md)
now covers AFTER construction and readback separately from write admission.
