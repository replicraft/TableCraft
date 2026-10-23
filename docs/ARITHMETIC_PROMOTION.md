# Materialized arithmetic result promotion

`tablecraft-expressions::numeric::promoted_arithmetic` now implements an explicit
materialized-value profile for the seven binary arithmetic operators. SQL scalar
evaluation and bound read queries use it after any contextual Text conversion.
`numeric::arithmetic` remains the existing checked Int64/Double kernel for consumers
that need that contract, including fixed-point Sum accumulation. Floating
Sum/DSum use a separate [exact binary total](testing/FLOATING_SUMS.md). Stored schemas,
values, parameters and core representational equality are unchanged.

| Materialized operands | Current result for +, - and * |
| --- | --- |
| Byte / Byte | Byte while in range; then Integer, Long or Double |
| Boolean / Integer with Byte, Boolean or Integer | Integer while in range; then Long or Double |
| Long with narrower integral types or Long | Long while in range; then Double |
| LongLong with any integral type | LongLong; overflow fails |
| Single with Byte, Boolean, Integer or Single | Single, with Double on Single overflow |
| Single with Long/LongLong, or Double with ordinary integers/floats | Double after checked nearest-even operand conversion |
| Currency + or - ordinary numeric | Currency after checked four-place operand conversion |
| Currency * integer or Currency | Currency; checked Decimal product followed by four-place half-even conversion |
| Currency * Single/Double | Double |
| Decimal with numeric or legacy DATE | Checked Decimal operands and result |
| Legacy DATE + or - numeric, without Decimal | Existing serial/date-result profile |
| Legacy DATE * numeric, without Decimal | Double serial product |
| Either admitted scalar NULL | NULL |

Boolean arithmetic uses -1/0 and an Integer result, not Boolean. Widening never
silently narrows an explicitly wider integer operand. Integral overflow beyond
Long promotes directly to Double, not LongLong. The resulting Double may round
the exact integer product; the value remains Double. LongLong overflow fails.
Single operands are evaluated through the existing finite Double kernel and
rounded to Single when its range permits. Gradual underflow and signed zero stay
IEEE values; nonfinite input/result checks remain active.

Currency products reuse rust_decimal. Valid four-place Currency products fit the
intermediate Decimal envelope before the final rounding; products outside the
Currency range fail. Currency/Double operand conversion retains the existing
binary-value rounding policy: the binary64 value spelled 0.00005 lies just above
the exact decimal midpoint. This differs from exact lexical Text conversion and
is covered as a distinct input.

Decimal takes precedence over a legacy DATE operand. Conversion uses the stored
DATE serial, not its civil interpretation. Other DATE operations retain their
existing APIs. Extended DateTime and other unsupported carriers do not become
numeric. Error data, Empty/Missing and arrays do not acquire implicit scalar or
storage representations through this service.

## Boundaries and remaining work

