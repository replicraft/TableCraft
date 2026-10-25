# Original calculated dependency preview replay

The [accepted replay](results/calculated-dependencies-2026-10-24.json) passes 50
commands and compares 170 exact modeled result cells across two original
ACE16-family V2016/V2019 files. All 1,899 pinned hashes remain unchanged before
and after execution, including the complete 1,885-file frozen Rust inventory.
There are 34 successful commands and 16 classified refusals.

This verifies the explicit [same-table dependency preview](../CALCULATION_PREVIEWS.md#same-table-calculated-dependencies)
against ordinary stored getters and an independent model. It does not qualify
Microsoft Access intermediate conversion rules, certify freshness, replace caches
or make calculated columns ordinary query inputs.

## Primary boundary and original producer

Microsoft's native table
[Calculated-data-type training](https://support.microsoft.com/en-us/access/training/video-combine-fields-using-the-calculated-data-type)
explicitly permits calculated results as values in other calculated fields.
That supports a same-table dependency boundary. It does not establish a table
function whitelist or require this preview's natural expression carriers to match
Access's declared intermediate storage conversions. No function call is added.

[CalculatedDependencyFixture.java](CalculatedDependencyFixture.java) creates fresh
files through public Jackcess database/table/column builders. Both `Chains` and
`EmptyChains` have ID, nullable Int32 Input, nullable Short Text Label and ten
calculated declarations. `Chains` has five rows; the other table is empty.
Expression evaluation remains disabled while authoring and reading.

| Calculated declaration | Exact stored expression |
| --- | --- |
| Twice | `[Input]*2` |
| Total | `[twICE]+[Input]` |
| Diamond | `[Twice]+[Total]` |
| Alias | `[Total]` |
| Constant | `7` |
| ConstantAlias | `[Constant]+1` |
| Caption | `[Label]&':'&[Label]` |
| Title | `[Caption]&'!'` |
| Divide | `[Input]\([Input]-2)` |
| LateError | `[Divide]+1` |

The public getter confirms each calculated marker, exact expression, native LONG
or TEXT type and matching ResultType property. Numeric caches are deliberately
555/444/999-style values unrelated to the ordinary inputs; Text caches are
explicit stale strings. Ordinary Input is 3, -4, NULL, 0 and 2. Label includes
accented/supplementary Unicode, empty Text, NULL, tab and quotation-mark data.
No source requires a provider, imported SQL, VBA or reference formula execution.

The original getter reopens read-only, disables expression evaluation, installs a
throwing link resolver and asserts that no linked handle was opened. It records
all public column values separately from their formula declarations. Base64 UTF8
and explicit integer/NULL markers preserve the getter's actual cells without TSV
ambiguity. generation.json records actual producer/getter/compile commands,
compiled-class hash and unchanged original file hashes.

## Independent result model

[compare_calculated_dependencies.py](compare_calculated_dependencies.py) models
expected results directly from ordinary Input/Label getters. It does not parse
stored expressions using TableCraft, execute reference SQL or use cached outputs
as the expected calculation. Numeric expectations are exact Python integer
multiples (2, 3 and 5) or constants; Text expectations are ordinary concatenation.
NULL numeric input propagates independently, while explicit `&` joins NULL Text
as an empty string under the existing preview profile.

Eight admitted fields are previewed in both populated and empty tables for both
profiles. The diamond exercises a shared dependency, mixed-case `twICE` resolves
through the existing identifier policy, aliases retain ordinary leaf dependence,
and constant aliases retain one result per source row. All 80 populated preview
values differ from the raw cached getter. Separate `--stored-calculations` queries
then match all 90 selected ID/cache cells exactly, demonstrating preservation
rather than replacing stored values with the recomputation.

TSV cells expose values, not Int32-versus-Int64 runtime carrier metadata. The
focused library tests separately prove that declared LONG intermediates can
produce natural Int64 preview values. This replay neither infers native carrier
coercion from an integer's printed spelling nor claims an Access equivalent
conversion at intermediate calculated fields.

## Refusals and existing boundaries

`Divide` and `LateError` hit the final Input value 2 and fail with division by
zero. Both commands require empty stdout, preserving buffered preview atomicity
through a nested calculated dependency. No successful prefix is returned.

Positive native-row, native-step and query-step limits of one require classified
runtime resource refusals with no stdout. An output limit of one likewise refuses
without publication. Selecting an ordinary Input as a calculated root remains
unsupported; an unknown target remains an explicit missing-column binding error.
All original input, getter, executable and source hashes remain unchanged.

The six focused dependency integration tests additionally cover preflight cycles,
unsupported transitive expressions, malformed reachable declarations, corrupt
unselected cache payloads, cached-source pruning, expanded expression ceilings,
caller-lowered limits and explicit dependency reporting. Those synthetic negative
controls do not qualify malformed layouts or arbitrary formulas as native valid
calculated fields. Existing cancellation/resource and unsupported-call coverage
continues to apply. No Windows Access oracle has run.

## Reproduction and provenance

The original Java/Python tools follow the repository MIT license. No third-party
fixture bytes or implementation source were copied. Reviewed Jackcess 5.0.4 is an
external Apache-2.0 reference JAR, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Fresh originals and getter stdout/stderr are preserved under ignored
`.cache/tablecraft-fixtures/calculated-dependencies-20261024/`.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/calculated-dependency-classes-20261024 docs/testing/CalculatedDependencyFixture.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/calculated-dependency-classes-20261024 \
  CalculatedDependencyFixture write \
  .cache/tablecraft-fixtures/calculated-dependencies-20261024/V2016.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/calculated-dependency-classes-20261024 \
  CalculatedDependencyFixture read .cache/tablecraft-fixtures/calculated-dependencies-20261024/V2016.accdb
```

Repeat for V2019 and capture getter stdout/stderr beside each original. The writer
refuses existing paths. Fresh timestamps can change regenerated binary hashes;
the accepted report pins the actual produced inputs.

The final immutable CLI is
`.cache/tablecraft-bin/calculation-dependencies-cli-20261024`, SHA-256
`8800b326d94cff1de051bc54edc56cd5b4b8b0ad2fc5d6eaf6c10f501f52d201`.
Its Rust manifest is
`.cache/calculation-dependencies-rust-source-20261024.manifest`, SHA-256
`c29f3fd550682eda70568fa2096b9f9239700e62ee57a7cb354236d256459421`.
The accepted report SHA-256 is
`4a2d8588df9aebbe09cbb9d45265778853bc01a0a0ffde3b177a8e9ade84667d`.

```sh
python3 docs/testing/compare_calculated_dependencies.py \
  --cli .cache/tablecraft-bin/calculation-dependencies-cli-20261024 \
  --cli-sha256 8800b326d94cff1de051bc54edc56cd5b4b8b0ad2fc5d6eaf6c10f501f52d201 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/calculated-dependencies-20261024 \
  --source-manifest .cache/calculation-dependencies-rust-source-20261024.manifest \
  --manifest-sha256 c29f3fd550682eda70568fa2096b9f9239700e62ee57a7cb354236d256459421 \
  --report .cache/calculated-dependency-replay.json
```

The runner refuses report overwrites and checks every live source inventory entry
before and after replay. A later checkout must restore the matching production
revision, `39f35d04a871f7784a62f2624011dae02d2bb08e`, for those checks. The raw
accepted completion log is `.cache/calculated-dependency-final-20261024.log`.
Java compilation, Python syntax and manual line-width/spacing checks passed.

The initial capability producer/getter probe remains separately preserved under
`.cache/calculation-dependencies-probe-20261024/`; its original refusal on the
older CLI is not counted in the accepted 50 commands. Three direct CLI diagnostic
checks established existing refusal wording before the accepted script run; they
are not additional modeled controls. No failed accepted replay was discarded.
