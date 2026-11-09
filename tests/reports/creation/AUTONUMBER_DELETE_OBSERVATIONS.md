# Ordinary LONG AutoNumber DELETE observations

The [compact receipt](results/ace16-delete-autonumber-originals-2026-10-26.json)
records 16 successful engineering DELETE candidates and 16 successful disposable
append descendants across ACE16 V2016/V2019. No engine change was needed. These
are public Jackcess comparisons, not native Access save/reopen or data-macro
qualification; ordinary backend and CLI write eligibility remains unchanged.

## Original construction and inspection

[AutoNumberDeleteFixture.java](AutoNumberDeleteFixture.java) is original
repository-owned code. It uses the reviewed Jackcess 5.0.4 JAR, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Its existing dependency provenance is in [the fixture ledger](FIXTURES.md).
Expressions are disabled, the time zone is UTC and linked opens throw.

Each fresh original contains Numbered and Loose tables, each with ordinary LONG
AutoNumber ID and LONG Score columns. Construction performs 260 public generated
inserts, then public construction deletes leave IDs 1, 3, 7, 64, 128, 259 and 260.
Score is ID multiplied by ten. These deletes are part of original construction;
the resulting originals are frozen before reference or candidate experiments.
Numbered has Primary and PrimaryAlias sharing physical index zero and the same
root (91 in V2016, 108 in V2019). Loose has no indexes.

Read-only reopens capture actual column flags/properties, row locations, counters,
index metadata and order. The AutoNumber flag byte is 7, ordinary LONG is 3,
extended flags are zero and neither column has stored properties. No properties
are stripped. The raw LONG counter at the actual format object's offset 20 and
its read-only getter both report 260; complex-counter offset 28 reports zero.
Inspection calls only `getLastLongAutoNumber`/`getLastComplexTypeAutoNumber`, never
next/adjust/restore methods. The getter does not allocate IDs.

## Deletion and subsequent insertion

For each table/profile, fresh public-reference copies delete first ID 1, highest
ID 260, subset 3/128/260, or all seven IDs. All 16 reference copies retain LONG
counter 260, including empty tables. Twenty separate reference append descendants
cover the originals and each delete state. Public `Column.AUTO_NUMBER` inserts
actually allocate 261; this is observed rather than inferred from surviving IDs.

Frozen engineering example `ace16-delete-restrict-20261026`, SHA-256
`aeb9a06d16d7d6356fe7ab0f2c53afd9f96f851da587ec0b407b5aff3c088f08`, was
compiled from `1e776246313e114c7ea0d8f4b18fe3fc4162ef7f`. Its
[build receipt](results/ace16-delete-restrict-example-build-2026-10-26.json)
source objects were verified against that immutable revision, independently of
later working-tree edits. Each request uses the original live ordinal and exact
raw record bytes, including framing and null mask. All 16 candidates match the
corresponding reference's complete row values, locations, counters, metadata and
index membership. All 16 disposable candidate append descendants allocate 261
and match the corresponding public-reference values and membership.

Independent raw framing checks retain exact surviving encoded key/reference
bytes, complete logical primary/alias declarations and zero entry counts for
empty indexes. Record bodies, native flags, historical index counters and both
AutoNumber counters remain unchanged. Whole-image comparisons allow only the
target live count, requested tombstone slot words and used index repack spans;
all other bytes and source length are identical. Originals and reference sources
remain unchanged. No seed reset, explicit-ID insert, GUID/random AutoNumber or
complex allocation behavior is qualified by these checks.

The first candidate replay stopped on an authored Python `KeyError`: an empty
index emits no ORDER records. The corrected lookup uses an empty order while
separately requiring both index declarations and zero actual populations. The
original driver and partial captures remain hash-pinned; no expected data or
engine behavior changed. The fresh accepted replay passed all 16 cases.

## Reproduction

Use fresh output directories and the exact reviewed JAR. Database binaries,
classes, complete getter transcripts and failed captures stay ignored. The
[reference capture](capture_autonumber_deletes.py) and
[candidate comparison](compare_autonumber_deletes.py) are import-safe, bounded
runners; no Cargo build, imported SQL or active database content is invoked.
The commands below describe a fresh reproduction, not byte-reproducible database
generation. The actual original hashes remain in the ledger and receipt.

```sh
mkdir -p .cache/autonumber-reproduction/classes
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/autonumber-reproduction/classes docs/testing/AutoNumberDeleteFixture.java
python3 docs/testing/capture_autonumber_deletes.py \
  --directory .cache/autonumber-reproduction/fixtures \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --classes .cache/autonumber-reproduction/classes \
  --report .cache/autonumber-reproduction/reference.json
python3 docs/testing/compare_autonumber_deletes.py \
  --binary .cache/tablecraft-bin/ace16-delete-restrict-20261026 \
  --binary-sha256 aeb9a06d16d7d6356fe7ab0f2c53afd9f96f851da587ec0b407b5aff3c088f08 \
  --build-receipt docs/testing/results/ace16-delete-restrict-example-build-2026-10-26.json \
  --reference .cache/autonumber-reproduction/reference.json \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --classes .cache/autonumber-reproduction/classes \
  --output .cache/autonumber-reproduction/candidates \
  --report .cache/autonumber-reproduction/candidates.json
```
