# Fixed-rate financial functions

`Pmt`, `PV` and `FV` now share a closed, allocation-free financial kernel in
`tablecraft-expressions`. They execute in source-free scalar evaluation, bound
retrieval, aggregates, grouped/correlated queries, domain expressions and qualified
native saved/named queries. No financial provider, VBA or application code runs.
Other financial functions remain unimplemented in the [tracker](ACCESS_BUILTINS.md).

```sh
tablecraft query eval "SELECT Pmt(.01,12,1000) AS Payment"
tablecraft query run "SELECT FV(.01,12,-100,,1) AS FutureBalance"
```

## Arguments and results

| Function | Required arguments | Optional arguments, default zero |
| --- | --- | --- |
| Pmt | Period rate, periods, present balance | Future balance, timing |
| PV | Period rate, periods, payment | Future balance, timing |
| FV | Period rate, periods, payment | Present balance, timing |

Microsoft documents these functions as returning Double, with timing 0 for
end-of-period payments and 1 for beginning-of-period payments. Cash paid out is
negative and cash received is positive. Rates and periods must use the same time
unit; the functions perform no implicit annual/monthly conversion. References:
[Pmt](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/pmt-function),
[PV](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/pv-function),
[FV](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/fv-function).

The current materialized profile converts numeric/Boolean operands through the
existing checked Double conversion kernel, including Currency/Decimal rounding
to Double. Explicit NULL is InvalidNullConversion. Text/date/binary/GUID operands
are unsupported. Timing must convert to exactly 0 or 1; other finite values fail
Domain. Optional omitted slots default to zero, including an omitted fourth slot
with a supplied fifth argument; explicit NULL is never replaced. Missing required
slots, wildcards, excessive arguments and qualified function names are rejected.
All supplied expressions evaluate in written order before conversion, so an early
NULL does not suppress a later expression error.

Periods currently convert to finite Double without Int16 rounding or a positive
integer restriction. Fractional/negative periods use the real-valued annuity
calculation when defined; native period coercion and admissibility remain pending.
A negative growth base requires integral periods. Zero divisors fail explicitly,
including zero-period Pmt and unavailable PV discount factors. PV/FV at zero
periods return the negated opposite balance. Non-finite inputs and non-finite
results fail rather than publishing NaN/infinity. Floating underflow follows IEEE
behavior. Results are Double, not silently stored Currency/Decimal.

## Numerical and execution boundaries

For rates above -1, logarithmic growth and expm1-based annuity factors avoid
ordinary near-zero-rate cancellation. A relative-factor path retains small-rate
limits even for subnormal rates. PV discounts directly; Pmt chooses a discounted
or undiscounted equation to avoid unnecessary growth overflow. Payment balances
are normalized before addition/division. The zero-rate case has its own exact
linear form. Other real power cases use explicit checked final results.

This is finite Double arithmetic, not arbitrary precision. Extreme intermediate
products/cancellation can still overflow or lose precision even where a more
elaborate scaled calculation could recover a finite result. Full native rounding,
subtype/coercion, negative/fractional-period, timing, NULL and error-order behavior
has not been qualified. Do not infer 100% Access compatibility from the numerical
model controls. No new runtime dependency or ambient locale policy is introduced.

The kernel does bounded constant work and allocates no buffers. SQL callers retain
ordinary cumulative expression/query limits and cancellation checkpoints. Bound
plans preserve argument dependencies through projection pruning, grouping,
correlations and collection-item walks. Results use normal buffered publication;
late errors remain errors even under TOP or an unselected IIf branch. UPDATE does
not gain financial calls in its separate pure editing profile.

## Evidence

Five focused expression controls include 128 generated period-by-period ledger
comparisons, numeric carrier/error checks, optional syntax, rate/range boundaries
and source-free evaluation. Three query controls cover pruning, filters, group
identity, correlations, aggregate/domain composition, reusable parameters, eager
errors, cumulative work and every observed cancellation checkpoint.

The original [SavedFinancialFixture](testing/SavedFinancialFixture.java) recipe
adds nine inert queries to the existing statistical inputs. The
[comparison runner](testing/compare_financial_functions.py) passes 246 independent
controls: 400-digit Decimal cashflow ledgers across rate/period/timing cases,
scalar/run paths, native raw getters and rendered SQL, direct/saved/named execution,
explicit parameters, grouping, derived inputs, UNION, crosstabs and buffered
refusals. Four fresh Jet 4/ACE 2007/V2016/V2019 files retain their hashes.
Jackcess only reads/writes metadata and renders SQL; it does not execute formulas.
Microsoft Access execution on Windows remains deferred.
