# Int32 key updates beside retained Binary indexes

Normal native17 fixed UPDATE can change a qualified Int32 key while retaining
untouched Binary index owners. The qualified BinaryKeys schema has Int32 Id,
nullable Binary KeyValue and Int32 Marker, three physical indexes and six logical
aliases. This extends the existing ordinary fixed service and original TCNI0004
intent with checked V1 recovery; it introduces no new wire version or fallback.

## Admission and preservation

Original declarations select the index facts before staging. Complete canonical
Source, required-program absence, constraints, schema identity and actual image
binding remain mandatory. Every physical index and alias passes strict membership,
ownership, represented-map, allocation and live LVAL exclusion checks. Read or
Actions qualification cannot substitute for mutation authority.

Actual named targets retain fixed-field flags, exact carriers, original ordinals,
stale witnesses, duplicate and span checks. In this retained-index profile,
indexed targets must be eligible fixed Int32 fields. Binary groups passed directly
to the fixed/TCNI0004 service refuse even when empty or equal. The existing Binary
fitting writer remains a separate valid service; host metadata selects that route
for actual Binary assignments without retrying failed fixed admission.

The independent positive changes physical ROW0 Id from 1 to 14, preserving its
Marker 17 and Binary NULL. The primary cursor moves that original ROW last, after
original rows 1 through 12. Both Binary cursors keep their original key plus ROW
associations. Only the four-byte Id payload and 117 active primary-entry bytes
are permitted edit spans. Primary bitmap/free/links/protected tail, both Binary
pages, histories 13/12/12, DATA masks/framing, aliases/maps/properties/catalog,
residual bytes and EOF remain exact. No allocation or historical counter increment
is introduced. Equal assignments and named empty groups retain the original file.

Fresh proof recaptures original Source and facts, independently reconstructs DATA
and I32 key populations, retains original Binary bytes, checks the complete final
Source and validates original properties before full-image and changed-count
comparison. Required NULL and duplicate primary assignments refuse.

## Consumers and recovery

Native ordinary fixed planning, typed retained `plan_columns`, SQL
`plan_batch_update` and CLI `query journal-update` use the same service. For example:

```sql
UPDATE BinaryKeys SET Id=14 WHERE Id=1
```

Native and typed fixed carriers are exact. SQL retains checked integral assignment;
actual overflow refuses before publication. Retained sessions preserve the lease,
refresh snapshots, reject stale preoperation handles, retain the completed receipt
and retire the journal namespace through the existing lifecycle.

Recovery consumes the same checked V1 token and original TCNI0004 request. Fresh
native reconstruction precedes effects. Pending/partial targets roll back;
committed incomplete or mixed targets refuse without writes or sync. Successful
completion confirmation is write-free. Pending rollback may rewrite BEFORE.

## Observed evidence

The [four source views](../../../tests/reports/storage/ACE16_RETAINED_BINARY_ID_UPDATE_SOURCES.md)
and 32 qualified inert reader observations preceded matching writes. Six distinct
contexts cover changed, equal and named-empty requests for two headers; no extra
reverse single-row case or duplicate image is added.

One direct-proof and five native functions pass: six exact commits and literal
proof positives, 150 protected-byte forgeries at each proof and journal boundary,
six wrong counts, four distinct stale proof inputs, six measured public resource
contexts and 36 recovery contexts. Six committed mismatches refuse without effects;
DATA-only and primary-INDEX-only partial targets test the coupled write. Six typed,
six SQL and six CLI saves/readbacks match the frozen files. Twelve post-save stale
handles and typed/SQL/CLI prepublication guards also pass.

Legacy indexed, original-indexed, BeforeChange and generated-companion regressions,
minimal/journal/preview builds, scoped strict checks, dependency boundaries and
formatting pass. The first Marker regression retained an obsolete selected-Id
refusal and reported four passes and one failure. Only the superseded Id-refusal
assertions were removed from current native/host/CLI tests; the remaining native
guard and old host/CLI suites pass. The independently qualified Id positives
replace that former boundary. No producer correction or expected-image retuning
was needed; historical source and result records remain unchanged.

See the [write result](../../../tests/results/storage/ace16-retained-binary-id-updates-2026-11-02.json)
for pinned receipts. These are portable TableCraft save and independent-reader
observations. Microsoft Access save/reopen and Windows mixed-client behavior
remain unrun; full ACE 16 compatibility is not established by this slice.
