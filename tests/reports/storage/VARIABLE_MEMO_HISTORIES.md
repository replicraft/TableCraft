# Wide-variable and Memo history replay

The [accepted replay](results/variable-memo-histories-2026-10-24.json) passes
16 controls and 33,422 exact getter-modeled result cells over two original
ACE16-family V2016/V2019 files. All 13 pinned executable, JAR, recipe, getter,
input and manifest hashes remain unchanged. Every entry in the 1,862-file Rust
manifest matches immutable committed archive
`3fbf14ea482965ae690ee4d1358effd280b0172f`.

No decoder change was needed. This extends the existing WideVariable/Mixed
history coverage with interspersed Memo fields and multi-page payload growth;
it is not the first 255-column read qualification or a native Access
compatibility claim.

## Original history and independent getter

[VariableMemoHistoryFixture.java](VariableMemoHistoryFixture.java) creates fresh
files through public Jackcess ordinary table/column builders, with expression
evaluation disabled. It starts with ID plus 16 variable fields, grows to 127,
128 and 254 variable fields, and adds eight rows at every stage. Nine variable
fields are Memo; the others are Short Text. One early root per stage remains
untouched. Other rows receive late-field/null updates and short or 24KB UTF8
Unicode Memo growth, then an Int32 primary index is added.

There are 32 retained rows, 255 columns and nine Memo fields per file. The
read-only getter mode records public column types and all 8,160 scalar cells,
including historical NULLs. Pinned Jackcess RowIdImpl/TableImpl row-data helpers
independently locate actual row payloads and read their two-byte field count;
this is explicitly implementation-level geometry inspection, not exclusively
public getter evidence. It follows reference row locations without importing
TableCraft format constants or mutating the source.

Both originals retain these actual field-count histories:

| Physical field count | Rows |
| --- | ---: |
| 17 | 1 |
| 128 | 1 |
| 129 | 1 |
| 255 | 29 |

The untouched IDs 1/9/17/25 expose counts 17/128/129/255 respectively. Row masks
and variable-tail widths cross the corresponding historical boundaries. The
reference reports 17 distinct root pages; rewritten rows and Memo payloads retain
their native relocation/long-value structure. Longest Memo content is 12,000
UTF16 units (24,000 UTF8 bytes), independently counted from getter text.

Getter mode opens read-only, disables expressions, installs a throwing link
resolver and asserts that no linked handle was opened. No reference SQL, VBA,
imported expressions or providers are executed. The original recipe, not a
patched binary, authors every qualifying layout.

## Comparison and resource boundaries

[compare_variable_memo_histories.py](compare_variable_memo_histories.py) compares
all 255 columns and a six-column early/boundary/tail projection through both
sequential and explicit PrimaryKey reads. Aggregates check count, historical
late-field presence, UTF16 Len and middle-field NULLs. A late-field NULL filter
returns the untouched historical roots. The two low-limit controls require
runtime native-step/output refusals with empty stdout.

Successful reads use explicit native and execution step ceilings of 10,000,000;
defaults remain unchanged. Initial unraised-budget exploratory reads correctly
exhausted their step allowance and are excluded from full-value qualification.
They remain in `.cache/variable-history-audit-20261024/comparison.json`, with
raised-limit exploration in comparison-raised-steps.json. Those failures were
checked resource refusals, not decoder defects or discarded mismatches.

The accepted suite has 12 successful reads and four classified refusals. Large
TSV outputs are fully compared in memory, then represented by exact byte length
and SHA-256 in the report rather than duplicating Memo text. Getter captures and
recipes retain enough information to reconstruct every expected cell. This is
bounded correctness/locality evidence, not throughput or arbitrary-scale
qualification.

## Provenance and reproduction

Fresh durable originals and getter stdout/stderr are under ignored
`.cache/tablecraft-fixtures/variable-memo-histories-20261024/`. generation.json
records actual producer/getter commands and unchanged original hashes. Initial
private probe originals remain separately preserved and are not substituted into
the accepted report. No third-party fixture bytes or implementation source were
copied; the original recipe/oracle/runner follow the repository MIT license.

The reviewed Jackcess 5.0.4 JAR is Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/variable-memo-history-classes-20261024 docs/testing/VariableMemoHistoryFixture.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/variable-memo-history-classes-20261024 \
  VariableMemoHistoryFixture write \
  .cache/tablecraft-fixtures/variable-memo-histories-20261024/V2016.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/variable-memo-history-classes-20261024 \
  VariableMemoHistoryFixture read .cache/tablecraft-fixtures/variable-memo-histories-20261024/V2016.accdb
```

Repeat for V2019, capture each getter as `.accdb.getters.tsv` /
`.accdb.getters.stderr`, and preserve originals. Fresh timestamps may change
binary hashes; the accepted report pins actual produced files.

```sh
python3 docs/testing/compare_variable_memo_histories.py \
  --cli .cache/tablecraft-bin/array-bound-declaration-cli-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/variable-memo-histories-20261024 \
  --source-manifest .cache/array-bound-declaration-rust-source-20261024.manifest \
  --source-revision 3fbf14ea482965ae690ee4d1358effd280b0172f \
  --report docs/testing/results/variable-memo-histories-2026-10-24.json
```

The frozen CLI SHA-256 is
`40fc7f58e5bfa48458762bfcc97b1b1ea3232ac6c7b10d4a16a15f83cbfb292e`;
manifest SHA-256 is
`8f377ddf7734b22c74ba71f79defbd12ddd116d93c5bbeacb86ddda084e50541`.
The runner checks every manifest entry against `git archive` bytes and pins the
manifest itself. It does **not** claim that a concurrently changing worktree
matches the frozen binary. Only the selected immutable archive supplies source
verification; actual input/tool/artifact hashes are checked before and after.

The runner refuses an existing report. Terminal log:
`.cache/variable-memo-history-final-20261024.log`. A passing exploratory durable
replay is excluded from accepted counts. Native Access comparison, broader
storage histories, live coordination and write qualification remain separate.
