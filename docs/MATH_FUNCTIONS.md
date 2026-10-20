# Numeric built-ins

`tablecraft-expressions::functions::math::MathFunction` supplies a closed,
allocation-free API over evaluated scalar values. `query eval` and `query run`
share it; retrieval additionally binds its operands, tracks aggregate membership,
prunes inputs and rewrites correlated slots. No external function resolves here.

| Function | Arguments | Current behavior |
| --- | --- | --- |
| Abs | One numeric value | Absolute magnitude with checked carrier retention |
| Sgn | One numeric value | Int16 -1, 0 or 1 using exact sign where available |
| Int | One numeric value | Floor toward negative infinity, retaining numeric carrier |
| Fix | One numeric value | Truncate toward zero, retaining numeric carrier |
| Round | Numeric value, optional place count | Half-even rounding; default zero places |
| Sqr | One numeric value ≥ 0 | Double square root |
| Exp | One numeric value | Double natural exponential; finite-result check |
| Log | One numeric value > 0 | Double natural logarithm |
| Sin / Cos / Tan | One numeric angle in radians | Double trigonometric result |
| Atn | One numeric ratio | Double principal arctangent in radians |

For example, `Int(CDec(-8.4))` returns Decimal -9, `Fix(CDec(-8.4))`
returns Decimal -8, and `Round(CDec(1.25),1)` returns Decimal 1.2.

For Abs/Sgn/Int/Fix/Round, Byte/Int16/Int32/Int64, Single/Double, Currency and Decimal are accepted. Unary
integral operations and Round retain integer widths, including Int64 values above
binary64's exact-integer range. Abs of a signed integer/Currency minimum fails
with Overflow instead of widening. Boolean operands follow the explicit numeric
TRUE=-1/FALSE=0 convention, returning Int16; Abs(TRUE) returns Int16 1. Sgn always
returns Int16. Signed floating zero has sign zero; finite subnormals remain valid.

Decimal Int/Fix use bounded exact coefficient/scale division. Abs retains Decimal
scale; Int/Fix produce scale zero. Round uses the adopted `rust_decimal` library's
half-even operation behind the existing private boundary. It reduces Decimal
scale only when requested and never converts Decimal/Currency through float.
Currency reconstructs the four-place coefficient with checked range/integrality;
floor/round can overflow at a Currency boundary even when the input is valid.
These operations preserve domain constructors and do not add a dependency.

Single/Double Int/Fix/Abs operate on finite IEEE values and retain their carriers.
Round uses a binary64 scaled half-even operation followed by conversion to the
original floating carrier. It preserves already integral large values and returns
the original value when the requested decimal grid is finer than its binary
spacing, avoiding unnecessary multiply/divide changes. This is an explicit
floating policy: native midpoint/subtype/evaluation-order fidelity still needs
Access comparisons. It is not Decimal arithmetic over a float's printed spelling.

All unary functions propagate NULL. Round evaluates both supplied operands in
written order, validates its place argument, then propagates a NULL first operand.
An explicit NULL place count is InvalidNullConversion. The existing checked Int32
half-even conversion supplies the count; the supported range is 0–28, tied to the
Decimal carrier's maximum scale. Negative/greater counts are Domain errors. This
is the current supported bound, not a qualified native maximum. Non-finite inputs,
unsupported Text/date/binary/GUID carriers, invalid arities, missing argument slots
and qualified VBA/UDF calls fail explicitly. Arguments do not short-circuit on
NULL, and retrieval buffers the complete result before publication.

## Evidence and limits

Primary rules come from Microsoft's [Abs](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/abs-function),
[Sgn](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/sgn-function),
[Int/Fix](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/int-fix-functions),
[type-conversion return-type remarks](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
and [Access Round](https://support.microsoft.com/en-us/access/round-function)
references. The [VBA Round examples](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/round-function)
supply midpoint goldens. These references guide implementation; no VBA runtime is
introduced, and documentation is not native Access comparison evidence.

Seven authored/generated value tests cover types, exact large values, minimum
boundaries, NULL/error behavior, midpoint goldens and independent integer models.
A scalar dispatch test and three retrieval tests cover both entry paths, grouping,
aggregates, pruning, correlations, parameter rebinding, crosstab cells, cancellation
and eager errors. The [CLI reference recipe](testing/compare_math_functions.py)
adds independent Python Decimal/integer models and registered sequential/indexed
native rows with unchanged hashes. Random/financial functions,
implicit locale/date/Variant coercion and native Access qualification remain open.

## Double-valued mathematical functions

Sqr/Exp/Log/Sin/Cos/Tan/Atn now reuse the existing checked `CDbl` conversion over
numeric/Boolean operands. Their non-NULL result is always Double, including for
Single, integral, Currency and Decimal inputs. Large Int64 values may round at
this explicit function boundary, just as in CDbl; exact integer/Decimal arithmetic
elsewhere retains its existing precision policy. Text, date and other nonnumeric
carriers remain unsupported. No locale or Variant parser is inferred.

Sqr rejects negative numbers; Log rejects zero and negative numbers with Domain.
Finite inputs whose calculated result is non-finite fail with Overflow. Exp uses
the actual binary64 finite-result boundary, rather than Microsoft's rounded
printed cutoff as a magic constant. Negative exponential underflow may produce
positive zero. Standard IEEE signed-zero behavior is retained, including Sqr(-0).
NaN and infinite inputs fail before calculation. A binary approximation to a
trigonometric pole is evaluated as that finite angle, with no invented pole
snapping. NULL propagates under this explicit query policy; exact native NULL,
error-order and last-bit behavior still need Windows comparison.

The scalar evaluator and retrieval share these kernels through closed numeric
call dispatch. Nested functions, WHERE, grouping, aggregates, correlated inputs
and reusable parameter plans retain their ordinary resource/cancellation checks.
No allocation, random state, clock, host callback or VBA runtime is introduced.

Microsoft documents Double-valued results and domains in
[Sqr](https://support.microsoft.com/en-us/access/functions/sqr-function),
[Exp](https://support.microsoft.com/en-us/access/functions/exp-function),
[Log](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/log-function),
[Sin](https://support.microsoft.com/en-us/access/functions/sin-function),
[Cos](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/cos-function),
[Tan](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/tan-function)
and [Atn](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/atn-function).
These guide the mathematical implementation; they do not qualify native SQL
coercion or platform-specific transcendental precision.

Three value tests include generated trigonometric identities and explicit carrier,
domain, finite/subnormal, NULL, signed-zero, overflow and arity controls. Scalar
dispatch and two retrieval tests cover composition, grouping/pruning, correlations,
parameter rebinding and eager failures. The
[high-precision recipe](testing/compare_transcendental_functions.py) compares
both CLI paths with Python Decimal square root/exponential/logarithm and
independent convergent trig series, using Machin's identity for pi. Its stated
four-ULP bound is a mathematical regression tolerance, not an Access error bound
or proof of cross-platform bit equality. Native Access remains unrun.
