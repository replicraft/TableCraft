# Bounded numeric means

Read-only `Avg` and `DAvg` now share a dedicated mean accumulator. Currency
averaging no longer enters the unsupported legacy Currency division path, and a
mean can succeed when its intermediate integral/Currency sum exceeds the source
carrier. Source values, stored schemas and ordinary `Sum` semantics are unchanged.

NULL inputs do not contribute to the total or count. Empty/all-NULL selections
return NULL. The executor still evaluates each selected operand once, checks
cancellation/work for each row and enforces its existing row/source/output limits.
The accumulator retains one bounded scalar total, not a second record buffer.

## Current result and precision profile

| Observed non-NULL inputs | Accumulation and result |
| --- | --- |
| Ordinary integral/Boolean values | Wider checked Decimal total; Double mean |
| Currency, optionally mixed with ordinary integers | Wider checked Decimal total; Double mean |
| Single/Double without explicit Decimal | Bounded exact binary total once floating input appears; one rounded Double mean |
| Explicit Decimal, including mixed numeric values | Checked Decimal arithmetic and Decimal mean |

The existing scalar conversion, Decimal arithmetic and promoted-division services
own these operations. Their Decimal implementation already uses rust_decimal;
this change adds no dependency or custom decimal arithmetic. Explicit Decimal
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
becomes NULL. The final quotient reuses the checked promoted-division service.

## Limits and remaining compatibility work

The wider exact total is still bounded by Decimal's 96-bit coefficient and scale
0–28. Explicit Decimal totals can overflow even when a mathematical mean would
fit; library arithmetic can round at its established scale boundary. [Exact binary floating means](testing/FLOATING_MEANS.md) now avoid floating-total
overflow and preserve cancellation residuals. Wider Decimal means, full Variant/SQL
result typing and native ACE/Jet rounding/precision qualification remain open.
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
