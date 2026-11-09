# Raw nonprimary field deletion and inert SDK reads

Four frozen raw hypotheses reopen through the reviewed Jackcess 5.0.4 reader,
with all originals and observed files unchanged. The
[9 November observation record](../../../results/schema/saved-field-drop-raw-sdk-read-2026-11-09.json)
retains complete BEFORE/AFTER hashes, pre-observation helper hashes, native column
metadata, all SDK property groups, index/settings observations and independent
raw reinspection. No SDK DROP writer or Microsoft Access Save was executed.

## What the controls establish

Two pinned empty ACE16 sources retain their ID primary index and explicit Name
AutoCorrect OFF settings. Number history loses `Score`; mixed compressed Text
history loses `Label`. Each source has two separate raw hypotheses:

| Variant | Changed pages | Current selected property group |
| --- | --- | --- |
| TDEF only | 7 | Remains as an orphan with its original two properties |
| TDEF plus newly appended property snapshot | 4, 7, 10 | Removed from the new live snapshot |

Both variants expose the same surviving columns and policies. The ID-only primary
index remains primary, unique and nonforeign. Both stored LONG/DDL OFF settings
remain zero. Number fields retain native numbers/IDs `0,2`, active count 2 and
high-water 3. Mixed fields retain `0,2,3`, active count 3, high-water 4 and raw
variable-slot high-water 2; surviving `OtherLabel` keeps variable ordinal 1,
TEXT(255), Unicode Compression true and AllowZeroLength true. Fixed offsets remain
unchanged. These are empty-source metadata observations, not populated-row proof.

The TDEF-only controls are readable despite their orphan groups. Read success
therefore does not establish complete property cleanup or native Save fidelity.
The appended variants preserve all retired MR2 rows and directory entries, the
database body/reference, maps, user data and index pages. They append a new user
snapshot rather than shrinking an old ROW and changing the adjacent DB row span.
This is an authored representation hypothesis; Access's save form is unverified.

The original dictionary is retained. Every key remains used in these particular
cases: `Score2` retains Required/DefaultValue and `OtherLabel` retains
AllowZeroLength. These observations do **not** qualify unused dictionary keys.
The SDK also enumerates an empty `ID` property map after column getter access;
independent raw inspection finds no persisted ID MR2 group, and hashes prove no
file write. Keep that in-memory map separate from stored groups.

## First-party recipes and provenance

[model.py](model.py) independently removes active descriptor/name records while
preserving high-water counts, native IDs/ordinals/offsets and the indexed TDEF
prefix/suffix. It reuses the unchanged first-party raw
[Short Text placement owner](../../rows/short_text_add/model.py) and
[actual live descriptor reader](../saved_field_rename/model.py), without importing
production codecs. The original indexed suffix is moved intact; successful SDK
index enumeration is separate evidence of the retained ID index.

[prepare.py](prepare.py) requires a fresh directory, checks both source pins and
freezes complete predictions plus all local/shared helper hashes before getters.
[observe.py](observe.py) checks those hashes and the reviewed JAR, compiles the
[first-party observer](FieldDropObserver.java), then opens only fresh copies
read-only. Every open immediately disables expression evaluation. Compile/getter
failures are retained, not retried or reclassified as passing observations.
The executed run had one successful compile, two successful baseline reads and
four successful hypothesis reads; every comparison was true. No failed attempt
occurred. No SDK source, third-party fixture or binary is copied.

The reviewed Apache-2.0 Jackcess 5.0.4 JAR SHA-256 is
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
The five executable/helper files are byte-identical to the executed recipes;
the record preserves their original hashes. Databases, class files, command logs,
private source paths and SDK binaries remain local.

From the repository root, with the pinned `number-second.accdb` and
`text-number-text.accdb` outputs described in the
[source record](../../../results/schema/nac-after-field-add-sdk-2026-11-08.json):

```sh
python3 tests/run.py tests/access/schema/saved_field_drop/prepare.py --repo . --sources-dir SOURCES --output-dir FRESH_OUTPUT
python3 tests/run.py tests/access/schema/saved_field_drop/observe.py --repo . --sources-dir SOURCES --output-dir FRESH_OUTPUT --jar REVIEWED_JAR --java-bin REVIEWED_JDK_BIN
```

No native DROP candidate, original intent, recovery, normal runtime or GUI editing
is supplied by this evidence. Native Access Save/layout/confirmation behavior,
dependent objects, populated rows, selected indexed fields, long-value retirement,
later edits on deleted layouts and older-format writing remain separate work.
The earlier [deleted-column histories](../../../reports/schema/COLUMN_HISTORIES.md) retain independent
populated-row read coverage; that reader coverage does not grant schema writes.
