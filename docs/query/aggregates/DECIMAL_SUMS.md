# Wider Decimal query sums

Decimal `Sum` and `DSum` now retain exact totals when an intermediate 96-bit
scalar fold would overflow or discard lower decimal places. The final result
still uses the validated 96-bit, scale 0–28 Decimal carrier. This is the default
engineering numerical profile, independent of `--declared-arithmetic`; it does
not qualify native Access accumulator precision, type promotion or error order.
Microsoft's [Access SQL Sum contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/sum-function-microsoft-access-sql)
establishes numeric query expressions and omitted NULL records, but does not
specify this accumulator implementation.

For example, the Decimal values `MAX_DECIMAL`, `1e-28`, `-MAX_DECIMAL` now sum to
`1e-28` in every encounter order. `MAX_DECIMAL`, `MAX_DECIMAL`, `-MAX_DECIMAL`
returns `MAX_DECIMAL`. Previously the first sequence could lose its residual and
the second could fail on its prefix. A final total that cannot be rounded into
the Decimal carrier still raises Overflow. Ordinary scalar Decimal addition
retains its existing checked arithmetic behavior.

## Shared accumulation and final conversion

The existing wider fixed total used by Decimal means now belongs to the shared
aggregate feature, with both Sum and Avg as real consumers. Lossless narrow
Decimal sums continue using the existing rust_decimal arithmetic path and retain
their coefficient/scale representation. When alignment/addition would lose
information or exceed the stored coefficient, the accumulator keeps an exact
wider coefficient under a bounded common scale. It uses checked i128 first and
the existing BigInt dependency when necessary; no new package or decimal value
implementation is introduced.

A fitting exact final total retains its scale, removing trailing zeroes only
when needed to fit the coefficient. Other totals use the mean's existing
per-candidate-scale half-even conversion with denominator one. Each scale is
rounded independently from the exact total, from 28 down to zero; a previous
rounded candidate never supplies the next. This avoids double rounding. Source
Decimal constraints do not constrain an aggregate result, whose metadata remains
unconstrained Decimal with possible NULL for an empty/all-NULL group.

## Existing boundaries

Only a Decimal-selected accumulation gets the wider Decimal behavior. Integral
and Currency prefixes keep their checked carrier rules, including overflow
before a later Decimal/floating value. An explicit Decimal after a floating
prefix still converts that already rounded Double prefix through the existing
Decimal conversion policy. Once Decimal is selected, later floating values keep
the existing normalized Decimal import. Heterogeneous input order can therefore
still matter; this change does not infer a new common query type or revise
[UNION conversion](../UNION_TYPES.md).

Every aggregate argument evaluates once, in written encounter order. NULL values
are omitted; unsupported carriers, Error data used numerically, conversion errors
and later argument failures remain observable. A later argument failure precedes
the final aggregate range check. In particular, deferred Decimal overflow cannot
hide a later division-by-zero or selected Error operand. An unused IIf branch
still evaluates, and a changing clock is never replayed by the accumulator.

The shared total checks work/cancellation and reserves bounded wide-integer
scratch before allocation. Its width derives from the Decimal coefficient,
maximum scale and checked row count. No second payload snapshot or retained input
vector is introduced. Final scale selection also consumes the shared budget and
can be cancelled. Failed queries remain buffered and return no partial result.

## Verification

Focused controls cover all permutations of large cancellation/residual cases,
signed 96-bit/scale-28 boundaries, positive/negative halfway rounding, final
Overflow, ordinary scale retention, empty/all-NULL input, grouped HAVING,
derived/domain/crosstab consumers, unchanged integral/Currency prefix failures,
late selected Error/division failures, once-only volatile operands and every
observed precision/final-rounding cancellation checkpoint.

The original [Fraction runner](../../testing/compare_decimal_sum.py) constructs typed
source-free SQL values and compares results with exact rational totals and
independent half-even rounding at each candidate scale. It also checks final
range refusals and actual row-three argument failures after an overflowing
Decimal prefix. No native database fixture or Access instance is used.


The recorded [24 October replay](../../testing/results/decimal-sums-2026-10-24.json)
passes 125 Fraction/half-even result controls and five refusals against one
unchanged executable. Source-free inputs are original authored/generated SQL;
no native fixture or Access SQL comparison is claimed. The current focused
Decimal-sum/mean, integral/Currency and floating-sum batch passes 21 tests;
strict query Clippy and formatting also pass.
