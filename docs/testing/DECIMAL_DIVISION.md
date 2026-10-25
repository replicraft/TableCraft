# Declared mixed Decimal division evidence

The explicit declared arithmetic profile applies the division operator's
effective-type exceptions to a Decimal paired with Double, Currency, Date or
numeric Text, in either operand order. The selected result carrier is Double.
Decimal paired with Single, integral or Decimal values retains Decimal arithmetic.
The ordinary materialized profile remains Decimal-first, including `query eval`.

This follows the operator-specific table in Microsoft's
[MS-VBAL division contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5).
That table takes precedence over the general arithmetic effective-type table.
Microsoft's [numeric Let-coercion contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/56fb78c3-a0ea-4bf0-8ee4-eb5f89bda4f7)
specifies nearest-representable rounding with ties to even for floating conversion.
These language contracts support an explicit engineering profile; they do not
establish native Access query equivalence or change function availability.

## Independent model

[The original Python runner](compare_decimal_division.py) models exact Decimal,
Currency and numeric Text inputs with `Fraction`. It rounds each operand to
binary64 with integer quotient/remainder arithmetic, recovers the exact rational
represented by those bits, divides those rational operands, then independently
rounds the quotient to binary64. It does not use Python floating division to
compute expected results, and it does not import TableCraft's type dispatch.

Eleven independent model controls include common rational goldens, normal and
subnormal boundaries, ties to even, signed zero, maximum finite and overflow.
For example, `CDec('0.3')/CDbl(0.1)` has expected bits `4007ffffffffffff`:
rounding the operands before dividing is distinguishable from rounding their
unconverted exact quotient. A separate exploratory cross-check compared 63
exact-ratio conversions with CPython's `Fraction`-to-float implementation; that
cross-check is not counted as a CLI result.

CLI result checks parse TSV using Python's CSV parser, verify `VarType`, and
recover Double bits from the emitted round-trip decimal representation. Retained
Decimal outputs are compared as exact rational values, independently of textual
scale. String conversion controls supply an explicit number-format policy;
missing policies and invalid text remain errors.

The replay includes both operand orders, explicit `CVar` wrappers, tiny and large
operands, negative zero, subnormal underflow, overflow and division by zero. It
also checks NULL admission, eager operand failures, default `run` and `eval`, and
profile-sensitive UNION, selector and aggregate composition. Positive query and
output limits verify buffered refusal without publishing result prefixes.

An optional pinned prior executable supplies a separate observational default
regression: 120 mixed expressions run under identical materialized-profile flags
on both executables, comparing exit status, stdout and stderr exactly. These
pairs check preservation of established behavior, including existing refusals;
they do not replace the independent expected-bit model or provide Access results.

## Accepted replay

The [2026-10-24 report](results/decimal-division-2026-10-24.json) records 269
independent CLI controls, 120 observational default baseline pairs and eleven
IEEE model controls. The two executable runs for each baseline pair account for
509 total CLI invocations; pairs are not counted as independent model controls.
All 32 pinned executable, runner, production source and manifest hashes remained
unchanged before and after execution.

The candidate executable is the immutable ignored copy
`.cache/tablecraft-bin/decimal-division-20261024`, SHA-256
`8ba31e1725f7c135a766427b2a2366488b22fd68375992c68e81522b3001e602`.
The prior baseline is `.cache/tablecraft-bin/linked-native-cli-final-20261024`,
SHA-256 `820562569b83061a0129c9140ac95f4225849f404c831d4718202c0e40dd1b46`.
The report retains exact runner arguments, each SQL command, exit status, stdout,
stderr and source hashes. Reproduce from the repository root with the recorded
arguments; `--source PATH` repeats to pin additional audited source files.

An initial successful replay with an absolute runner-path inventory was retained
privately and excluded; the accepted replay uses repository-relative paths. No
runtime source or immutable executable changed between those runs. There are no
imported fixture binaries, third-party code or database writes in this source-free
corpus. Windows Access execution remains unrun.
