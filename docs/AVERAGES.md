# Bounded numeric means

Read-only `Avg` and `DAvg` now share a dedicated mean accumulator. Currency
averaging no longer enters the unsupported legacy Currency division path, and a
mean can succeed when its intermediate integral/Currency sum exceeds the source
carrier. Source values and stored schemas are unchanged. Floating Sum/DSum now
share the [bounded binary accumulator](testing/FLOATING_SUMS.md); fixed-point sums
retain their separate checked fold.

NULL inputs do not contribute to the total or count. Empty/all-NULL selections
return NULL. The executor still evaluates each selected operand once, checks
cancellation/work for each row and enforces its existing row/source/output limits.
The accumulator retains one bounded numeric total, not a second record buffer.

## Current result and precision profile

These rules apply to values actually delivered to the accumulator. A
[UNION result conversion](UNION_TYPES.md#provisional-mixed-numeric-profile) happens
first: a Decimal/Double UNION supplies Double rows and has a Double mean. Dynamic
conditional values can still deliver both original carriers and retain the
Decimal precedence below. Source values and accumulator kernels are unchanged.

| Observed non-NULL inputs | Accumulation and result |
| --- | --- |
| Ordinary integral/Boolean values | Bounded exact decimal total; Double mean |
| Currency, optionally mixed with ordinary integers | Bounded exact decimal total; Double mean |
| Single/Double without explicit Decimal | Bounded exact binary total once floating input appears; one rounded Double mean |
| Explicit Decimal, including mixed numeric values | Exact total with wider fallback; bounded Decimal mean |

The existing scalar conversion, Decimal arithmetic and promoted-division services
own these operations. Their Decimal implementation already uses rust_decimal;
ordinary sums and quotients keep that path. When an exact sum cannot fit it,
a bounded i128/BigInt fallback aligns coefficients without discarding digits.
num-integer supplies quotient/remainder; final wide Decimal quotients use
half-even rounding at the highest scale up to 28 whose coefficient fits 96 bits. Explicit Decimal
retains precedence. Conversion of a previously floating total to Decimal follows
the existing normalized conversion policy. Retained scale is representational:
`1.8750` and `1.875` are not interchangeable core values, even when numerically equal.

Non-Decimal successful means select Double, including Currency means; conversion
can round to binary64. Large integral means no longer require their intermediate
sum to fit Int64 or to be exactly representable as Double. This is the mean's
explicit result conversion, not a relaxation of integer operators, parameter
carriers or stored fields. Returned metadata describes the qualified carrier;
runtime domain targets with unknown expression types can remain Dynamic.

Nonnumeric fields do not acquire implicit numeric/date interpretation. Operational
failures remain failures, including late invalid operands. No failed operand
becomes NULL. Quotients whose totals fit Decimal reuse the checked promoted-division service.

## Limits and remaining compatibility work

Decimal totals can exceed the stored 96-bit envelope: scale alignment and the
checked i64 count bound the fallback coefficient to at most 254 bits. BigInt
scratch is charged before allocation; work and cancellation checks remain explicit.
Results still use the validated 96-bit, scale 0–28 Decimal carrier. Ordinary
rust_decimal quotient behavior, retained scales and conversions remain unchanged.
[Exact binary floating means](testing/FLOATING_MEANS.md) avoid floating-total
overflow and preserve cancellation residuals. Mixed-type conversion order, full
Variant/SQL result typing and native ACE/Jet rounding/precision qualification
remain open. In particular, a floating prefix converted after Decimal appears
still passes through the current bounded scalar conversion policy.
This is a working current profile, not complete native Access aggregate behavior.

Microsoft's [Avg reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/avg-function-microsoft-access-sql)
defines the mean of numeric data with NULL fields excluded, and includes a freight
average example. It does not qualify our precise accumulator/carrier choices.
Windows Access comparisons remain separate from the independent evidence below.

## Evidence

Rust controls reproduce the former UnsupportedNumericCombination and intermediate
Currency overflow failures, then check ordinary/domain means, NULL/empty input,
Currency extremes/cancellation, wider integral totals and mixed Decimal precedence.
Synthetic ACE 16-family native fixtures check Currency data and ordinary/domain
queries with metadata-only binding. The original
[Fraction/CLI runner](testing/compare_currency_average.py) checks 69 exact rational
integer/quarter-unit cases, including values that overflow a Currency sum. It uses
source-free UNION inputs and Python Fraction, not a native Access SQL engine.


[Exact floating means and measured costs](testing/FLOATING_MEANS.md) extend this
profile for pure floating inputs; bounded mixed-type/Decimal and native behavior
remain distinct.

## Wider Decimal totals (22 October 2026)

Focused Avg/DAvg controls cover repeated maximum coefficients at scales 0 and 28,
negative extremes, mixed-scale cancellation down to 1e-28 and positive/negative
halfway rounding. These are independent mathematical controls, not Access results.
The dedicated `decimal_averages` release benchmark includes result destruction and
excludes setup/binding; its ordinary cases are kept separate from overflow goldens.

Six alternating release pairs on this local Darwin/arm64 host measured median
execution of 4,096 rows/one group at 0.910 ms before and 0.939 ms after (about
29 microseconds, 3.2% more). For 2,048 rows/128 groups, medians were 3.109 and
3.097 ms, within the observed spread. No broad speed improvement is claimed.
[Raw samples](testing/results/decimal-means-2026-10-22.json) retain the initial
single runs as well as every paired sample; baseline is `233cd29` with the identical
benchmark harness. Run `cargo bench -p tablecraft-query --bench decimal_averages`.


## Native adapter and independent rational controls

The native adapter regression reuses original synthetic NUMERIC/index fixture
recipes for profile bytes 1, 2, 5 and 6. Each reads nine positive or negative
28-digit coefficients, plus a NULL, at declared scale 0 or 28. Their exact totals
exceed the core carrier. Avg and DAvg return the original coefficient/scale;
preparation reads no user records and retains Decimal result metadata. This
qualifies the tested engine-to-query path, not Microsoft Access SQL behavior.

The original [Fraction/CLI Decimal runner](testing/compare_decimal_average.py)
passes 123 independent controls, including full core maxima, nonterminating
quotients, positive/negative halfway cases, mixed scales, tiny cancellation and
96 deterministic generated groups. Python Fraction computes exact means and
rounds each candidate scale independently with ties to even. It compares numeric
values; retained-scale representation has separate Rust goldens. Run:

```sh
python3 docs/testing/compare_decimal_average.py --cli target/debug/tablecraft
```

No third-party fixture/code or native Access result is introduced. These checks
do not qualify mixed-client behavior, other native field encodings or writes.
