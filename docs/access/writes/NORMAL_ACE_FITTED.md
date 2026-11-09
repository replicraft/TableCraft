# Ordinary ACE fitting Text and Binary writes

`NativeImage::plan_recoverable_ordinary_fitted_updates` accepts ordered
`NativeFittedIntentGroup` requests containing ordinary Short Text/Binary updates
and optional Boolean, Int32, Int64, Currency or legacy DATE groups. It uses the
same fresh [ordinary admission](NORMAL_ACE_ORDINARY_FIXED.md), constraints and
physical preservation checks as ordinary fixed writes. A metadata selection
hint grants no permission; every plan and recovery checks the actual source.
A separate [indexed Binary fitting branch](NORMAL_BINARY_INDEX_UPDATE.md) now
maintains qualified Short Binary secondary root leaves. Other indexed targets,
dependent/generated targets and required programs retain their existing limits.
Decimal can remain an unchanged sibling, not an update carrier.

## Record-local fitting

All variable changes to an original record share its existing payload capacity.
The serializer rebuilds the variable region once, then overlays fixed payloads
and mask bits. Changing group order does not change the result. NULL and present
empty Text/Binary remain distinct. Text obeys declared UTF-16 limits, compression
metadata and qualified raw/compressed/escaped-BOM encodings; Binary retains exact
bytes. Numeric NULL transitions preserve inactive payloads. Noops retain original
raw representations. Empty groups still validate target metadata.

Slot positions, record lengths, unrelated variable data, inactive slack, masks,
footer framing and all bytes outside affected records remain protected. A request
that exceeds aggregate record capacity refuses before journal publication. For these unindexed targets there
is no record growth, page allocation, index maintenance or EOF change.
[Inline Memo/OLE targets](NORMAL_INLINE_LONG_UPDATE.md) now have separate fitting
qualification under complete inline-only LVAL authority. External long targets,
richer property profiles and broader macro handling remain work.

## Original intent and consumers

TCNI0005/profile5 stores complete BEFORE binding and original ordered group names,
explicit Fixed/Variable families, row ordinals and exact expected/replacement
carriers. It contains no native field layout, final rows, predicate, program,
permission or absence certificate. Empty groups retain their family. The carrier
ceiling is 8 KiB and the complete intent ceiling is 64 MiB; these processing
limits do not enlarge native field widths. TCNI1–4 formats remain unchanged.
Recovery re-admits BEFORE, replans original requests and compares both complete
images before allowing recovery or write-free confirmation.

`NativeJournalEditor` and retained `OfflineWriteSession` select ordinary variable
planning when a request includes a Text/Binary target, even with zero matches.
Original encoded capacity chooses fitting or qualified
[owned-page growth](NORMAL_ACE_OWNED_GROWTH.md), then qualified
[first EOF allocation](NORMAL_ACE_EOF_ALLOCATION.md), before staging. The explicit
fitted native API remains fitting-only.
Typed column batches retain exact snapshot handles/carriers. Fixed-only requests
keep TCNI4; positive-program and Jet4 routes keep their existing families.
The existing SQL/session and `query journal-update` consumers share this planning
and journal lifecycle; no new command or encoding choice is introduced.

SQL acceptance is narrower than typed acceptance. Its representable cases cover
Text NULL/empty/Unicode/BOM changes, Binary empty→NULL, self-assignment and true
zero-match updates. Binary growth/present-empty literals, shared variable growth,
joint five-fixed carriers and DATE signed-zero requests retain native/typed
coverage; no unsupported SQL literal or coercion is invented.

## Evidence and limits

