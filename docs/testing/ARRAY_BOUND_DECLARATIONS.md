# Array-bound Long declaration replay

The [accepted source-free replay](results/array-bound-declarations-2026-10-24.json)
passes 77 controls and all 1,867 unchanged hashes, including every entry in the
frozen 1,862-file Rust inventory. The candidate has 40 commands (17 successes,
23 classified refusals); the fixed older CLI has 37 (17 successes, 20 refusals).
Each executable checks 123 modeled result cells.

Six paired cases change from an unknown-declaration refusal to the correct
Scalar Long overflow. The other 31 paired outcomes remain unchanged. Three
additional candidate controls retain positive runtime byte/step/output limits.
This corrects declaration propagation; it does not introduce new array functions,
inputs, native Access query-availability claims or VBA runtime execution.

## Independent signature and arithmetic model

Microsoft documents [LBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/lbound-function)
and [UBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ubound-function)
as returning Long. The existing TableCraft runtime already returns Int32, whose
VarType is 3; the query declaration classifier now records that known Scalar
category instead of leaving it unknown. These signature references do not by
themselves prove direct query availability or native Access behavior. The
[existing inspection profile](../VALUE_INSPECTION.md) retains those scope limits.

[compare_array_bound_declarations.py](compare_array_bound_declarations.py) uses
Python integer arithmetic with signed 32-bit limits. It independently models the
existing zero-based explicit Array profile: lower bound 0 and upper bound
`length - 1`, including -1 for an empty extent. The vectorized signature matrix
covers lengths 0 through 3, both functions, and omitted, explicitly missing and
explicit-one dimensions, with result values and VarType3 under both arithmetic
profiles. Qualified VBA.Array and rounded dimension1.4 controls retain the same
existing input behavior. Zero-base/module and empty-extent native qualification
remain separate.

The six declaration discriminators are direct upper/lower/empty-upper bounds,
a derived field, a scalar subquery and a correlated scalar subquery. Examples:

```sql
SELECT UBound(Array(1,2)) + CLng(2147483647);
SELECT LBound(Array(1,2)) - CLng(-2147483648);
```

With explicit declaration semantics these are Scalar Long operations and must
overflow. The older classifier could not establish that category and refused
with ArithmeticDeclaration. Default arithmetic and explicit CVar wrapping retain
Double widening, checked as exact values plus VarType5. All selected widened
integers lie within exact binary64 representation; the model verifies integer
round trips rather than inferring a carrier from decimal display alone.

## Retained boundaries

UNION, First and Min outputs keep their existing unknown categories even when
their actual values came from array bounds. They continue to refuse the
category-dependent overflow operation; the fix does not infer every downstream
category from its runtime carrier. An empty scalar subquery remains actual NULL.
The runner does not construct a named host source; named propagation and
metadata-only preparation retain their separate Rust integration coverage.

Classified failures cover dimensions zero/two/True, half-even rounding to two,
NULL/nonnumeric/overflowing dimensions, NULL/scalar inputs and eager array-element
errors. UBound(NULL,1/0) retains division error precedence over invalid NULL
conversion. Positive execution bytes/steps and output limits require runtime
resource/output diagnostics and empty stdout. This is proportional regression
evidence, not exhaustive quota/cancellation or native error qualification.

## Artifacts and reproduction

There are no new native fixtures, JARs, external sources or third-party code.
The original runner uses Python's standard library. Its report pins:

- Candidate `.cache/tablecraft-bin/array-bound-declaration-cli-20261024`, SHA-256
  `40fc7f58e5bfa48458762bfcc97b1b1ea3232ac6c7b10d4a16a15f83cbfb292e`.
- Baseline `.cache/tablecraft-bin/concat-policy-cli-20261024`, SHA-256
  `747425183d2e0869646fa359f3f557a540bff6fb835116a8912e6346a38c4cf3`.
- Source `.cache/array-bound-declaration-rust-source-20261024.manifest`, SHA-256
  `8f377ddf7734b22c74ba71f79defbd12ddd116d93c5bbeacb86ddda084e50541`.

```sh
python3 docs/testing/compare_array_bound_declarations.py \
  --cli .cache/tablecraft-bin/array-bound-declaration-cli-20261024 \
  --cli-sha256 40fc7f58e5bfa48458762bfcc97b1b1ea3232ac6c7b10d4a16a15f83cbfb292e \
  --baseline .cache/tablecraft-bin/concat-policy-cli-20261024 \
  --source-manifest .cache/array-bound-declaration-rust-source-20261024.manifest \
  --manifest-sha256 8f377ddf7734b22c74ba71f79defbd12ddd116d93c5bbeacb86ddda084e50541 \
  --report docs/testing/results/array-bound-declarations-2026-10-24.json
```

The runner refuses an existing report, validates every inventory entry before
and after replay, and retains commands, output, diagnostic classifications and
baseline pair counts. The terminal log is
`.cache/array-bound-declaration-final-20261024.log`. A passing exploratory run
remains private and is not added to accepted counts. No failed model cases or
fixture mutations are excluded from the accepted suite.
