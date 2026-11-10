# Power operand and error boundaries

The [accepted replay](../../results/general/power-2026-10-24.json) passes 627 CLI commands
with seven independent model goldens and seven unchanged executable/source
hashes. Production base `80750158` changes one runtime branch: a converted zero
base with a negative exponent now reports Domain. It previously reported
DivisionByZero. Microsoft's [explicit power rule](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/c3f526af-9d58-47f9-a6be-22ab81e8747a)
requires runtime error 5 for that case. This is an operator contract correction;
it does not add VBA execution or a new built-in function.

## Independent model

[compare_power.py](../../access/reading/tables/compare_power.py) uses exact `Fraction` exponentiation after
binary64 operand conversion for bounded integral exponents. The accepted finite
examples use exactly representable dyadic values and powers; seven goldens check
zero-negative refusal, zero-to-zero, negative integral powers, signed underflow,
overflow and an exact finite result. Separate explicit controls cover square
roots and large odd integer/Decimal/Text exponents that round to an even Double
before exponentiation. Successful observations check both result bits and
`VarType` Double.

This is not an independent correctly rounded implementation of arbitrary real
power. It does not establish platform libm equivalence or Microsoft Access
precision. Existing conversion, finite/range, NULL, admission and eager error
contracts remain in force. Actual division-by-zero operands retain that error;
the corrected power error cannot suppress an earlier failure.

The replay includes:

- 390 retrieval commands: fifteen zero-base expressions, thirteen negative
  exponent expressions and both arithmetic profiles. Inputs include integer,
  Boolean, floating, Currency, Decimal, legacy Date and ordinary/raw Text
  carriers, signed zero and a computed underflowed zero.
- 158 additional retrieval controls across both profiles.
- 79 default scalar-eval controls. Scalar eval does not accept the retrieval
  command's `--declared-arithmetic` option.

Controls retain eager error ordering, invalid raw Text, Error admission, NULL,
missing number-format policy, signed zero, finite powers and overflow. All
refusals require exit 2, empty stdout and the expected diagnostic classification.
Successful commands require exact parsed TSV, no diagnostics and the modeled
result. This source-free replay does not read or mutate native databases.

## Rust verification and reproduction

[The verification record](../../results/queries/read-query-power-2026-10-24.json) pins the
complete 1,843-file Rust inventory, source commit and terminal logs. Twenty-two
unique focused Rust tests pass, including thirteen new tests; overlapping reruns
are not added. Strict Clippy covers expression unit-test code and the Power query
target. Normal CLI building and formatting pass.

Query tests cover derived/grouped/UNION/subquery/crosstab consumers, prepared
parameter rebinding, exact cancellation with valid parameters, runtime domain
aggregates and invalid constant crosstab headings during metadata-only preparation
before any source scan. Kernel/evaluator tests retain separate division errors,
finite limits, conversion failures, shared allowances and cancellation checkpoints.

```sh
python3 tests/run.py tests/access/reading/tables/compare_power.py --self-test
python3 tests/run.py tests/access/reading/tables/compare_power.py \
  --cli .cache/tablecraft-bin/power-cli-20261024 \
  --expect-sha256 bde91ce6702cf0b7e6b931024a27c6760e2f6f70aef5699c02e78e83172c9430 \
  --source crates/query/tablecraft-expressions/src/numeric/arithmetic.rs \
  --source crates/query/tablecraft-expressions/src/numeric/arithmetic/power_tests.rs \
  --source crates/query/tablecraft-expressions/src/evaluation/arithmetic.rs \
  --source crates/query/tablecraft-expressions/src/evaluation/arithmetic/power_tests.rs \
  --source crates/query/tablecraft-query/tests/power_boundaries.rs \
  --report .cache/power-new-replay.json
```

The runner refuses an existing report and paths outside the repository. Frozen
executables and terminal logs remain ignored. No third-party implementation or
fixtures are copied; the Python model is original repository code.

The initial query-test compile used a nonexistent test-only error variant and
was corrected before successful checks. An excluded 706-command runner probe
had 82 mismatches caused by an unsupported scalar-eval option and a diagnostic
spelling mismatch. Its report remains under ignored `.cache/`; production was
not changed to satisfy those runner corrections. The accepted replay contains
only supported modes and preserves all intended retrieval profiles.

Full suites, release/performance tests and Windows Access comparisons were not
run. Complete Variant promotion, Error semantics and the remaining read/query
roadmap remain open.

## Windows expression regression (9 November 2026 Pacific)

CI run [42768447941](https://github.com/replicraft/TableCraft/actions/runs/42768447941)
at source `74f1fb66c` ran the full standalone expression suite on native Windows
x64 and ARM64. Both targets passed **419 tests, zero failures and zero ignored**
using `cargo test --lib --bins --tests -p tablecraft-expressions
--no-default-features --locked` with their respective `CARGO_BUILD_TARGET`.
The [x64 job](https://github.com/replicraft/TableCraft/actions/runs/42768447941/job/129670365161)
recorded its result at 06:29:22 UTC on 10 November; the
[ARM64 job](https://github.com/replicraft/TableCraft/actions/runs/42768447941/job/129670365576)
recorded its result at 06:32:08 UTC. These are 9 November Pacific observations.

This verifies the `aa14df718` software-power correction against the retained
signed-zero oracle and adjacent binary64 boundary tests on both Windows targets.
The exact half-minimum-subnormal expectation was not relaxed: odd negative
powers retain negative zero, and representable neighbors retain nonzero
subnormals. The direct `libm::pow` kernel replaces host-CRT power evaluation;
operand admission, domain checks and finite-result handling remain unchanged.

Both overall jobs subsequently failed a separate Windows updater-helper lint
(`HelperPlan.kind` was unused on Windows). This receipt records the successful
expression steps, not a passing overall CI run. Complete job logs were retained
locally as `.cache/main-ci-x64-42768447941.log` and
`.cache/main-ci-arm64-42768447941.log`.

These are Rust expression tests executed on Windows. They do not execute
Microsoft Access, establish its native edge-range rounding, or establish
universally correctly-rounded real exponentiation.
