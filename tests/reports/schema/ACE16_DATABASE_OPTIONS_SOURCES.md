# Explicit database-option insertion reference

The [saved reference](../../results/general/ace16-database-options-sdk-reference-2026-11-08.json)
records explicit Name AutoCorrect OFF authoring through reviewed Jackcess 5.0.4
on fresh copies of two current TableCraft creator outputs: an empty keyless
`Items(Id, Score)` table and an empty Increment primary-key variant. Original
bytes remain unchanged. This is SDK write/reference evidence, not a TableCraft
option writer, native Microsoft Access comparison or application workflow.

Both originals have the seven canonical system roles and no MSysDb object.
Calling `getDatabaseProperties()`, adding the two LONG/DDL zero properties and
calling `save()` fails with `PropertyMaps cannot be saved without a row id`.
The failure is preserved rather than treated as an SDK-created metadata object.
Missing properties do not establish disabled tracking or an enabled numeric
representation.

The explicit engineering recipe then inserts a real MSysDb row: native identity
`0x0f000004`, Databases parent `0x0f000002`, type 2, system flags `0x80000000`.
SDK serialization of `Track Name AutoCorrect Info` and `Perform Name AutoCorrect`
produces one unnamed MR2 map with two LONG/DDL zero entries, 152 bytes total.
Both complete outputs reopen through a read-only SDK getter with exactly one
MSysDb object, both exact properties and the original user table/index counts.
No enabled-setting or NameMap behavior is inferred from authoring explicit OFF.

Two SDK write sequences remain distinct:

- Inserting an empty role and subsequently saving its properties creates a
  relocated catalog root plus a deleted physical record. Both observations pass
  SDK reopening, but they do not enter the current rename source grammar.
- Supplying the serialized property map in the initial role insertion preserves
  nine ordinary catalog slots. It appends one external LVAL page at EOF, marks
  that page in the existing LvProp owned/candidate map cells 16 and 17, appends
  the MSysDb catalog record and increments the catalog definition's record count.
  The original global map and all user TDEF/DATA/INDEX bytes remain unchanged.

The SDK's reviewed preferred inline threshold is 64 bytes. This 152-byte map uses
the single-external-row carrier, not the larger inline representation used by
some independently read-qualified engineering fixtures. Reader acceptance does
not authorize introducing a new native write-size rule.

The independent [literal model](../../access/schema/ace16_database_options/model.py) calls no
TableCraft or SDK codec. Its complete expected files match both one-call SDK
outputs exactly: 40,960 bytes for keyless and 45,056 bytes for generated-primary,
with zero differing bytes. The model retains all original bytes except the exact
catalog count/directory/free-space/append scope and two existing allocation bits,
and authors the new LVAL page separately. The saved record pins source, output,
helper, model and SDK hashes, including the earlier failures.

The maintained [engineering helper](../../access/schema/ace16_database_options/InsertOptions.java)
accepts a mode and a caller-created fresh copy. Modes are `read`,
`missing-role-save`, `insert-off` and `insert-complete-off`. The last mode captures
the complete role and properties in one insertion. Run it only on fresh copies;
it intentionally exercises SDK writes. Generated databases and Java artifacts
remain ignored rather than becoming third-party fixture dependencies.

The next native operation must require an explicit user request, prove the
original creator/catalog/allocation ownership and preserve user indexes/bytes.
It must keep original seven-role constructor languages distinct from sources
containing the new eight-role metadata. Existing add-table source admission
assumes the original seven-role ledger, so composition needs its own source guard
and retained-intent qualification. Existing rename V1/V2 guards still require
inline OFF properties and do not admit this new external-LVAL source. This
reference supplies no normal option mutation, recovery or frontend admission.

## Native candidate checkpoint

`NativeImage::plan_disable_name_auto_correct` now constructs a separate owning
candidate on the source-qualified single-table creator ledger. It reuses the
existing native catalog record owner and shared dense-page append helper,
appends the one external property LVAL page, and verifies exact unchanged source
bytes outside the evidenced append/count/map-bit scope. It does not silently
change creator defaults. This candidate has no original persistence intent,
adapter/runtime review or frontend setting yet. Its compilation, focused
candidate tests and SDK full-image comparisons are pending the coordinated batch.
Existing add-table and rename contracts are unchanged: the newly added role is
not automatically admitted by either earlier source language.
