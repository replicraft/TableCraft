# Fixed-rate financial functions

`Pmt`, `PV` and `FV` now share a closed, allocation-free financial kernel in
`tablecraft-expressions`. They execute in source-free scalar evaluation, bound
retrieval, aggregates, grouped/correlated queries, domain expressions and qualified
native saved/named queries. No financial provider, VBA or application code runs.
[NPer/IPmt/PPmt](PERIOD_PAYMENTS.md) now extend the same dispatch and numerical
profile. [SLN/SYD/DDB](DEPRECIATION.md) add scalar depreciation with a distinct
optional factor default. [Rate](RATE.md) adds bounded iterative solving and explicit
convergence errors. [EuroConvert](EURO_CONVERSION.md) supplies fixed currency
conversion; [NPV/IRR/MIRR](CASHFLOW_FUNCTIONS.md) have a separate bounded array
profile. The [tracker](../ACCESS_BUILTINS.md) retains their individual qualification limits.

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
to Double. [Legacy DATE operands](../dates/DATE_NUMERIC.md) now use their stored serials.
Explicit NULL is InvalidNullConversion. SQL numeric Text now uses the explicit
execution-local NumberFormat at the same checked conversion position. Extended-
date/binary/GUID operands remain unsupported. Standalone numeric kernels still
reject Text without a contextual evaluator. Timing must convert to exactly 0 or 1; other finite values fail
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

The Pmt/PV/FV kernels do bounded constant work and allocate no buffers. SQL callers retain
ordinary cumulative expression/query limits and cancellation checkpoints. Bound
plans preserve argument dependencies through projection pruning, grouping,
correlations and collection-item walks. Results use normal buffered publication;
late errors remain errors even under TOP or an unselected IIf branch. UPDATE does
not gain financial calls in its separate pure editing profile.

## Evidence

Five focused annuity controls include 256 fixed-seed period-by-period ledger
comparisons, numeric carrier/error checks, optional syntax, rate/range boundaries
and source-free evaluation. Three query controls cover pruning, filters, group
identity, correlations, aggregate/domain composition, reusable parameters, eager
errors, cumulative work and every observed cancellation checkpoint.

The original [SavedFinancialFixture](../../testing/SavedFinancialFixture.java) recipe
adds nine inert queries to the existing statistical inputs. The
[comparison runner](../../testing/compare_financial_functions.py) passes 246 independent
controls: 400-digit Decimal cashflow ledgers across rate/period/timing cases,
scalar/run paths, native raw getters and rendered SQL, direct/saved/named execution,
explicit parameters, grouping, derived inputs, UNION, crosstabs and buffered
refusals. Four fresh Jet 4/ACE 2007/V2016/V2019 files retain their hashes.
Jackcess only reads/writes metadata and renders SQL; it does not execute formulas.
Microsoft Access execution on Windows remains deferred.

## Explicit declaration profile

