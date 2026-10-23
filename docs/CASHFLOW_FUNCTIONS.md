# Bounded periodic cashflow functions

NPV, IRR and MIRR now accept explicit ordered numeric cashflows in
`tablecraft-expressions`, scalar `query eval` and read-only `query run`, including
qualified native saved/named retrievals. They return finite Double results and
share the operation's cumulative allocation/work budget and cancellation callback.
This is a partial expression profile; native Access query admissibility, Variant
array coercion and numerical equivalence remain unqualified.

```sh
tablecraft query eval 'SELECT NPV(.1,Array(-100,0,121)) AS PresentValue'
tablecraft query run 'SELECT IRR(Array(-100,0,121),) AS Rate'
tablecraft query run 'SELECT MIRR(VBA.Array(-100,0,121),.05,.1) AS ModifiedRate'
```

## Inputs and timing

`CashFlows` owns an immutable sequence admitted by the existing checked Double
conversion, including finite numeric/Boolean carriers and legacy DATE serials.
Every zero and position is retained. Its constructor rejects NULL, nonnumeric and
non-finite values rather than dropping them, silently coercing them or changing
periods. It permits an empty sequence as data, but each financial operation rejects
empty input. Debug output reports length only. The constructor accepts at most
65,536 elements, charges the numeric buffer before fallible allocation, and checks
cancellation/work before copying each element and before publication. SQL parsing
and binding retain their existing smaller structural ceilings.

The shared SQL shape checker recognizes only direct `Array(...)` or `VBA.Array(...)`
inputs at the financial function's array position. All elements must be supplied
scalar expressions. Qualified application functions, missing elements, nested
arrays, wildcards and scalar/NULL substitutes for arrays fail. IRR accepts an
omitted final guess, defaulting to 0.1; explicit NULL is an error. The remaining
arguments are required. All written expressions evaluate in written order before
numeric conversion, so an earlier NULL does not hide a later division error.

The explicit array constructor is consumed by the financial call. It cannot be
selected as a scalar result, stored in a field or supplied through a scalar
parameter. General Variant arrays, host array bindings, indexing, module Option Base and
Split/Filter/Join remain unimplemented. [Direct-array inspection and single-dimensional
bounds](VALUE_INSPECTION.md) now work through the separate bounded array input. No VBA
execution or provider lookup is involved. Absolute array lower bounds do not
change the relative financial periods of this explicit sequence.

NPV discounts its first element at period one. A time-zero investment belongs
outside the NPV call when modelling a time-zero initial payment. The current
kernel permits a nonempty single-sign sequence and negative discount bases,
rejecting exactly rate -1. Microsoft's NPV documentation says both signs are
required, whereas its .NET Visual Basic implementation imposes no sign check.
TableCraft's current choice follows the mathematical discount ledger and the
latter reference; the exact VBA/ACE boundary requires native comparison. It is
not claimed as settled compatibility.

IRR and MIRR place the first element at period zero, require at least two positions
and both cashflow signs, and retain trailing zeros in the investment duration.
IRR guesses and MIRR finance/reinvestment rates must be greater than -1.

## Kernels, limits and query ownership

NPV uses normalized compensated Horner summation, retaining multiplication and
addition residuals. A nonzero coefficient lost entirely during normalization fails
with PrecisionLoss; unrepresentable intermediates fail rather than returning
infinity. Extreme cancellation/underflow and exact native rounding are not fully
qualified. This is a Double result, not a stored Currency or Decimal calculation.

IRR shares the [Rate solver](RATE.md): at most twenty guess-directed iterations,
an absolute rate-step tolerance of 1e-7 and a scaled cashflow residual requirement.
Each sample evaluates the ordered polynomial and analytic derivative with bounded
discount factors. Every visited cashflow is metered in addition to solver
checkpoints. Multiple roots remain guess dependent; exhaustion returns Convergence
rather than a last iterate. Ill-conditioned roots and native basin/error equivalence
remain open.

MIRR separately accumulates financed negative present values and reinvested
positive terminal values using logarithmic sums, then takes the duration root.
This avoids overflow from forming large terminal balances before the root.
Every element and final publication is metered. Extreme near-equality precision,
negative-base rates and native rounding/error behavior remain unqualified.

Preparation binds element expressions from metadata without scanning records or
inventing an array field type. The owned query plan stores flattened written
operands with an explicit array-element range; structural grouping equality checks
both operands and range. Normal dependency pruning, grouped expressions,
correlations, aggregate/domain expressions and collection-item walks visit every
operand. Runtime allocates an explicit temporary element buffer without charging
array elements as table rows, then reuses the scalar kernel and shared budget.
Results are buffered through the existing query boundary. UPDATE remains outside
this retrieval-only function profile.

## Evidence and references

Focused kernel controls cover timing, zero positions, compensated cancellation,
multiple roots, empty/single-sign/non-finite inputs, budgets, payload redaction and
every observed IRR checkpoint. Two fixed-seed 256-case properties compare independent
discount/reinvestment ledgers and analytically constructed roots. A separate
period-polynomial control checks IRR values, magnitudes and derivatives on both
sides of zero. Scalar/bound tests cover evaluation order, shape/depth/allocation
limits, metadata-only binding, grouping identity, correlations, reusable
parameters, domain/aggregate composition and late failures.

