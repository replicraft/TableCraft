# Ordinary ACE fitting Text and Binary writes

`NativeImage::plan_recoverable_ordinary_fitted_updates` accepts ordered
`NativeFittedIntentGroup` requests containing ordinary Short Text/Binary updates
and optional Boolean, Int32, Int64, Currency or legacy DATE groups. It uses the
same fresh [ordinary admission](NORMAL_ACE_ORDINARY_FIXED.md), constraints and
physical preservation checks as ordinary fixed writes. A metadata selection
hint grants no permission; every plan and recovery checks the actual source.
Indexed/dependent/generated targets and required programs remain outside this
fitted route. Decimal can remain an unchanged sibling, not an update carrier.

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
that exceeds aggregate record capacity refuses before journal publication. There
is no record growth, page allocation, index maintenance for these targets or EOF
change. Memo/OLE, richer property profiles and broader macro handling require
separate qualification.

## Original intent and consumers

TCNI0005/profile5 stores complete BEFORE binding and original ordered group names,
explicit Fixed/Variable families, row ordinals and exact expected/replacement
carriers. It contains no native field layout, final rows, predicate, program,
permission or absence certificate. Empty groups retain their family. The carrier
ceiling is 8 KiB and the complete intent ceiling is 64 MiB; these processing
limits do not enlarge native field widths. TCNI1–4 formats remain unchanged.
Recovery re-admits BEFORE, replans original requests and compares both complete
images before allowing recovery or write-free confirmation.

`NativeJournalEditor` and retained `OfflineWriteSession` select fitted planning
when an ordinary request includes a Text/Binary target, even with zero matches.
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

[Authored sources](../../testing/ACE16_ORDINARY_FITTED_SOURCES.md) supply 30 literal
outcomes. Thirty native commits, fresh independent proof, malformed-image checks,
exact resource/cancellation checks and repeated recovery pass. Twelve recovery
contexts start from genuinely mixed BEFORE/AFTER record bytes. Thirty typed and
eighteen SQL session saves pass; the CLI passes eighteen saves and exact TSV
readbacks. All 1,093 ACE unit tests and 83 recovery tests pass, alongside strict
feature-profile checks. See the [write result](../../testing/results/ace16-normal-ordinary-fitted-writes-2026-11-01.json).
Evidence currently covers these authored schemas. Genuine native-created fitted
schemas, independent SDK reads and Microsoft Access save/reopen remain unqualified.

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
