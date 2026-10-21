# Bounded annuity rate solving

`Rate(nper,pmt,pv[,fv[,type[,guess]]])` now extends the shared
[financial profile](FINANCIAL_FUNCTIONS.md). Required period count, payment and
present balance convert through the existing numeric/Boolean-to-Double kernel.
Optional future balance and timing default to zero; guess defaults to 0.1.
Three through six arguments are supported. Optional missing slots use their own
defaults; explicit NULL remains InvalidNullConversion. Missing required positions,
wildcards, excess arguments, nonnumeric/non-finite operands and timing other than
exactly 0/1 fail. All supplied SQL expressions evaluate eagerly before conversion.

```sh
tablecraft query eval "SELECT Rate(12,-100,1000) AS PeriodRate"
tablecraft query run "SELECT Rate(12,-100,1000,,,0.05) AS PeriodRate"
```

Microsoft's [Rate reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/rate-function)
defines Double results, a default guess of 0.1, iterative calculation and a limit
of 20 tries. Its stated 0.00001-percent precision corresponds to a rate increment
of 1e-7. TableCraft retains that iteration ceiling and absolute rate-step tolerance,
with an additional cashflow-residual requirement. This does not establish native
algorithm, basin, rounding or failure equivalence.

## Solver and metering

The current solver works in real positive-growth coordinates `x=log1p(rate)`,
with positive periods and rate guesses above -1. It begins at the supplied guess,
uses an analytic Newton step when possible, a previous-point secant/probe otherwise,
and bisects when a proposal leaves an observed sign bracket. Log steps and
representable rate bounds are explicit. There is no automatic restart, unbounded
bracket search or second guess after failure.

Positive log rates use discounted cashflows; negative log rates use undiscounted
cashflows. Geometric payment weights and their derivatives use expm1/log-relative
forms near zero. Cashflows and residual terms are scaled before summation. The
solver evaluates the representable returned rate after log/rate conversion, rather
than qualifying an unreturnable log point. A candidate requires a residual no larger
than 64 Double epsilons times its term magnitude and, after the initial guess,
a rate change no larger than 1e-7. An initially qualifying guess can return directly.
This is a floating numerical certificate, not a proof of a unique root or of
accuracy in every ill-conditioned equation.

All-same-sign/zero cashflows, nonpositive periods, guesses at or below -1 and
recognized one-period identities return Domain. Nonzero input cashflows lost
entirely during normalization return PrecisionLoss; unrepresentable equation
terms return typed range errors. An unqualified candidate after 20 iterations returns
`EvalError::Convergence`, never the final iterate. Other roots may exist; the
supplied guess can select a different root or fail to converge.

`FinancialFunction::apply_with_check` accepts an explicit caller-owned checkpoint.
Rate calls it before each iteration, before success and after iteration exhaustion,
with at most 21 checkpoints. Callback errors propagate without retry. The simpler
`apply` remains available with the same hard iteration ceiling and no host callback.
The scalar evaluator's `apply_financial` composes iteration cancellation/work
charging with its existing ResourceBudget. Bound queries use that same evaluator
bridge and cumulative query budget; no nested budget or public execution is reset.
Existing noniterative financial kernels retain their constant-work behavior.
No schedule/result buffers or runtime providers are introduced.

## Query integration and evidence

Bound financial arguments retain normal pruning, grouping, correlations, aggregate,
domain and collection-item walks. Default guess syntax is copied fallibly and
charged during preparation. Native saved/named SELECT/UNION/fixed crosstab paths
reuse the same executor. Results remain buffered, including late solver failures
and cancellation before publication. UPDATE's separate pure profile is unchanged.

Six new expression controls cover common/zero/negative rates, supplied guesses,
two roots of one equation, ambiguous/impossible targets, explicit non-convergence,
every observed solver checkpoint, optional syntax and per-iteration budgets. One
property uses 256 fixed-seed ordinary ledger cases. An independent integer-period
cashflow polynomial checks the analytic slope on both sides of zero. A new bound
query control checks metadata defaults, reusable parameters, all observed query
cancellation points, cumulative work and no returned iterate on exhaustion.

The original [SavedRateFixture](testing/SavedRateFixture.java) adds eleven inert
queries to the existing fourteen-query forest/data recipe. The
[comparison runner](testing/compare_rate_function.py) passes 294 controls across
scalar/run and raw-getter/rendered/direct/saved/named paths. Independent 120-digit
Decimal ledgers and bisection provide expected roots; scalar coupled-loan cases
also check residuals using the returned rate and payment. Rate comparisons use
1e-9 absolute tolerance, stricter than the documented 1e-7 rate increment; those
cashflow certificates use 1e-10 relative tolerance. Stored cases cover explicit
parameters, grouping, derived reads, UNION, crosstabs, supplied guesses and buffered
late/impossible/non-convergent refusals across four fresh Jet 4/ACE 2007/V2016/V2019
files. All hashes remain unchanged. Jackcess does not execute formulas or queries.

## Remaining qualification

The positive-base solver does not qualify roots/guesses at or below -1, native
multiple-root selection, exact native failure basins, fractional-period/coercion
rules, Variant subtypes, timing/NULL/error order or native precision. Extreme
range and ill-conditioned/cancelling equations can fail even with a mathematical
root. This implementation deliberately uses its own bounded safeguarded algorithm;
a native Access comparison is required before claiming algorithmic equivalence.
Windows execution remains deferred, not a blocker or completed evidence.
