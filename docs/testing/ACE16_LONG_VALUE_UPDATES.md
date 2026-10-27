# Memo and opaque OLE replacement previews

With `ace16-write-preview`, `NativeImage::preview_ace16_long_value_updates`
accepts a table, one Memo/OLE column and `SlotScalarUpdate` assignments. Each
assignment supplies a snapshot-local live-row ordinal and complete decoded
expected/replacement values. Memo uses `Value::Text`, OLE uses `Value::Binary`;
both accept `Value::Null`. NULL and present empty values remain distinct.

This is a separate non-journalable engineering service returning
`Ace16WritePreview`. It does not enable ordinary CLI/backend saves, execute
macros or qualify Microsoft Access save/reopen behavior. Existing exact-span,
page-only and variable Short Text/Binary APIs retain their own type and placement
contracts.

## Admission and source authority

The initial profile is plain unencoded ACE 16, subversion zero, ordinary variable
Memo/OLE with native flag 2, extended flags 0 and zero declared length. The
selected field must have a complete qualified inline owned/candidate map pair,
no stored field properties and no index. A readable nonunique Memo index still
does not authorize replacement. Hyperlink, compressed Memo, append-only/history,
calculated/complex fields and unsupported metadata remain refused.

All existing table/index/property/relationship/constraint gates apply before
staging, including empty requests. Unsupported validation and active-content
coverage cannot be skipped. Unchanged numeric relationship keys do not supply a
new Memo/OLE relationship encoder. Defaults remain stored and are not evaluated
for an existing-row UPDATE.

The [read-only storage checker](../access/storage/LONG_VALUE_CHECKS.md) is a
prerequisite, not write permission. A separate paid catalog census inventories
complete current/historical column declarations, ordinary/index ownership and
live external descriptors, including fields without map pairs. A known empty
list is distinct from an unknown suffix. Current NULL/inline fields do not prove
that unknown orphan ownership is absent. Unknown declaration ownership, selected
shared references or overlapping column/catalog ownership refuse this first
writer profile. Owned orphan rows with arbitrary bodies remain valid storage.

Before publication, an immutable original operation supplies the table, column
and requests. The finalizer independently rebinds admission, ownership, logical
roots and exact old/new values from the original image. Staged `Root` references
and staged metadata cannot expand that authority. Duplicate, missing and stale
public requests fail before a candidate is returned.

## Actual storage transitions

Memo is encoded as raw UTF-16LE with an exact existing-decoder roundtrip. It has
no Short Text 255-unit limit. Ambiguous leading-marker values which do not roundtrip
are refused rather than normalized or silently truncated. OLE bytes remain opaque.
Existing long-value/text ceilings and caller budgets still bound both carriers.

NULL clears the selected presence bit and removes its logical descriptor payload.
Present empty uses an inline twelve-byte descriptor. Nonempty payloads up to
64 bytes prefer inline storage. If that completed ordinary record cannot fit on
a fresh qualified ordinary page, the service uses a single external row instead;
a busy original page alone does not change the storage choice. Present empty
cannot be represented by a zero-length external row.

Single storage uses one external row of at most 4,076 bytes. Larger values use
chained rows containing a four-byte next reference and at most 4,072 payload
bytes; the last link is zero. Final tails retain their actual length. Fresh
descriptors have the observed zero reserved word. Semantic no-ops preserve the
original descriptor, body, masks and noncanonical representation byte-for-byte.

New LVAL rows append to qualified column-owned pages carrying the `LVAL` owner
marker. They are ordinary LVAL slots, distinct from hidden ordinary-table
forwarding targets. The engineering placement policy tries the highest currently
hinted page first, then lower hinted pages after actual failed fits. This applies
separately to column-owned LVAL pages and ordinary parent destinations. Actual
free bytes, slot-directory cost and the 255-row ceiling determine whether an
append fits. A false hint does not mean physically full or authorize reuse.

After each actual failed candidate attempt, its hint is cleared. Allocation
appends a fully initialized EOF page only after every eligible hint fails, using
already represented column-owned/candidate/global cells. No map growth, arbitrary in-file free-page
reuse or fixed free-byte threshold is inferred. LVAL chunks and an ordinary
parent forwarding target share one EOF sequence and one aggregated byte ledger.
An existing indirect ordinary-table map does not qualify indirect column-LVAL
allocation.

The [multiple-candidate checks](results/ace16-long-value-multiple-candidates-2026-10-27.json)
include independent complete expected images for two fitting hints, a failed high
hint followed by lower-page reuse, complete exhaustion, the native slot ceiling
and ordinary parent forwarding. Empty/no-op requests preserve both hints, while
an unowned hint still fails admission. A private correlated-forgery check requires
every actual failed attempt even when its map clear is also removed. All scans
and retries retain cancellation and cumulative work/allocation controls. These
are engineering controls; the earlier 60-case original-file corpus has at most
one hinted LVAL page and does not establish native multiple-hint selection.

