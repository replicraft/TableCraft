# Compressed current Memos with stored histories

Reviewed 24 October 2026. A fresh original append-only Memo created through the
reference builder with Unicode compression enabled exposes a concrete history
admission defect. Ordinary current-Memo queries already read it correctly, but
history discovery previously required the visible Memo's extended flags to be
zero. Its supported history members therefore remained unavailable.

The engine now admits exactly the ordinary Unicode-compression bit on the
*visible current Memo*. It retains the existing Memo type, variable storage,
ordinary flags and scalar-codec checks. Calculated/unknown flags stay refused.
History templates and flat stored members still require their prior zero extended
flags. Their encodings, mapping checks and decoders do not change. See the
[stored-history contract](../STORED_HISTORY_QUERIES.md).

## Original evidence

The [producer](CompressedHistoryFixture.java) creates fresh ACE 2007, 2016 and
2019 files with two independently scoped append-only Memo tables. It disables
expression evaluation throughout. Current values include ASCII, Unicode/NUL and
line endings, empty/NULL values and larger Text. Stored versions separately
contain duplicate text/timestamps, older entries, empty/NULL cells and larger
Unicode text; two parents per table have empty histories. Current values are not
replaced by a latest historical entry.

The [compression oracle](CompressedHistoryOracle.java) opens originals read-only,
disables evaluation and uses public Column compression/history association getters
plus metadata getters on the pinned VersionHistoryColumnInfoImpl reference class.
The existing original VersionHistoryQueryOracle and VersionHistoryOracle provide
current values, actual hidden/member names, flat stored order and exact DATE bits.
The reference implementation metadata cast is research evidence, not an exclusively
public-API claim or copied implementation.

Actual visible Memo fields have native type 12, variable storage, flags 0x02 and
extended flags 0x01. Their hidden key remains type 18/flags 0x07/extended flags
0x20. Flat history value fields remain ordinary Memo with extended flags 0x00.
The replay checks these original native declarations independently of the selected
history path, alongside the reference compression getters. It does not infer
compressed child/template support from visible current-Memo compression.

## Accepted replay

The [runner](compare_compressed_histories.py) reuses the original stored-history
query model in both ordinal and stored-database comparison modes. It compares
member projection/permutation, current/history separation, aligned text/time
values, empty/all-NULL collection presence, grouping/counts and member criteria
against independent getters. Unsupported bare members/modes and explicit
native/query/output exhaustion remain buffered refusals.

The [accepted report](results/compressed-history-2026-10-24.json) records 198
passing controls: six original metadata comparisons and 96 query controls in
each comparison mode. The three files retain 30 total parent records and 36 stored
versions. All 26 pinned input, JAR, producer/oracle/helper, selected implementation
and immutable executable hashes remain unchanged. The CLI SHA-256 is
`73a3a3b2a39c109fcf2e6778e32eaf998fbaad1bb4fe1e36dcfa26b0549cca81`.
The reused query runner explicitly permits a 64 MiB cumulative native byte pool;
other limits/default policies remain unchanged.

An initial metadata-runner attempt accidentally indexed text-sort rows under the
same Memo names as native column rows. It stopped at its metadata assertion,
produced no accepted report and is excluded from passing counts. The corrected
runner selects the native six-cell column declarations before the complete replay.
Earlier tiny compressed-history capability files remain preserved but excluded.

Focused engine tests cover exact visible flags, calculated/unknown/fixed-field
refusal, unchanged child/template gates, resources and cancellation. Adapter tests
retain metadata-only binding, complete-cache reuse and late-orphan failure under
TOP. The combined engine/adapter targets pass 19 tests with scoped strict Clippy.
These results qualify the current offline engineering read path, not native
Access SQL, compressed child variants, history writes, freshness or live-file
coherence. [Fixture provenance](../FIXTURES.md#original-compressed-memo-history-inputs)
records ownership and original hashes.

```sh
python3 docs/testing/compare_compressed_histories.py \
  --cli .cache/tablecraft-bin/compressed-history-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/compressed-history-20261024 \
  --report /tmp/compressed-history.json
```