The original [fixture recipe](testing/SavedCashFlowFixture.java) stores twelve
inert definitions alongside the original forest's fourteen, using reviewed
Jackcess only to construct metadata. The [comparison runner](testing/compare_cashflow_functions.py)
uses independent 120-digit Decimal discounting, reinvestment and bisection
certificates; raw getters and rendered SQL remain separate from formula evaluation.
It passes 280 controls across four original Jet 4/ACE 2007/2016/2019 files with
unchanged bytes. See [fixture provenance](FIXTURES.md#cashflow-query-inputs).
No SQL or formula runs in Jackcess. Microsoft Access was not run; these fixtures
do not prove Access itself accepts Array calls in saved SQL.

Primary references:

- Microsoft [NPV](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/npv-function), [IRR](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/irr-function) and [MIRR](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mirr-function): argument order, cashflow timing and iteration contract.
- Microsoft [Array](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/array-function): Variant arrays, empty constructors and qualified lower-bound distinctions; full Variant behavior is outside the implemented profile.
- [.NET Visual Basic Financial source](https://github.com/dotnet/runtime/blob/main/src/libraries/Microsoft.VisualBasic.Core/src/Microsoft/VisualBasic/Financial.vb): reviewed sign/rate/array boundaries, independently of native VBA/ACE qualification. No source was copied or dependency adopted.

Numeric parameters using the shared conversion kernel now also accept
[legacy DATE serials](DATE_NUMERIC.md), with existing numeric domain/NULL rules.
Extended numerical conversion and complete Variant/native behavior remain open.


## Explicit numeric Text rates and guesses (23 October 2026)

NPV's discount rate, IRR's supplied guess and MIRR's finance/reinvestment rates
now accept numeric Text through the existing checked Double conversion and
execution-local NumberFormat. No machine locale is inferred. The optional IRR
guess remains distinct from NULL and defaults to 0.1 only when omitted.

Conversion order is shared by numeric-only and contextual callers: NPV converts
its rate before constructing cashflows; IRR constructs its array before converting
a supplied guess; MIRR constructs its array before converting finance then
reinvestment rates. Every written expression still evaluates before conversion.
Thus an earlier conversion/array error is not hidden by later normalization, and
a later expression's division error remains eager.

Only scalar control arguments gain Text conversion. Cashflow elements retain the
strict numeric/Boolean-to-Double constructor, including their position/zero and
NULL rules. Text elements are not silently parsed or skipped. Direct Array shape,
publication and native Access query-admission limits remain unchanged. Numeric-only
kernel consumers retain their existing API and reject Text without context.

The internal scalar converter receives the same caller budget and uses only the
explicit numeric format, Text limit and cancellation callback. It invokes no
binding, collation, session or application provider and allocates no duplicate
argument/element list. Direct evaluator cashflow calls now check work/cancellation
at entry; normalization, array construction and solver work retain cumulative
charges. Conversion does not reset or raise any limit.

Two evaluator tests and two query tests cover all three functions, both query
scopes, reusable parameters/formats, validation order, strict array admission,
shared Text limits and every observed Text-path cancellation checkpoint. CLI
controls retain both query commands and buffered failure. The expanded original
[financial Text runner](testing/compare_financial_text.py) passes 174 controls
(126 source-free and 48 native integrations), including independent NPV discount,
IRR root and MIRR endpoint examples plus the existing scalar-financial controls.
Original four-family file hashes and bytes remain unchanged. These engineering
controls do not establish native array admissibility, Variant coercion or Access
numerical equivalence.


## Ordered expression-state admission (23 October 2026)

Cashflow calls now collect scalar controls and explicit array elements as
expression values in written order before conversion. Error data fails
TypeMismatch when its control/element is admitted; it never becomes a numeric
code, zero or a discarded period. Thus NPV(CVErr(1),Array(1/0)) and
IRR(Array(CVErr(1)),1/0) both report DivisionByZero. Later volatile array operands
execute once even if an earlier Error control eventually fails. Operational
failures remain failures and earlier random draws are not rolled back.

Borrowed input accessors preserve the existing conversion order. NPV converts its
rate before constructing numeric cashflows. IRR/MIRR construct the array first,
then convert the guess or financing/reinvestment controls. Array elements remain
ordered and strict: Error/Empty fail TypeMismatch, Missing is non-scalar and NULL
retains InvalidNullConversion. Existing numeric Text array refusal, legacy DATE
conversion and all relative positions/zeros remain unchanged. Explicit numeric
Error-code conversion remains a separate supported operation.

Host Empty scalar controls use zero under the current Double rate/guess profile.
This differs from omitted IRR guess, which retains 0.1. General Variant-array or
Variant-guess qualification remains unfinished; no SQL Empty constructor or
stored array/state carrier is introduced. Query Nz(NULL) still returns Text.
The documented [NPV](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/npv-function),
[IRR](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/irr-function)
and [MIRR](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mirr-function)
references retain the Double cashflow/period contracts and optional-guess default;
the reviewed MS-VBAL Error/Empty coercion rules guide explicit state admission.
Native SQL admissibility and evaluation/error precedence remain unqualified.

Value-only APIs share the same constructor/kernels. Internal accessors borrow
already evaluated payloads. The temporary SQL array now charges its actual
ExpressionValue element width before fallible allocation; the numeric buffer
still charges f64 elements separately. This can exhaust an existing byte allowance
earlier; limits are not raised. Construction, conversion, solver passes and final
publication share the existing work/cancellation budget. Numeric algorithms,
solver limits and formal Double output categories remain unchanged.