[Authored sources](../../../tests/reports/rows/ACE16_ORDINARY_FITTED_SOURCES.md) supply 30 literal
outcomes. Thirty native commits, fresh independent proof, malformed-image checks,
exact resource/cancellation checks and repeated recovery pass. Twelve recovery
contexts start from genuinely mixed BEFORE/AFTER record bytes. Thirty typed and
eighteen SQL session saves pass; the CLI passes eighteen saves and exact TSV
readbacks. All 1,093 ACE unit tests and 83 recovery tests pass, alongside strict
feature-profile checks. See the [write result](../../../tests/results/rows/ace16-normal-ordinary-fitted-writes-2026-11-01.json).
The [genuine property/OLE sources](../../../tests/reports/storage/ACE16_GENUINE_PROPERTY_LVAL_SOURCES.md)
add two unchanged SDK-created originals and twelve independently modeled fitting
outcomes. Twelve native commits, twelve typed and twelve SQL session saves, and
twelve CLI saves/readbacks match the complete expected images. Two additional
stale-handle predecessor saves retain completed receipts. Recovery rejects altered
protected content and handles repeated and mixed partial states. The independent
SDK getter matches all twelve modeled outcomes. All 1,093 ACE units, 83 recovery
regressions, 79 adapter units and 94 CLI units pass, with strict feature-profile
checks. See the [genuine write record](../../../tests/results/storage/ace16-genuine-property-lval-writes-2026-11-01.json).
Microsoft Access save/reopen remains unqualified.

## Missing property declarations

Constraint audits retain `NotDeclared` for absent properties; they do not report
an inferred rule as checked. The current no-property profile follows the
[Jackcess column validator](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java),
which treats an absent stored AllowZeroLength value as allowing empty strings.
The [DAO property contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-allowzerolength-property-dao)
describes its validation role; API construction defaults alone do not identify a
persisted file setting. This is an implementation reference and explicit profile
assumption, not native Access qualification. Nonempty property bags retain their
separate admission limits; an explicit false AllowZeroLength rule cannot be
skipped by an empty request or normal save.

## Genuine tables with properties and read-only LVAL siblings

The target admission profile over the native seventeen-field catalog can retain
an actual supported nonempty
LvProp bag beside an unindexed Short Text target and a read-only Memo/OLE sibling.
This extends admission; it authors no property, long value, catalog entry or
schema. The initial genuine witness is Sentinel.Marker with its original
57-byte PublishToWeb bag: table property, native BYTE, flags0, value0. This is not
permission for arbitrary property bags or unknown native fields.

Strict MR2 framing accounts for every original dictionary, group and property
byte and agrees with the existing decoded metadata before shared property
semantics qualify names, native types, flags and values. Unreferenced names,
padding, unknown active keys and present-empty bags refuse. Recognized field and
table constraints still validate original and final rows; property strings are
not implicitly evaluated as programs. BODY, MODULE, EXTRA and remote carriers
retain their separate absence requirements.

Catalog membership follows actual logical roots and physical storage ownership,
including qualified relocated records, rather than fixed catalog page/slot
assumptions. Full original object identity, native field numbers and selected
record references remain bound. A target certificate still requires the complete
catalog/child/alias census, selected TD classification, actual LVAL declarations
and existing Source preservation checks. Relocation is not an exemption from
ownership or overlap validation.

Selected single-page user declarations now classify ordinary Memo/OLE fields
with exact flags, widths, variable ordering and nonzero owned/candidate map
references. Each actual LVAL field has exactly one declared pair, followed by the
exact terminator and no unknown tail bytes. The original map census and live
reference checks protect LVAL content/storage against modified DATA. General
Text sort versions0/1 retain their original declaration bytes. Unknown extensions,
calculated/complex/append-only/hyperlink metadata still refuse. The global closed
content family keeps its prior no-user-LVAL policy.

Memo/OLE remain unsupported update targets, including empty groups. Marker fits
its original record; OLE Bytes, its inline reference/body, the property bag,
inactive slack and every unrelated file byte remain exact. Existing TCNI0005
stores only original requests and fresh recovery repeats admission. Genuine
source/expected reads and native/session/CLI writes pass as separate phases;
source admission preceded producer execution. No native Microsoft Access save/reopen, Windows or
broader property/LVAL authoring claim is made.