All implemented scalar financial and cashflow functions retain their reviewed
Scalar Double return under `query run --declared-arithmetic`, even when arguments
are Variant. An explicit CVar on the result requests widening. This changes
declaration-aware overflow checks, not the financial kernels or default arithmetic.
See [reviewed signatures and scope](../numeric/DECLARED_ARITHMETIC.md#financial-date-and-inspection-signatures-23-october-2026).


## Contextual numeric Text inputs (23 October 2026)

All eleven currently admitted scalar financial calls now use one internal
conversion/check bridge. Pmt/PV/FV/NPer/IPmt/PPmt, SLN/SYD/DDB and Rate retain
their existing Double input profile. EuroConvert converts only its amount,
full-precision and triangulation slots with their Double/Boolean/Integer targets;
its currency-code strings stay strings. No converted argument list or Text clone
is introduced. Numeric kernel callers retain the host-free conversion context.

The evaluator supplies numeric Text conversion under explicit NumberFormat.
Conversion happens in the kernel's existing order, so an earlier NULL/invalid
numeric operand is not hidden by a later Text normalization failure. EuroConvert
still validates source/target codes before its optional numeric options. All
written SQL expressions remain eager; missing format, malformed Text, range and
operational failures retain explicit errors. No ambient locale is selected.

Direct evaluator calls now check cancellation/work before entering a valid
financial call, including noniterative functions; solver and Euro checkpoints
continue through the same context. This adds one shared entry work step to
existing financial evaluation. Text normalization retains its separate byte/work
charges. Failed calls retain consumed budgets and publish no partial result.

This extends the existing engineering input profile; it does not decide native
period Integer-versus-Double rules, full Variant coercion or native solver basins.
Cashflow-array functions retain their separate [contracts](CASHFLOW_FUNCTIONS.md).
Two evaluator and two query tests cover numeric Text, typed Euro options, all
scalar functions, plan/policy reuse, direct/derived/correlated input, validation
order and cancellation. CLI tests cover both query commands and buffered failure.
The original `testing/compare_financial_text.py` now passes 174 controls (126
source-free and 48 native), including [cashflow scalar Text controls](CASHFLOW_FUNCTIONS.md#explicit-numeric-text-rates-and-guesses-23-october-2026).
Scalar financial cases use independent zero-rate ledgers, elementary depreciation,
identity currency conversion and unchanged registered four-family ID inputs.
Native Access coercion and precision remain unqualified.


## Ordered expression-state admission (23 October 2026)

All eleven scalar financial calls now evaluate every supplied SQL expression into
an explicit expression value before kernel conversion. Error data fails
TypeMismatch at its conversion position; its code is never used as a number
implicitly. Thus SLN(CVErr(1),1/0,10) reports DivisionByZero, and later Rnd
arguments execute once before an earlier Error value is rejected. Operational
failures remain failures and earlier random draws are not rolled back.

One internal borrowed argument accessor preserves written Double conversion order
and EuroConvert's amount, source currency, target currency and option validation.
For example, EuroConvert(1,'USD',CVErr(1)) retains the unsupported-currency Domain
failure before admitting the target Error. NULL still fails conversion, and
scalar numeric algorithms, defaults, exact currency facts and iterative work
checks are unchanged. Existing Value-only APIs share these kernels.

Host Empty maps to zero in numeric controls under the current materialized Double
profile and the typed Euro Boolean/Integer option conversions. It does not supply
an omitted default: explicit Empty triangulation precision is zero and fails the
existing minimum, while omission remains distinct. Empty currency-code positions
and Missing are refused. Query Nz(NULL) retains empty Text and creates no Empty
state. This is not full native parameter-width or Variant qualification; the
existing Pmt-period and other owning profile limits remain open.

The reviewed [MS-VBAL specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
defines Error Let-coercion refusal (5.5.1.2.9) and Empty numeric zero (5.5.1.2.11).
The earlier financial references retain each function's formula/domain guidance.
No VBA runtime, new state carrier, provider or parameter default is introduced.
Native argument-conversion/evaluation precedence remains unqualified.

Entry/iteration/formatting checks share the existing work and cancellation limits.
Argument payloads are not cloned or cached across calls. Binding retains its
metadata-only behavior and formal Double return categories. Cashflow array
admission and its scalar-control handling remain in the separate owning contract.


NPV/IRR/MIRR now have their own [ordered expression-state path](CASHFLOW_FUNCTIONS.md#ordered-expression-state-admission-23-october-2026):
all written controls/elements evaluate before conversion, array states remain
strict and NPV-versus-IRR/MIRR validation order is retained. Their numeric kernels,
period positions and native qualification limits remain separate.

## Retained String evaluator parity

The public retained-value evaluator now accepts valid active String ranges for
all eleven scalar financial functions, including EuroConvert's currency codes and
optional full-precision/triangulation controls. It strictly publishes each reached
String and reuses the same converter or currency parser as materialized Text.
Previously the retained accessor refused every StringBytes input, although the
materialized API accepted its Text equivalent. Ordinary scalar and bound query
paths already admitted valid String operands; this fixes API representation
consistency, without introducing a new query function or numerical profile.

The private argument/conversion context retains each kernel's admission order.
All supplied numeric arguments convert in written order before formula/domain
validation. EuroConvert admits amount, validates source currency, validates target
currency, converts its Boolean option and converts/validates Integer precision,
in that order. Currency codes are not numerically converted and do not require a
NumberFormat when the other inputs are already numeric. Explicit numeric Text
still requires the supplied number policy. Rate's existing solver checkpoints,
convergence/refusal rules and defaults remain intact.

Publication only decodes the selected byte range, rejecting selected unpaired
surrogates and odd byte counts. Invalid discarded backing bytes are ignored. It
uses the same cumulative byte/work budget, cancellation callback and per-Text
ceiling as conversion and solver work; it does not clone an argument vector or
copy a raw backing buffer. Borrowed scalar inputs retain their original carriers.
Reached arguments and conversions execute once. Earlier operational SQL failures
remain eager, including failures in unselected IIf branches. Existing NULL,
Empty/Error/Missing rules are unchanged, including refusal of Empty currency
codes and the distinction between omitted and explicitly zero-valued controls.

Microsoft's [Access SLN reference](https://support.microsoft.com/en-us/access/functions/sln-function)
provides concrete query examples and Double argument/result rules. Its
[EuroConvert reference](https://support.microsoft.com/en-us/access/functions/euroconvert-function)
distinguishes currency String expressions from numeric amount and precision
controls. These support the existing semantic kernels, not a new native retained
representation or native failure precedence. Windows qualification remains open.

Three added financial evaluator tests cover all eleven functions and every
supported optional-argument prefix under two number formats, active ranges,
independently expected conversion/currency failure order, policy separation and
all observed cancellation points plus byte/work/per-Text exhaustion. The
cashflow scalar controls have the corresponding
[three-function contract](CASHFLOW_FUNCTIONS.md#retained-string-scalar-controls).
Two kernel instrumentation tests record exact argument/conversion order and
once-only admission across both families, including Euro's unchanged currency
lookup checkpoints. Two query tests cover 56 successful scalar/bound retained
compositions and 42 malformed/eager-error refusals across all fourteen calls.

The combined focused batch passes 78 distinct tests: 17 financial/cashflow
evaluator tests, 36 financial/cashflow kernel tests and 25 query tests. Strict
scoped Clippy, no-default-features expression compilation, formatting and diff
checks pass. No new CLI replay or native Windows comparison was run for this
API-parity slice.
