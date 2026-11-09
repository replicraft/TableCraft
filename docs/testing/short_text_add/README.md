# Explicit saved-table Short Text declaration: SDK evidence

Eighteen complete independently constructed AFTER images match eighteen inert SDK
saves on copies: eight compression twins across four source classes, four repeated
Short Text additions, four Number-after-Short-Text additions, and explicit
compressed width1/80 probes. All original
files remain unchanged. A nineteenth comparison pins the third Number addition
against a genuine SDK AFTER. The
[observation record](../results/add-short-text-sdk-source-2026-11-08.json) retains
complete hashes, raw fields/MR2 bodies, changed pages and read-only property facts.
No native Short Text ADD candidate, original-intent language, runtime/GUI command,
Microsoft Access execution or older-format admission is supplied by this record.

## Exact declaration and producer distinction

The requested field is TEXT, 255 UTF-16 units, Required false, AllowZeroLength
true and no DefaultValue property. Separate typed SDK metadata observations verify
that DefaultValue is absent, rather than a present property whose value is null.
Raw MR2 comparison independently establishes false as byte 0x00 and true as byte 0xff
for these specific Boolean properties; conceptual API values alone do not supply
native encoding.

The current TableDraft builder selects explicit Required false and AllowZeroLength
true. Runtime's ShortText TypeDefault lowers to no stored default, and its native
width is 255 units. Current creator extended flags remain zero, so compression
is off. The uncompressed SDK twin matches that physical declaration.
The [defaults audit](DEFAULTS.json) pins the actual source files and separates
this implemented producer from a faithful future UI declaration.

Microsoft documents [Required default No](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/required-property-access)
and [AllowZeroLength UI default Yes versus programmatic No](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/allowzerolength-property).
Its [Short Text workflow](https://support.microsoft.com/en-us/access/create-or-delete-a-short-text-field)
uses Click to Add, naming and Save, and documents Unicode Compression UI default
Yes. Future UI ADD should explicitly request that independently qualified on
carrier. This evidence changes no existing creator default or frozen request.

The [DAO Size contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-size-property-dao)
and [modern field-size instructions](https://support.microsoft.com/en-us/access/set-the-field-size)
support the explicit 255 target. Some Microsoft references contain conflicting
256 wording, and an older FieldSize reference states default 50. No universal
application default, settings profile or supplementary-character counting claim
is inferred; TableCraft's existing unit limit remains explicit.

## Sources and byte scope

| Source | Existing field/property state | Physical result |
| --- | --- | --- |
| ID only | No field property bag | Catalog root forwards once; first LVAL page 10 allocated |
| Inline Number | Score has Required false/default zero inline | Catalog physical span retained; first LVAL page 10 allocated |
| Two Number fields | Exact previous second-Number SDK AFTER | New full property body appends as row 2 on existing page 10 |
| Three Number fields | New genuine third-Number SDK AFTER | New full property body appends as row 3 on existing page 10 |

All Short Text AFTERs are 45,056 bytes. The new variable descriptor uses type 10,
width 510 bytes, actual variable ordinal 0, General sort order 01000409 and updated
variable-column count. Its primary/index tail relocates without changing its
bytes; INDEX page 9 remains exact. New initial-property pages change maps 3,
catalog 4, definition 7 and page 10; repeated external sources change only 4/7/10.
Old external property rows, existing column policies and dictionary order remain
exact. AllowZeroLength is added to the dictionary without renumbering prior keys.
The extra compressed width1/80 probes have the same exact MR2 body and other
metadata as the255 case; only the new field width word changes to2/160/510. These
are explicit requested widths, not inferred application defaults.

The compressed/uncompressed twins differ at precisely one new field extended-flag
byte; all property bodies and other bytes are identical.

Appending OtherLabel to each compressed Label AFTER also matches the complete
independent raw image. Its variable ordinal becomes 1 and the new full MR2 body
appends to the same LVAL page. The unchanged Number helper then separately adds
AfterLabel to each compressed Label source, with explicit Required(false) and
typed zero. Label's compression/AZL/no-default facts and every old property row
remain exact. A source whose dictionary previously held only Required/AZL appends
DefaultValue without renumbering old keys. These are useful bidirectional mixed
source oracles, not native mixed-history writer admission.

The third Number BEFORE is the exact second-Number SDK image
`8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9`.
Its full independent and SDK AFTER hash is
`337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6`.
The unchanged maintained Number helper produced this additional save, without
altering its earlier recipe, outputs or original sources.

## Reproduction and independence

The original [write helper](CreatedShortTextColumnOracle.java) and
[read-only property observer](ShortTextProperties.java) are first-party source.
They use the previously reviewed Apache-2.0 Jackcess5.0.4 JAR, with its complete
hash pinned in the record. Every SDK open disables expression evaluation. No
query, macro, VBA, provider or native application executes. Generated databases,
classes and the SDK binary remain local and untracked.

The [independent model](model.py) reuses only the earlier first-party literal
[raw Number framing](../number_column_repeat/raw.py), never production or SDK
encoders. BEFORE and the explicit declaration construct the expected complete
image before comparison. A portable model rerun also matches all eighteen images
and the third Number; historical execution helper/model/report hashes remain
recorded. The portable SDK wrapper itself was not rerun after relocation: its
unchanged Java helpers are the executed owners.

From the repository root, place the three exact input files named
`generated-primary-before.accdb`, `gui-number-inline-before.accdb` and
`id-only-second-number-sdk-after.accdb` under `SOURCES_DIR`. Their original hashes
are checked. Use fresh SDK/raw output directories:

```sh
python3 docs/testing/short_text_add/probe.py \
  --jar "$JACKCESS_JAR" --sources-dir "$SOURCES_DIR" --output-dir "$SDK_OUTPUT"
python3 docs/testing/short_text_add/model.py \
  --sources-dir "$SOURCES_DIR" --sdk-dir "$SDK_OUTPUT" --output-dir "$RAW_OUTPUT"
```

The wrapper includes the separate metadata-only observer for distinguishing
property absence and typed values. One observation-report helper/local-name
collision is retained separately from the corrected portable-model pass; expected
bytes and SDK outputs were unaffected. No CI test depends on private local database
paths. Zero native dates are preserved; save timing/events, full Unicode behavior,
row migration, page exhaustion, arbitrary properties and native Access parity
remain unqualified.
