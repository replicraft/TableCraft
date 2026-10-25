# Boolean equality index selection evidence

The 2026-10-25 [accepted replay](results/boolean-index-selections-2026-10-25.json)
passes 22 CLI controls against two fresh, inert ACE 16-family databases. Sixteen
successful queries compare 8,000 returned ID rows with independent getter-based
expectations; six buffered refusals retain empty stdout. There are 2,000 original
input rows, exercised through several access paths. This is engineering evidence;
Microsoft Access execution and optimizer equivalence remain unqualified.

The implementation is documented in [native indexes](../NATIVE_INDEXES.md#boolean-indexes)
and [automatic selection](../AUTOMATIC_INDEXES.md). Exact Boolean field/literal
equality can use automatic or explicitly required single-key indexes. The original
predicate remains mandatory. Numeric/Text coercions, Boolean ranges, NULL tests,
parameter hints and partial conjunction extraction remain outside this addition.
Microsoft's [Yes/No query criteria](https://support.microsoft.com/en-us/access/examples-of-query-criteria)
specify True/False filtering. Physical interpretation retains the separately
reviewed Boolean codec; no SQL integer reinterpretation was introduced.

## Independent inputs and model

The existing original MIT repository recipe
[BooleanIndexFixture.java](BooleanIndexFixture.java) produced fresh V2016/V2019
files using the reviewed Jackcess 5.0.4 JAR. These files preserve the complete
recipe, including its original unique/primary and stored NULL-to-false assertions.
The new comparison uses `BooleanIndexed`: 1,000 ordinary rows, an ascending index
and a descending index. The earlier complete-index evidence separately covers
actual native node trees; the new engine tests cover bounded branch pruning.
No reference SQL, macros, VBA or imported code runs.

The original read-only [IndexRowsOracle.java](IndexRowsOracle.java) returns actual
ID and Boolean cells through a public index cursor. The model admits only literal
`true`/`false` getter spellings and compares the complete selected ID sequences.
Each profile has 286 true and 714 false rows. Its complete raw getter stdout SHA-256
is `62a1142bd031613c2ee65889dc68b80b0af0a3983168dcd450e457395f6bd5ef`.
Getter outputs are also retained next to their ignored originals as
`boolean-seeks-V2016-20261025.getters.tsv` and
`boolean-seeks-V2019-20261025.getters.tsv`; separate read-only recapture confirmed
the exact accepted getter hashes and unchanged input bytes.
The fresh recipe's insertion order and within-value cursor order both retain
increasing IDs; the replay does not impose an SQL ordering guarantee generally.

| Profile | Original bytes | SHA-256 |
| --- | ---: | --- |
| V2016 | 729,088 | `b98be4717e8eaf2d8ed91633aed70cb7cdc30afd071619ffe32d6bed165be484` |
| V2019 | 798,720 | `34fccdca50890054130d8620f3f10f238752b95e443295fb1a82cb2558f37f44` |

Both originals remain under ignored `.cache/tablecraft-fixtures/`. They supplement
rather than replace the earlier registered Boolean index inputs.

```sh
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/BooleanIndexFixture.java \
  .cache/tablecraft-fixtures/boolean-seeks-V2016-20261025.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/BooleanIndexFixture.java \
  .cache/tablecraft-fixtures/boolean-seeks-V2019-20261025.accdb V2019
```

The producer refuses existing output paths. The runner verifies reviewed JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc` before
launching Java. Its report pins both original sources, inputs, executable, runner
and seven engine/query/Access boundary files before and after execution. All 14
repository-relative pins agree. Every subprocess has a 60-second ceiling.

## Accepted execution

For both truth values and both profiles, sequential access is the complete-read
control; automatic selection and required `BoolAsc`/`BoolDesc` indexes return the
same complete selected sequence. The indexed modes succeed with native row cap
900, below the 1,000-row declaration. Unindexed execution at cap 900 refuses the
complete snapshot. Positive native row budget 1 and output budget 1 separately
produce runtime resource/output errors with empty stdout. These are bounded
correctness checks, without performance claims.

```sh
python3 docs/testing/compare_boolean_seeks.py \
  --cli .cache/tablecraft-bin/read-query-text-boolean-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs .cache/tablecraft-fixtures/boolean-seeks-V2016-20261025.accdb \
    .cache/tablecraft-fixtures/boolean-seeks-V2019-20261025.accdb \
  --report docs/testing/results/boolean-index-selections-2026-10-25.json
```

The accepted executable SHA-256 is
`013203c54f1235727e73c56f702ed70498acd1ac924a05fc4f46ef350849b8b1`.
The report SHA-256 is
`2e8a0c2218141a5c8fa375683ac38cddfcb52223572ed30adbb7b392ab734817`.
The terminal replay log is `.cache/boolean-selection-replay-final-20261025.log`.
The report retains exact commands, stdout hashes, getter hashes and classified
refusal diagnostics. Existing originals and source pins were rehashed independently
after the successful replay.

## Focused Rust checks

The final batch passes 69 tests: ten engine Boolean-index tests, 56 query retrieval
tests and three new Access selection tests. Four new engine tests cover all four
priority profiles, both directions, projected/zero-width rows, selected retention,
wrong declarations, disjoint branch pruning, key/row disagreement, cancellation
and resource errors. Three new backend-neutral tests preserve residual filters,
nullable Boolean semantics and declined coercive/unsafe hints. The three adapter
tests verify metadata-only preparation, required/automatic routing, failed chosen
indexes and complete-cache isolation. Production Boolean mutation remains blocked.

```sh
cargo test --locked -p tablecraft-ace -p tablecraft-query -p tablecraft-access \
  --test boolean_indexes --test retrieval --test boolean_selection
cargo clippy --locked -p tablecraft-ace -p tablecraft-access -p tablecraft-query \
  --lib --test boolean_indexes --test boolean_selection --test retrieval \
  -- -D warnings
```

Both commands completed successfully. Final logs are
`.cache/boolean-selection-tests-final-20261025.log` and
`.cache/boolean-selection-clippy-current-final-20261025.log`. Owned Rust formatting,
Python compilation and whitespace checks also passed. This was a focused batch;
the full workspace suite was not run for this slice.

An initial Clippy invocation repeated `--lib` and was rejected by Cargo argument
parsing before compilation; its log remains
`.cache/boolean-selection-clippy-20261025.log`. It is excluded from passing checks.
A separate baseline observation with the older saved32-selector executable
(`2762809bf145adcfc3631a713ee36b3008e63503d9c94c19570dec9bc6322d7f`)
refused the true/automatic/cap-900 query with a structural limit and empty stdout.
Its `.cache/boolean-selection-baseline-20261025.{stdout,stderr}` captures remain
preserved and are excluded from the 22 accepted controls.
