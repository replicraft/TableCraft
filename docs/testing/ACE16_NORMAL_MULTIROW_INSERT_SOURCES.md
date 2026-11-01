# Ordinary multirow INSERT sources

The [literal model](ordinary_insert_originals/multirow_model.py) and
[requests](ordinary_insert_originals/multirow_requests.json) start with the unchanged
genuine originals. They append `(24, 'new', NULL)` and `(25, 'tail', NULL)` in both
orders, using independently specified 21-byte and 23-byte records. Directory
entries and physical row order follow the written order. Both outcomes retain
the original row, OLE, properties, maps and EOF byte for byte; only new records,
directory cells, DATA counters and the table live-row count change.

Eight native read views and eight independent SDK reports pass before matching
producers run. The gate checks exact rows/order, long-value counts, declarations,
existing source admission and the complete untouched byte complement. Run the
ignored selector `normal_ordinary_sources::insert_multirow::` with absolute
`TABLECRAFT_NORMAL_MULTIROW_INSERT_SOURCE_DIR` selecting the constructed images.

The [count-three observer](preserved_fixed_scalars_genuine/OrdinaryMultirowInsertGetter.java)
retains the original formatter and unrelated table bounds, using the established
bounded read-only channel and pinned Jackcess 5.0.4. Select
`--inspect-sentinel-three` on inserted views; the original observer reads BEFORE.
Generated images, classes and JARs stay outside Git. Existing
[fixture provenance](FIXTURES.md) remains unchanged.

The [source record](results/ace16-normal-multirow-insert-sources-2026-11-01.json)
retains pins and observed read outcomes. This evidence qualifies expected bytes
and row order, not native INSERT/save/reopen or Windows durability behavior.
