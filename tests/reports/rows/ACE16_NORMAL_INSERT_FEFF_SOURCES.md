# Leading U+FEFF INSERT sources

The [independent literal model](../../access/rows/ordinary_insert_originals/feff_model.py) and
[requests](../../access/rows/ordinary_insert_originals/feff_requests.json) preserve the unchanged
genuine originals. A requested leading U+FEFF in Marker needs a marked-wide
payload: `FF FE 00 FF FE 6E 00 65 00 77 00` preserves U+FEFF followed by `new`.
Separate unescaped UTF16 controls decode to `new`, losing the requested leading
character. Those readable aliases are not valid INSERT expectations.

Six native views and six independent SDK reports pass before enabling this
representation in normal INSERT. The native gate checks exact values, literal
whole-image reconstruction, original row/OLE preservation and existing source
admission. Run ignored `normal_ordinary_sources::insert_feff::` with absolute
`TABLECRAFT_NORMAL_INSERT_FEFF_SOURCE_DIR` selecting the constructed images.
The SDK observations reuse unchanged bounded original/two-row getter classes;
no new Java observer or compiler invocation participates.

The [source record](../../results/rows/ace16-normal-insert-feff-sources-2026-11-01.json)
retains pins, alias classifications and actual outcomes. All original properties,
maps, unrelated bytes and EOF stay exact. This finite witness does not qualify
all leading-U+FEFF combinations, zero low bytes at marked-wide boundaries or
Microsoft Access save/reopen behavior. The first INSERT source and write records
remain unchanged; the proof correction has separate execution evidence.
