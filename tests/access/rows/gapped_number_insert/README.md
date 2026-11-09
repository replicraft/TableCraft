# Gapped Number first-row raw hypothesis and inert reads

One independently authored first-row prediction was frozen before a single
Jackcess 5.0.4 read-only observation run. One compile and two reads succeeded: the
empty post-DROP baseline and the predicted row. Source and observed copies retain
their hashes. This is **zero SDK INSERT writes and no Microsoft Access Save**;
it is not a positive TableCraft writer or GUI insertion claim. The
[record](../../../results/rows/gapped-number-insert-raw-sdk-read-2026-11-09.json) preserves
frozen helper/prediction/observation hashes and actual observations separately
from the later sparse writer work, which is applied but unrun at this checkpoint.

## Literal prediction

The Number source is the appended-property-cleanup DROP image in the
[existing raw deletion record](../../../results/schema/saved-field-drop-raw-sdk-read-2026-11-09.json),
SHA256 `6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b`.
Its live fields are ID/Score2 at native numbers and stored IDs 0/2, with maximum
column count 3 and retained fixed offsets 0/8. A live vector position is not a
native field number or a presence bit.

The [first-party model](model.py) imports the maintained raw DROP declaration
reader and uses a literal record `030001000000000000000000000005`. The native
record retains extent 3, ID 1, zero padding for missing native 1, Score2 zero and mask
bits 0/2. Only TDEF/history page 7, DATA 8 and primary leaf 9 change. The whole AFTER
hash is `b451db719c7735c626fe58bd2f3e39530a4dbb03c2bda1d39a4c5ceb26618c30`;
all property history, database OFF bytes, catalog/maps and page 10 remain exact.
No production encoder, native plan or writer supplies expected bytes.

The [original Java observer](GappedInsertObserver.java) opens a fresh copy
read-only and immediately disables expression evaluation. It checks the actual
LONG metadata, native numbers, stored IDs, high-water, fixed offsets, stored OFF
settings and primary index; sequential and primary-index cursors both return
ID 1/Score2 zero once in the predicted image and no rows in the baseline. The
[wrapper](observe.py) checks pinned source/prediction/JAR hashes, preserves all
compile/read outcomes, and verifies originals and copies unchanged.

## Reproduction and provenance

The reviewed Jackcess 5.0.4 Apache-2.0 JAR has SHA256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`,
with reviewed upstream source pin
`be0a2139b087a227d14cafd4ea231306a5a3a862`. No SDK code, binary or database is
redistributed. The three first-party files are byte-identical to the executed
recipes. Their original preparatory docstrings say unobserved/unexecuted because
they were frozen before the observation; the result record supplies subsequent
execution status without rewriting those historical hashes. No successful
observation was rerun during promotion and no failed attempt occurred.

Use the pinned `number-history-append-live-group-removed.accdb` from the
[raw DROP preparation](../../schema/saved_field_drop/README.md), with a fresh output for
each command, from the repository root:

```sh
python3 tests/run.py tests/access/rows/gapped_number_insert/model.py --repo . --source PINNED_DROP_SOURCE --output FRESH_PREDICTION
python3 tests/run.py tests/access/rows/gapped_number_insert/observe.py --source PINNED_DROP_SOURCE --prediction FRESH_PREDICTION/number-gapped-first-row.accdb --jar REVIEWED_JAR --java-bin REVIEWED_JDK_BIN --output FRESH_OBSERVATION
```

The current service checkpoint retains the original dense-source INSERT refusal
and its corrected ActiveContent expectation. Sparse-generated writer validation,
SDK insertion output, multiple rows/index families, deleted Text variable holes,
populated DROP and native Access behavior remain separate evidence boundaries.
