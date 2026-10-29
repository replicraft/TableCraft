# Original-file Extended Date/Time INSERT proof

One immutable public-builder V2019 original now passes a complete engineering
INSERT comparison and a separate fresh inert typed-row/map read. The candidate
matches the SOURCE/request-derived image byte for byte. A separate two-call
public reference proves the same logical values and physical key/ROW histories.
Its inactive NULL cells differ from the engineering byte policy as described
below. These independent comparisons do not qualify Microsoft Access execution,
native saves or Extended Date/Time index keys.

The repository-authored [helper](ExtendedDateTimeInsertFixture.java) uses the
reviewed Jackcess 5.0.4 dependency with explicit UTC, `LOCAL_DATE_TIME`, disabled
expression evaluation and refused linked opens. Items contains manually assigned
required Int32 ID, nullable Int32 Group and nullable native `EXT_DATE_TIME` Stamp.
Two physical Int32 indexes have four logical names. Sentinel has a separate
unchanged row. No defaults, generated identities, calculation, timestamp index,
source patching or count search is involved.

The [plan](plan.json) and [TSV requests](requests.tsv) preserve the exact executed
recipe bytes, including its original prospective status text; passing phase
receipts, rather than that status text, establish the observations here. Four old
rows contain epoch, NULL, maximum and a 1900 century date. Six new rows cover leap
day, one 100 ns tick, NULL, maximum, epoch and the century date. Every present
timestamp is decoded from all 42 native bytes with seven fractional digits.
Existing NULL fixed payloads are opaque, never reserialized from typed values.

## Actual physical result

The source has 117 pages and definition 106. C3/V0 records have a 52-byte fixed
extent plus one mask byte. Its single DATA hint is page 111, with four slots and
3,862 free bytes. Six 53-byte records plus their two-byte directory cells consume
330 bytes. The result retains the same file size and maps, uses slots 111:4..9
and has 3,532 DATA bytes free. Neither DATA nor INDEX allocation is needed.

The physical roots remain 108 and 109. Ten exact ID and Group key/ROW pairs per
physical owner are checked, including NULL Group and duplicate families. Table
rows grow 4→10. Primary history grows 4→10 and Group history 3→5 because only
11 and 20 add families; aliases do not duplicate these increments. All four old
records, logical references, schema/property bytes, Sentinel and protected prefix
storage remain unchanged outside independently derived mutation ranges.

The public reference has exactly 42 differing bytes from the prefrozen image.
They are all in new ID 12's inactive fixed cells: one Group byte and 41 Stamp
bytes. Both presence bits are clear. The public cells contain the previous
record's Group 11 and ASCII epoch-plus-one-tick bytes, while the engineering
constructor writes zero payloads. They are logically NULL, not stored operands
to interpret or coerce. Every other byte agrees, including masks, counters,
physical indexes and maps. The proof allows opaque new public NULL payloads;
this observation never changes the engineering image or becomes constructor
authority. Both index unused-scratch difference counts are zero in this case.

## Separate phases and retained preparation history

SOURCE3 consists of one compilation, one fresh creation and one inert combined
inspection. Capture leaves qualification false. A separate saved-only raw gate
then binds declarations, properties, ROWs, masks, maps, descriptors, aliases,
histories and catalog roles. One complete expected image is frozen before REF2,
the isolated build or any native output. REF2 is one fresh public copy/append and
one inert inspection. Native1 must match the complete image before fresh1 is
bound; the fresh reader checks typed rows/maps and the full image again afterward.
Every actual runtime phase and the first completed pure construction pass.

Before execution, an unexecuted qualifier checkpoint was superseded to add source
image pinning and terminal closure revalidation. Two unfrozen module-import
attempts failed before any constructor/image and were corrected by import order
only. Their draft bytes/history remain retained. The first source-export attempt
also stopped on ENOSPC before any Cargo invocation, with a zero-byte archive and
no source/target directory. It remains distinct from the qualified isolated build.

The successful build uses committed `c3313b3e2d73f90956e6048792cfd7db66bc2742`,
an initially absent dedicated target and all 4,610 Git-bound source files. Actual
engine compilation and the unchanged source inventory are recorded in the
[compact receipt](../results/ace16-insert-extended-datetime-example-build-2026-10-28.json).
The separately retained executable, archives, logs and source artifacts remain
pinned even when disposable completed Cargo targets are reclaimed.

## Saved verification

```sh
python3 -B docs/testing/ace16_insert_extended_datetime/verify.py
python3 -B docs/testing/ace16_insert_extended_datetime/verify.py --verify-git
```

The default starts no subprocess and imports no cached Python implementation.
It checks exact maintained transformations and immutable pins, rederives the
complete image before reading any AFTER, verifies the archive/build/artifact,
and replays the public and fresh-reader proofs. Optional Git mode starts one
metadata-only batch checking all 4,610 source blobs and the committed build
receipt. Neither mode runs Java, Cargo or the producer. Missing private evidence
returns `available:false` and status 2; contradictions return status 1 without
falling back to a cached success.

Runtime caps remain 30 seconds, 8 MiB files, 16 MiB per stream and 512 MiB heap.
The example's unchanged 1,024-byte default admits the explicit 27-byte
`extended-date` cells. No resource limit was raised. This fitted, one-profile
comparison does not qualify allocation, splits, timestamp keys, data-macro
semantics, normal backend editing or native Access save/reopen. See the
[result](../results/ace16-insert-extended-datetime-2026-10-28.json) and
[owning contract](../ACE16_EXTENDED_DATETIME_INSERTS.md).
