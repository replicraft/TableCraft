# Field ADD after explicit Name AutoCorrect OFF

Four fresh-copy SDK saves match four complete raw predictions frozen before any
SDK mutation. The [Nov 9 observation record](../../../results/schema/field-add-after-off-sdk-2026-11-09.json)
retains source/prediction/helper hashes, raw field and ROW observations, unchanged
regions and read-only property results. Both Number and compressed Short Text ADD
work on the two independently pinned OFF-after-ADD sources.

This is engineering representation evidence. It supplies no native TableCraft
ADD-after-OFF source admission, original intent, GUI completion, Microsoft Access
save comparison or older-format qualification.

## Observed storage order

| Source | Previous user ROW | Database ROW | New user ROW | Targets |
| --- | ---: | ---: | ---: | --- |
| OFF after first Number | 0 | 1 | 2 | LONG `AfterOff`; compressed TEXT(255) `LabelOff` |
| OFF after Text → Number → Text | 2 | 3 | 4 | Same two separate declarations |

Each new complete user MR2 body appends after the database body. The database
catalog record, actual descriptor and 152-byte OFF body stay exact at their old
locations. All retired user bodies and old property directory entries remain
exact too. The user descriptor changes to the newly appended ROW. Only catalog
page 4, user definition page 7 and existing property page 10 change; maps and
INDEX page 9 remain exact. All four images retain their 45,056-byte length and
nine logical catalog objects.

Number explicitly stores Required(false) and DefaultValue `0`. Short Text uses
255 UTF-16 units, Unicode Compression true, Required(false), AllowZeroLength(true)
and absent DefaultValue. Inert SDK getters verify the declarations and both stored
LONG/DDL-zero OFF settings after each save. No existing creator default changes.

The database row is an independent live object reference, not another user ADD
snapshot. These results support a future constructive event replay that retains
the OFF insertion between user ADD events; a total LVAL row count cannot establish
user history.

## First-party recipes and provenance

[model.py](model.py) reuses the unchanged maintained Short Text/Number raw encoders.
Its only specialization replaces that isolated module instance's old last-row-only
body selector with a checked follow of the actual user LvProp descriptor. It does
not edit the maintained files or use the SDK/production engine as an encoder.

[prepare.py](prepare.py) freezes complete predictions and source/helper hashes.
[run.py](run.py) verifies those hashes before each explicit SDK mutation and uses
only the unchanged first-party
[Number](../fields/CreatedNumberColumnOracle.java) and
[Short Text](../../rows/short_text_add/CreatedShortTextColumnOracle.java) Java writers.
[AfterOffProperties.java](AfterOffProperties.java) adds a read-only declaration/OFF
check. Every SDK open immediately disables expression evaluation. Sources are
copies of the retained, independently pinned
[OFF-after-ADD outputs](../../../results/schema/nac-after-field-add-sdk-2026-11-08.json).

The previously reviewed Apache-2.0 Jackcess 5.0.4 JAR keeps SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
No SDK source or third-party fixture is copied into the repository. Databases,
classes, binary JAR, raw commands and private paths remain local. The executed
model/observer are byte-identical to the maintained files. Portable prepare/run
recipes replace local paths with CLI arguments; both original execution hashes
and portable recipe hashes are retained separately. Portable prepare/model replay
reproduces all four predictions; the portable SDK runner has not been executed.
The recorded SDK writes used the original recipe hashes. No failed SDK attempts
occurred, and successful mutation commands were executed once.

From the repository root, with a directory containing the pinned source files
`number-first.accdb` and `text-number-text.accdb`:

```sh
python3 tests/run.py tests/access/schema/field_add_after_off/prepare.py --sources-dir SOURCES --output-dir FRESH_OUTPUT
python3 tests/run.py tests/access/schema/field_add_after_off/run.py --sources-dir SOURCES --output-dir FRESH_OUTPUT --jar JACKCESS_JAR --java-bin JDK_BIN
```

A reproduction must use fresh output paths; it does not overwrite prior evidence.
