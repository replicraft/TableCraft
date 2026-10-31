# Variable creation capacity sources

Two independent header-5 sources extend the [variable-scalar source profile](ACE16_VARIABLE_CREATION_SOURCES.md) at the single-DATA-page boundary. They preserve the earlier six originals and reuse their exact five-field schema and qualified canonical envelope. Their source bytes are authored independently of the production constructor, encoder and candidate-map policy.

`maximum-rows` contains 57 records. Every record has an Int32 ID, eight-unit leading-BOM Text stored in 19 bytes, sixteen Binary bytes, a seventeen-byte NUMERIC payload and its Boolean bit. The complete record occupies 69 bytes; the two-byte directory slot brings each row's page cost to 71. After the fourteen-byte page header, 57 rows leave 35 free bytes.

`null-tail` adds a genuine all-NULL non-Boolean row with Boolean false. Its fixed prefix and variable footer occupy seventeen bytes despite its empty payload spans. The record and directory slot consume nineteen bytes, leaving sixteen free bytes. No Required declaration forbids that authored row.

The owned map retains DATA page 8 in both files. Its candidate bit remains set at 35 free bytes because another minimum legal row and slot fit, and clears at sixteen bytes. This is a minimum-record fit boundary for these sources, not a claim that every requested row fits a candidate page or that arbitrary table constraints use this threshold.

The read-only native qualifier checks all 115 user-row observations, complete metadata capture, actual owned/candidate membership and every unchanged byte outside DATA, the definition row count and the selected candidate byte. The independent SDK phase reuses the unchanged successful variable footer-v2 getter and compiled classes. Its separate saved verifier reconstructs both complete original images and compares the full ordered metadata/system/user inventories, including raw variable records.

Both native and independent SDK source/read qualification passed. The first native compilation stopped with ENOSPC before tests; that attempt is retained. The retry passed one native test over both originals and all 115 rows, followed by a successful fresh export of both files. Actual image hashes are frozen. Both fresh SDK reads passed, and the saved verifier matched both complete images and full inventories (115 user-row observations) without child or writer calls. The ordinary native qualifier and whole-image hash anchor also pass (two tests; the explicit exporter remains ignored). No original source or expected observation was adjusted after a producer result.

These inputs do not establish a variable production constructor, physical page growth, native Access save/reopen, macro absence or normal imported-file write permission. The [focused source result](results/ace16-variable-capacity-source-2026-10-31.json) retains phase outcomes separately.
