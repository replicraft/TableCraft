# Ordinary INSERT sources

The [literal model](../../access/rows/ordinary_insert_originals/model.py) and
[explicit requests](../../access/rows/ordinary_insert_originals/requests.json) use the unchanged
SDK-created genuine originals. They add `(24, 'new', NULL)` to Sentinel using an
independently specified 21-byte ordinary row and existing owned DATA capacity.
The new visible slot and payload, DATA free/slot counts and table live-row count
are the only changed spans. The entire complement, original row, readonly OLE,
properties, maps and EOF remain exact. No native writer supplies an expectation.

Four native views and four independent SDK reports pass before any insertion
producer. The native source gate checks exact rows, schema, properties, long-value
counts, maps and existing source admission. Its ignored selector is
`normal_ordinary_sources::insert::`; provide an absolute
`TABLECRAFT_NORMAL_INSERT_SOURCE_DIR` pointing to the constructed images.
Literal image hashes avoid an additional JSON dependency in the engine.

The [two-row observer](../../access/rows/preserved_fixed_scalars_genuine/OrdinaryInsertGetter.java)
preserves the existing formatter, unrelated table bounds and bounded read-only
channel. Compile using the established channel and pinned Jackcess 5.0.4, then
select `--inspect-sentinel-two` for inserted views. The original one-row observer
remains unchanged and reads the BEFORE images. Generated databases, classes and
JARs remain outside Git; [fixture provenance](../reading/FIXTURES.md) remains unchanged.

See the [source record](../../results/rows/ace16-normal-insert-sources-2026-11-01.json) for
image and observer pins and actual read outcomes. These results qualify source
expectations, not normal INSERT implementation, required data-program behavior,
Microsoft Access save/reopen or Windows durability.