Only the selected ordinary descriptor payload is reframed. Fitted records retain
their original physical capacity and opaque padding. Growth can append a hidden
ordinary record and update the stable original root's forwarding pointer. Logical
row/index references and live counts remain stable. Original row traversal,
independent of request-slice order, determines append order.

Old chunk rows, slot words and ownership remain unchanged, including chains
which become unreachable after replacement or NULL. There is no retirement,
compaction or orphan-row reuse. These boundaries follow the
[60 public UPDATE observations](LONG_VALUE_UPDATE_OBSERVATIONS.md); they are not a
claim about universal Access garbage-collection policy.

## Independent candidate proofs

The finalizer checks aligned bounded growth before allocation and reparses the
actual output header/catalog. Complete table/index/constraint checks and the
long-value checker must succeed before publication. Requested values must decode
exactly, and unselected record payloads, gaps, masks, old chunks, indexes,
historical counters and metadata remain protected.

A separate original-state replay derives every candidate clear, column/table
owned set and global free clear from actual attempted record lengths, prior page
capacity and EOF events. Planner edit masks alone grant no authority. Physical
page audits separately check exact directory/free geometry, forwarding stubs,
new owners, chain links, payload fragments and initialized unused bytes.

Focused authored serialization tests and independent literal integration tests
exercise these contracts. A private causal test preserved the initial finalizer's
acceptance of a correlated unrequested replacement; immutable operation rebinding
now rejects it while the requested control still succeeds. The separate
[revised 60-case candidate replay](LONG_VALUE_UPDATE_ACCEPTANCE_OBSERVATIONS.md)
uses a committed frozen consumer. The prior public-writer observations remain
distinct from that candidate evidence.

## Explicit engineering consumer

The feature-gated [example](../../crates/engines/tablecraft-ace/examples/preview_ace16_long_value.rs)
buffers a complete candidate before creating a fresh output. It shares the
existing quoted TSV reader and synchronized fresh-file publisher. Its separate
input profile allows 64 KiB per encoded value; the Short Text/Binary example
retains its 1,024-byte value ceiling. Both retain the 16 MiB file and 4,096-request
limits under a shared 256 MiB/100,000-row/100-million-step operation budget.

Each headerless TSV row has five fields: original live ordinal, expected kind,
expected data, replacement kind and replacement data. Kinds are `null`, `text`
and `binary`; NULL data is empty, Text is UTF-8, and Binary is hexadecimal. The
limit applies to encoded data, so 64 KiB of hex represents 32 KiB of Binary.
The entire request file validates before the database or output opens.

```sh
cargo run --locked --offline -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_long_value -- \
  before.accdb fresh-candidate.accdb Sentinel Memo requests.tsv
```

The new consumer passes nine functions and the retained slot consumer passes
eleven; six shared parser/publisher functions count once in the fourteen distinct
consumer total. Checks include a real chained Memo candidate, exact source/old
chunk preservation, fresh output, explicit width profiles and late malformed
input without publication. These checks do not replace original-file replay.

The [scoped implementation record](results/ace16-long-value-updates-native-2026-10-27.json)
pins the actual source, preserved attempts and selected final checks. Shared
consumer functions and repeated default/preview executions are counted explicitly.

## Original-input census correction

The first frozen consumer attempted all 60 raw-source controls and accepted none:
V2016 refused an unrelated opaque backing field, while V2019 encountered an empty
deleted catalog slot. The [first-refusal evidence](LONG_VALUE_UPDATE_REFUSALS.md)
preserves every request, source, error and no-output result.

The [census correction record](results/ace16-long-value-census-correction-2026-10-27.json)
separates physical reference inspection from scalar projection. Existing scalar
long-value profiles, supported calculated Memo outer storage and the observed
variable OLE/flag2/ext0x10 backing profile can be traced without decoding their
opaque value wrappers. This grants neither scalar support nor selected mutation
authority; the selected writer still requires ext0. Unknown physical profiles
and incomplete ownership declarations remain refused.

Ordinary logical-root enumeration now checks the raw deleted/hidden flag before
requiring a body range. A zero-length deleted/relocated slot is skipped; a live
empty or malformed range still fails. Explicit LVAL/forwarding target access
retains its separate physical checks. Both actual original catalog censuses
complete under read-only diagnosis; those checks alone are not replacement
acceptance. The subsequent [revised-artifact replay](LONG_VALUE_UPDATE_ACCEPTANCE_OBSERVATIONS.md)
matched all 60 unchanged controls, including whole-image and cached getter/map
proofs. The initial 0/60 outcome remains preserved separately.
