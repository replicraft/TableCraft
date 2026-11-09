# Repeated field ADD after explicit OFF

Four second-ADD SDK saves match complete independent raw predictions frozen before
all writes. The [Nov 9 record](../../../results/schema/field-add-repeat-after-off-sdk-2026-11-09.json)
retains original/prediction/helper hashes, observations and the initial pre-SDK
metadata correction. The sources are preserved first-ADD-after-OFF outputs from
the [preceding record](../../../results/schema/field-add-after-off-sdk-2026-11-09.json).

This is representation evidence. It adds no TableCraft repeated-source proof,
candidate, original intent, normal Save admission or GUI completion. Native
Microsoft Access/Windows and older formats remain unverified.

## Actual order and unchanged owners

| Source | Old user ROW | DB ROW | New user ROW | Separate targets |
| --- | ---: | ---: | ---: | --- |
| Number → OFF → Number | 2 | 1 | 3 | LONG `SecondNumber`; TEXT(255) `SecondLabel` |
| Text → Number → Text → OFF → Text | 4 | 3 | 5 | Same targets |

The actual database descriptor continues pointing to its original 152-byte body;
it is earlier than the live user ROW. The new complete user body appends after
all existing snapshots. Every retired row, old directory entry, DB catalog record,
DB body/reference, allocation map and INDEX byte stays exact. Only pages 4, 7 and 10
change. All images retain 45,056 bytes and nine logical catalog objects. The
unchanged SDK writers/getters verify Number Required(false)/default 0 and Text 255,
compression true, Required(false), AllowZeroLength(true), absent default, plus
both stored LONG/DDL-zero Name AutoCorrect settings. Every open disables
expression evaluation immediately.

These observations support a future source proof that follows actual object
references and chronological user snapshots around the DB insertion. Neither
total LVAL row counts nor a zero settings observation supplies source authority.
The current last-database-append checker and first-ADD source grammar stay closed.

## First-party provenance and reproduction

[model.py](model.py) reuses the maintained descriptor-bound
[first-ADD model](../field_add_after_off/model.py) and its original raw encoders.
Only the new observation follows the actual DB descriptor instead of treating the
last allocated property row as the database. No production encoder or SDK API
supplies the expected bytes. Public format constants are named; the exact
executed model hash is retained separately, and portable model replay reproduces
all four complete predictions. The portable SDK runner has not been executed.

An initial prediction manifest reused the first-event last-row observation. Review
caught that metadata assumption before any SDK write. Its original model and
manifest hashes remain in the public record and full local originals are retained.
Correcting the DB observation changed no predicted AFTER byte or hash. The SDK
compile and all four distinct mutations then succeeded once; there were no SDK
failures or repeated identical successful mutation commands.

[prepare.py](prepare.py) freezes all complete predictions and owner hashes;
[run.py](run.py) verifies them before explicit mutation of fresh copies. It reuses
the unchanged first-party Number/Text writers and
[read-only property observer](../field_add_after_off/AfterOffProperties.java).
The reviewed Apache-2.0 Jackcess 5.0.4 artifact retains SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
No third-party source, fixture, binary, class file or local machine path is tracked.

From the repository root, with the two pinned source files
`number-first-long.accdb` and `text-number-text-text.accdb`:

```sh
python3 tests/run.py tests/access/schema/field_add_repeat_after_off/prepare.py --sources-dir SOURCES --output-dir FRESH_OUTPUT
python3 tests/run.py tests/access/schema/field_add_repeat_after_off/run.py --sources-dir SOURCES --output-dir FRESH_OUTPUT --jar JACKCESS_JAR --java-bin JDK_BIN
```
