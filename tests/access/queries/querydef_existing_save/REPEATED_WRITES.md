# Repeated-root and same-page expression reference

Six further explicit Jackcess 5.0.4 metadata updates on copies match six complete
independently authored AFTER images. These extend the [first six observations](README.md)
without changing that recipe, model, report or original files. The
[new first-party recipe](ExistingQueryRepeatFixture.java),
[new raw model](repeat_model.py) and
[observation record](../../../results/queries/querydef-repeat-fragment-save-2026-11-08.json)
retain their own provenance. Native Microsoft Access was not run. The new engine
candidate is connected. Nine normal native cases and two explicit SDK cases
pass in the [native follow-up batch](../../../results/queries/query-fragments-number-batch-2026-11-08.json);
TCQE0002 intent, adapter and runtime Save admission remain pending.

The unchanged three-query, two-DATA-page source has SHA-256
`33e0e41c2152f4e7e5aadb18e0978b4f224ee225d8042dc08d70528f37cbca05`.
All explicit source/output paths are fresh-copy paths under ignored
`.cache/querydef-repeat-save-sdk-20261108/`. No database, classes, JAR or third-party
source is tracked. The reference uses the same previously reviewed Apache-2.0
Jackcess 5.0.4 JAR; its hash is pinned in both reports. Every SDK open disables
expression evaluation. No query, macro, VBA, UDF, provider or native application
executes.

| Case | Actual metadata updates | Observed result |
| --- | --- | --- |
| Initial growth | Constants projection `42` → `1000` | Logical root95:42 forwards to hidden95:44 |
| Forwarded shrink | `1000` → `7` | Retains physical62-byte target; zero padding grows |
| Forwarded growth | `7` → `1000000` | Root now forwards to new hidden95:45; old95:44 retained byte-exact |
| Forwarded no-op | `1000000` → itself | Entire file unchanged |
| Two projections | ManyLocalColumns Order96 `95` → `1000`, then Order95 `94` → `7000` | Two targets appended95:44/45 in explicit update order, opposite native slot order |
| Projection and WHERE | Order96 `95` → `1000`; WHERE `[p].[ID] >= 2` → `[p].[ID] >= 1000` | Two targets appended95:44/45 on the same DATA page |

All complete file comparisons have zero differing bytes. File lengths remain
393,216 bytes. SDK ordered key/logical-ROW fingerprints and source INDEX-signature
page bytes remain equal. The unchanged original is rechecked against its first
report hash. The model constructs each complete expected image from the retained
BEFORE and exact request; the SDK AFTER is used only for comparison. It imports
the first-party raw framing helpers from the unchanged earlier model, never a
production encoder/planner or SDK encoder.

A retired hidden row is not another live query fragment. The repeated-growth
case requires distinguishing the live root's current target from older hidden
records whose bytes remain preserved. A blanket refusal of every unreferenced
hidden record would reject this observed SDK result. Conversely, these inert
observations do not authorize aliases, cross-owner targets, cycles or arbitrary
hidden data to become live logical rows.

Actual commands (both execution steps exited zero, SDK stderr empty):

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/querydef-repeat-save-sdk-20261108/classes \
  docs/testing/querydef_existing_save/ExistingQueryRepeatFixture.java
java -cp .cache/querydef-repeat-save-sdk-20261108/classes:.cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  ExistingQueryRepeatFixture \
  .cache/querydef-existing-save-sdk-20261108/files/multi-page-before.accdb \
  .cache/querydef-repeat-save-sdk-20261108/files \
  > .cache/querydef-repeat-save-sdk-20261108/sdk-observations.tsv \
  2> .cache/querydef-repeat-save-sdk-20261108/sdk-stderr.log
python3 tests/run.py tests/access/queries/querydef_existing_save/repeat_model.py
```

The helper refuses existing AFTER paths. Preserve the retained evidence rather
than deleting outputs to rerun these exact commands. The model writes its raw
report under ignored cache; the maintained report adds execution/boundary facts.

The native candidate checkpoint reuses the real reader, logical-root ownership,
normal map/LVAL role protection, record replacement and forwarding/page-event
audits. New source-bound selection uses actual signed ObjectId, Attribute, Order
and expected native inline UTF16, never a decoded vector position. Normal Rust
cases use independent raw fixtures; separate ignored, hash-pinned SDK cases need
an explicit local gate and are not part of a passing normal test count. No Rust
tests were run during the original SDK observation. The later follow-up record
separately retains executed Rust gates and their initial compile/lint corrections.

Compiled/cache/property histories, Microsoft Access Save/events/timestamps,
external Memo allocation, cross-page placement, fragment-topology changes and
older file profiles remain unqualified. The frozen TCQE0001 grammar and its
reconstruction are unchanged.

A later metadata-only correction makes the model path repository-relative. The
initial executed model/report snapshots and hashes remain preserved in ignored
`model-history/`; the maintained report records that history without exposing a
machine path. Only the independent raw model was rerun. All six source/SDK AFTER
records and binary hashes remain identical; no SDK outputs were regenerated.
