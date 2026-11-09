# OFF metadata after field ADD: first-party engineering recipe

Run from the TableCraft repository root with Python 3, a retained JDK 21 and
the reviewed Jackcess 5.0.4 JAR. No Cargo, native application, database content
execution or production native codec participates.

Copy this recipe directory to a fresh local directory. Supply the five generated
source files at the paths in `source_manifest.json`, or change only its `source`
paths to equivalent files with the exact recorded hashes. Sources are the retained
Number/ShortText SDK ADD outputs described by the repository's Number, repeated
Number, ShortText and mixed-third-Text evidence records. No fixture bytes or SDK
binary accompany this recipe.

```sh
python3 path/to/fresh-recipe/prepare.py
python3 path/to/fresh-recipe/run.py \
  --jar "$JACKCESS_JAR" \
  --java-bin "$JAVA_HOME/bin"
```

`prepare.py` verifies source pins, copies originals and writes complete expected
images plus `predictions-before-sdk.json`. It runs no SDK operation. `run.py`
checks the frozen model hash, compiles the helper and runs one explicit
`insert-complete-off` operation on each fresh output copy. It saves each stdout,
stderr, command, exit code, output digest and structural comparison. Existing
outputs are refused. Originals and snapshots are never mutated. Keep all failed
attempts; use a fresh directory for another execution.

The maintained `docs/testing/ace16_database_options/model.py` owns the independent
literal 152-byte OFF MR2 body. This model imports only that first-party raw body
builder. It independently constructs the database-role catalog row and dense
physical appends. It masks physical catalog flags for offsets while retaining
every old directory word and row. It refuses allocation, compaction and sources
outside these 11-page fitting cases.

`InsertOptions.java` is the maintained first-party OFF insertion helper with
`database.setEvaluateExpressions(false)` immediately after every open. The exact
mechanical change is retained in `inert-helper.patch`. It explicitly inserts a
new MSysDb catalog role with the OFF body; this is not an observation of an Access
UI setting action. It observes the role, typed LONG/DDL zero properties, table
column counts, empty rows and retained primary index after reopening.

The five recorded observations all match their predictions completely. They
append the new role's property body to the **same catalog-column LVAL page 10**
already containing user-table property history. Directory slots 1, 2 or 3 identify
the new body, depending on history. Old user catalog descriptors, old LVAL bodies
and directory words, all map bytes, user schema/data/index pages and file length
remain unchanged. Only pages 2, 4 and 10 change. The catalog logical object count
becomes 9, while its new physical row is slot 9 for forwarded-user sources and
slot 8 for the ordinary inline-Number source.

This establishes a source-composition observation, not native writer admission,
full Access compatibility, rename support or an allocation/rollover oracle.
The old TCDO0001 9/10-page contract is unchanged. Its decoder does not admit these
11-page sources merely because the SDK comparison passed.

## Recorded execution and wrapper portability

The [five-case evidence record](../results/nac-after-field-add-sdk-2026-11-08.json)
pins the actual helper/model and complete source/AFTER images. All five expected
images were frozen before SDK mutation. The initial compile failure occurred
before any database mutation and remains identified in that record. The executed
helper differs from the earlier maintained helper only by the explicit expression
evaluation disable after every open; the original helper was not overwritten.

The published Python wrapper joins the Java classpath with `os.pathsep`. This is
a wrapper-only portability correction after the five recorded macOS executions;
it has been syntax-reviewed but has not been executed or verified on Windows.
The executed helper and independent model hashes remain unchanged. Original
private logs, failed compile attempt and frozen predictions remain retained.

See the [native options contract](../../access/writes/DATABASE_OPTIONS.md) for
current source admission, which this SDK evidence does not broaden.