This service uses materialized carrier identities. The query layer now retains
[conservative result metadata](QUERY_RESULT_TYPES.md), including stable successful binary carriers; value-dependent widening remains
Dynamic. This service does not infer a native declared SQL expression type or
implement every static non-Variant overflow rule. Native
Access comparisons are still needed for its query-specific result typing. Integer
literals still materialize as Int64; this change does not reinterpret literal
spelling or suffixes. Existing &H/&O syntax now has its own [bounded signed
radix literal profile](RADIX_LITERALS.md). Unary minus and the division-family profiles are described
below. Unary plus retains its existing identity/conversion profile. Full declared/
Variant promotion and native operator/error equivalence remain backend work.
Double-selected materialized arithmetic now rounds LongLong operands through the
existing explicit converter; strict kernels and key comparison retain their
separate precision guards. [Contextual predicates](NUMERIC_PREDICATES.md#rounded-longlong-predicates)
have their own Double-selected conversion path.

Text conversion remains [execution-local](ARITHMETIC_TEXT.md). For example, a Text
operand converted to Double participates as Double; Text + Text still concatenates.
Metadata-only computed crosstab headings use the same policy-independent arithmetic
without source scans. Aggregate kernels retain their separate result profile:
`Sum` of Byte expressions still accumulates to the existing Int64 result.

The evaluator charges node work and conversions under the same cumulative budgets;
the promotion kernels do not allocate or read sources. Source field policy routing,
filter preservation, typed snapshots and native storage contracts are unchanged.
No runtime host, dependency, crate, serializer or write behavior is added.

```sh
tablecraft query eval "SELECT VarType(CByte(255)+CByte(1)), VarType(CSng(1)+CInt(1))"
tablecraft query run "SELECT Octet+CByte(1), VarType(Octet+CByte(1)) FROM ScalarSamples" --database example.accdb
```

## References and evidence

[MS-VBAL arithmetic](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
defines effective operand types and separate Variant/static overflow branches.
The [addition reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
includes Byte widening; [multiplication](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/917bed61-f19f-4e7d-9f93-2e44d4ec7432)
and [division](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
have operator-specific exceptions. These are implementation references, not native
Access SQL qualification or a claim that the unimplemented branches work.

Four kernel tests cover typed ranges/widening, Single overflow/underflow, fixed
point/DATE selection and a fixed-seed generated independent integer model. Query
and CLI tests verify carriers through derived/grouped/correlated/pivot reads,
inspection and buffered late overflow. A DATE regression now checks serial
multiplication instead of refusing it.

The original [range/type/Fraction runner](testing/compare_arithmetic_promotion.py)
passes 818 controls, including native Byte/Integer/Single field projections over
four unchanged original ScalarFixture files. The existing
[DATE runner](testing/compare_date_arithmetic.py) passes 62 model controls after
updating multiplication to its working profile. Getters only supply raw fields;
only TableCraft executes SQL. Native Access typing, rounding, optimizer and
error-order qualification remain pending.


## Unary minus and division-family selection

`numeric::promoted_negate` now preserves materialized unary-minus carriers. Byte
and Boolean select Integer; negating Integer minimum widens to Long, and negating
Long minimum widens to Double. LongLong and Currency minimum negation overflow.
Single retains Single and signed zero; Decimal and raw DATE keep their checked
owning profiles. SQL numeric Text is converted to Double first. The standalone
`numeric::negate` kernel retains its original contract.

| Operation / operands | Current result |
| --- | --- |
| Division with a Decimal | Decimal through the existing checked conversion/kernel profile |
| Division with Currency or legacy DATE, without Decimal | Double after checked numeric conversion |
| Division with Single and Byte/Boolean/Integer/Single | Single, widening on Single overflow |
| Other admitted numeric division | Double with checked nearest-even operand conversion and finite-result checks |
| Power with Currency or legacy DATE | Double through explicit conversion; other power operands keep the existing Double profile |
| Integer division/Mod with LongLong | LongLong after checked half-even operand conversion |
| Integer division/Mod with Boolean/Integer on the left and a fractional/Text carrier on the right | Integer after checked half-even operand conversion |
| Other integer division/Mod with fractional/Text or Long operands | Long after checked half-even operand conversion |
| Integer division/Mod with Byte/Byte | Byte |
| Other Byte/Boolean/Integer integer division/Mod | Integer |

Fractional carriers here mean Single, Double, Currency, Decimal and legacy DATE.
Target selection uses original operands before Text conversion, including the
left/right distinction in the current documented profile. Operands must each fit
the selected width after rounding even if the eventual quotient would fit. Text
uses the direct lexical checked conversion, preserving digits past a bounded
Decimal's precision. Quotients truncate toward zero; remainders retain the existing
dividend-signed profile. Integer/Long quotient overflow may widen under the
materialized-value rule; LongLong overflow remains an error. These rules do not
establish full native static typing or native error precedence.

NULL propagation, written operand evaluation and buffered failures remain intact.
A later division by zero is not hidden by an outer NULL or unselected eager
branch. Floating-selected zero/zero now fails with Overflow; Decimal division and
integer division/Mod retain DivisionByZero. Strict kernel integer-to-float precision guards are unchanged. DATE conversion
uses stored serials; it does not infer civil elapsed time or extended date arithmetic.

[MS-VBAL unary minus](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/181ac001-7156-4fed-a518-fb90b828ec59),
[division](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
and [integer division/Mod](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/09fdf266-a9d5-4ecb-b7d8-ffd1361cb349)
provide operator-specific input to this profile. Their wider declared-type and
runtime semantics are not claimed as fully implemented.

Two additional kernel tests cover width/minimum boundaries, Single division,
fixed-point/DATE ratios and converted operand range failures. Query and CLI tests
cover resulting carriers, aggregates/subqueries and eager/buffered refusals.
The original [division runner](testing/compare_division_promotion.py) passes 1,040
Fraction/half-even/type/native-field controls over the unchanged priority-family
inputs. Only TableCraft executes SQL; native Access qualification remains unrun.

## Rounded operands for Double-selected arithmetic

Materialized +, - and * with LongLong and Single/Double now convert both operands
to Double before applying the operation. LongLong floating division and power
likewise use that conversion. This reuses the existing checked `Conversion::Double`
implementation, with nearest-even rounding of integers beyond 53 significant bits.
The result is Double. For example, `9007199254740993+0.0` now returns the Double
9007199254740992 instead of PrecisionLoss. Operand rounding precedes the operation;
it is not exact integer arithmetic followed by one final rounding.

Integer-only +, - and * retain exact LongLong arithmetic and checked overflow;
integer division/Mod retain their checked rounded-integer paths. Decimal, Currency
and DATE keep their earlier precedence. NULL propagation, eager expression
failures, nonfinite refusal and result overflow/domain checks remain in force.
No source value or parameter is rewritten. Strict `numeric::arithmetic` and set/group key comparison retain their existing
precision guards. Contextual predicates use their separate
[materialized comparison profile](NUMERIC_PREDICATES.md#rounded-longlong-predicates).
An explicit arithmetic expression can produce rounded keys that group together;
that does not relax comparison of the original stored integers.

Microsoft's [effective arithmetic types](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
and [division types](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
require operand coercion to the selected effective type. They motivate this
materialized profile; they do not establish native Access SQL equivalence.
Existing result metadata already describes these successful cases as Double.

Focused goldens include halfway ties in both directions, signed Int64 endpoints,
all Double-selected operator families, parameter rebinding, derived aggregates,
strict-kernel separation and eager failures. A fixed-seed generated significand
model independently checks integer rounding. The original
[runner](testing/compare_rounded_double.py) passes 176 controls using an independent
integer quantization/IEEE model and raw ID getters from the four unchanged registered
20,000-row inputs. Sequential and both ID-index directions are compared. Only
TableCraft executes SQL. No dependency or native write behavior changes.


## Declared-type boundary

The current arithmetic service receives materialized Value carriers, not an
operand's declared expression type. The query binder infers only successful
carrier guarantees; narrow arithmetic that may widen remains Dynamic. Default execution retains this materialized behavior. The explicit engineering
profile below separately tracks declared Scalar/Variant provenance, without
claiming automatic native SQL declaration classification.

Microsoft's [MS-VBAL arithmetic rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
distinguish declared types from effective runtime types and make overflow depend
on declared Variant participation. These are language-reference inputs, not proof
that Access SQL treats every field or conversion expression identically. The next
promotion work must establish that distinction at the query boundary and qualify
ACE/Jet expression behavior; changing materialized widening or inferring types
from sample rows would not establish declared-type compatibility.


The first working [declared integral API](DECLARED_ARITHMETIC.md) now accepts
explicit scalar/Variant declarations, blocks widening for two declared integral
scalars and preserves the existing materialized profile with a Variant participant.
A metered evaluator entry point and independent probe exercise it. Explicit
`query run --declared-arithmetic` (legacy alias `--declared-integral`) now tracks provenance through fields, parameters,
casts, selectors and nested results. Unknown declarations remain explicit errors
when required; broader inference and native SQL qualification remain open.
Normal SQL dispatch keeps its previous materialized profile.


The broader `declared_numeric_arithmetic` API now enforces Scalar Single overflow
for qualified +, -, *, and / while retaining Variant widening and Double-selected
mixed-operand results. This is part of the explicit query engineering profile;
default materialized promotion, Currency/Decimal range checks and native Access
qualification remain separate. See [Single-selected behavior](DECLARED_ARITHMETIC.md#single-selected-overflow-and-the-numeric-kernel).


## Zero-divisor error selection

The checked floating division kernel now distinguishes zero/zero (Overflow) from
nonzero/zero (DivisionByZero), including either sign of floating zero. Materialized
and explicit declared arithmetic share this classification after their existing
operand conversion and validation. Single/Double, integral, Currency and legacy
DATE ordinary division therefore follow the selected floating rule. Nonzero
subnormal numerators remain nonzero and produce DivisionByZero with a zero divisor.
Non-finite input validation still precedes classification.

Decimal-selected division retains DivisionByZero for every converted zero divisor,
including zero/zero. Integer division and Mod also retain DivisionByZero, including
when half-even conversion rounds a divisor such as 0.5 to zero. This is based on
separate [ordinary division](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
and [integer division/Mod](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/09fdf266-a9d5-4ecb-b7d8-ffd1361cb349)
language rules. The wider Empty-operand exceptions and assignment of nonfinite
Double values before raising an error are not implemented by the scalar service.

NULL operands still propagate after written expressions evaluate. An operand's
operational failure remains eager: `0/(1/0)` fails with DivisionByZero before the
outer division runs, whereas `(0/0)/(1/0)` fails with Overflow in its left operand.
IsError does not turn either operational failure into CVErr data. Aggregates,
domain functions, correlated subqueries and buffered TOP queries retain these
failures rather than filtering them out or publishing earlier rows.

Kernel tests cover carrier pairs, Scalar/Variant combinations, signed zeros,
Decimal scales, subnormals, NULL and nonfinite inputs. Prepared-query tests cover
parameter reuse after each failure class and every ordinary/domain numeric fold
and encounter/extrema/count consumer. The original
[comparison runner](testing/compare_zero_divisors.py) passes 609 explicit operator/type
error expectations in both CLI modes and the declared profile, plus raw Long
getters from unchanged registered 20,000-row priority-family inputs through
sequential and both index directions. The date matrix uses explicit CVar;
unreviewed function declarations retain their existing refusal boundary.
Only TableCraft executes SQL. These checks do not establish native Access error
codes, worksheet-style error cells, build-specific error precedence or optimizer
equivalence. Windows qualification and remaining P0 value semantics stay open.


```sh
python3 docs/testing/compare_zero_divisors.py \
  --cli target/debug/tablecraft --jar path/to/reviewed/jackcess-5.0.4.jar \
  --directory path/to/registered-large-indexes
```
