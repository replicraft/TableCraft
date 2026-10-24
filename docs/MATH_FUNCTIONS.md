# Numeric built-ins

`tablecraft-expressions::functions::math::MathFunction` supplies a closed,
allocation-free API over evaluated scalar values. `query eval` and `query run`
share it; retrieval additionally binds its operands, tracks aggregate membership,
prunes inputs and rewrites correlated slots. No external function resolves here.

| Function | Arguments | Current behavior |
| --- | --- | --- |
| Abs | One numeric/legacy DATE value | Numeric carrier retention; legacy DATE returns Double magnitude |
| Sgn | One numeric/legacy DATE value | Int16 -1, 0 or 1 using exact sign where available |
| Int | One numeric/legacy DATE value | Floor toward negative infinity, retaining numeric or Date carrier |
| Fix | One numeric/legacy DATE value | Truncate toward zero, retaining numeric or Date carrier |
| Round | Numeric value, optional place count | Half-even rounding; default zero places |
| Sqr | One numeric value ≥ 0 | Double square root |
| Exp | One numeric value | Double natural exponential; finite-result check |
| Log | One numeric value > 0 | Double natural logarithm |
| Sin / Cos / Tan | One numeric angle in radians | Double trigonometric result |
| Atn | One numeric ratio | Double principal arctangent in radians |
| RGB | Three numeric channels | Packed Long with upper saturation; see [color contract](COLOR_FUNCTIONS.md) |
| QBColor | One numeric index | Packed Long from the Basic palette; see [color contract](COLOR_FUNCTIONS.md) |

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

The unary magnitude/integral/transcendental functions propagate NULL; RGB/QBColor
reject NULL under their separate color contract. Round evaluates both supplied operands in
written order, validates its place argument, then propagates a NULL first operand.
An explicit NULL place count is InvalidNullConversion. The existing checked Int32
half-even conversion supplies a nonnegative Long count, independently of Decimal
storage scale. Negative counts are Domain errors; conversion outside Long still
overflows. [Higher-precision rounding](#round-precision-beyond-decimal-storage-scale)
uses bounded storage/work rather than a loop proportional to that count. Non-finite inputs,
unsupported carriers under each function profile, invalid arities, missing argument slots
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
native rows with unchanged hashes. Broader implicit locale/date/Variant coercion and native Access qualification
remain open; random and financial calls have separate owning contracts.

## Double-valued mathematical functions

Sqr/Exp/Log/Sin/Cos/Tan/Atn now reuse the existing checked `CDbl` conversion over
numeric/Boolean and [legacy DATE serial operands](DATE_NUMERIC.md). Their
non-NULL result is always Double, including for
Single, integral, Currency and Decimal inputs. Large Int64 values may round at
this explicit function boundary, just as in CDbl; exact integer/Decimal arithmetic
elsewhere retains its existing precision policy. Extended dates and other unsupported
nonnumeric carriers remain rejected; SQL numeric Text uses the explicit policy below. No locale or Variant parser is inferred.

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


## Explicit numeric Text operands (23 October 2026)

All fifteen math identities, including Rnd, now accept numeric Text through the
SQL evaluator's supplied NumberFormat. Value operands convert to Double; Round's
place count uses checked Long conversion; RGB and QBColor use checked
Integer conversion, while Rnd's
control uses Single. Pure numeric APIs retain their host-free conversion boundary
and reject Text without a contextual evaluator. No system locale is inferred.

Round validates the converted count before converting its value. Counts retain
the nonnegative Long range. Numeric Decimal/Currency sources retain their exact
carrier when only the count is Text. RGB validates channels in red/green/blue
order after eager argument evaluation; failed Rnd control conversions do not
advance state. Existing numeric domains, NULL behavior and carrier rules remain.
All valid direct evaluator math calls charge entry work and check cancellation;
Text conversion shares its existing limits and cancellation checkpoints.

Successful Text-source Abs/Int/Fix/Round results have static Double metadata.
Binding derives that type without parsing Text, scanning records or consulting a
provider. This metadata also supplies downstream UNION carrier selection.
Full Variant/Date coercion, native color argument widths, native precision and
error precedence remain unqualified; every function remains Partial.

The original [Text conversion runner](testing/compare_math_text.py) passes 164
controls: 132 source-free eval/run controls and 32 native integrations over the
four registered large-index fixture families. Elementary results, half-even ties
and an independent integer Rnd recurrence supply expected values. Original hashes
are verified before and after; this is not Microsoft Access execution evidence.


## Legacy DATE unary functions (23 October 2026)

Abs/Sgn/Int/Fix now accept a materialized legacy DATE without a format provider.
Abs returns the absolute raw OLE serial as Double; Sgn returns its Integer sign.
Int/Fix apply floor/truncation to the raw Double serial, then reconstruct a checked
legacy Date. Their result carrier remains Date rather than Double. A negative
fraction therefore follows numeric serial rules, independently of its calendar
interpretation: Int(CDate(-0.25)) has serial -1 while Fix has negative-zero serial.
Raw signed zero is preserved by Int/Fix; Abs produces positive zero and Sgn zero.

Reconstruction enforces the existing legacy envelope. Int(CDate(-657434.75))
overflows, while Fix returns the valid minimum whole day. Extended DateTime and
Round's Date input remain unsupported; they need separate contracts. Existing
numeric/Boolean/Decimal/Currency profiles and formal Variant return categories
are unchanged. Static binding exposes Date for Int/Fix Date inputs and Double for
Abs Date inputs without reading records or evaluating expressions.

The reviewed [MS-VBAL specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
and [Fix Date branch](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/97f8abc4-fd64-4ae4-9c80-1548db613ec9)
define Int/Fix Date results through CDbl, the numeric operation and CDate. Abs's
nonnumeric branch selects Double; the Date coercion contract supplies its serial.
The existing numeric Fix operation follows the public Int/Fix reference's
truncation-toward-zero rule; the formal numeric prose's general ceiling description
is not adopted for positive fractions. Native Access comparisons remain unrun.

Three kernel tests include independent integer-quarter models, exact signed-zero
and negative-fraction goldens, envelope checks and distinct Extended/unsupported
refusals. Query controls cover derived results, static metadata, empty subqueries,
UNION carrier selection and eager errors; CLI failures remain buffered. The
[DATE runner](testing/compare_date_numeric.py) adds independent raw-serial math and
VarType controls and four registered native fixture families with unchanged hashes.


## Expression Error/Empty math arguments (23 October 2026)

Scalar and bound math calls now evaluate all supplied expressions into explicit
expression values before kernel admission. Error data is rejected with
TypeMismatch when the kernel requests that operand, without interpreting its code
as a number. Earlier Error data therefore cannot suppress a later operational
argument failure: RGB(CVErr(1),1/0,0) reports DivisionByZero. The same path applies
to derived/correlated operands and Error-valued encounter aggregates.

One internal borrowed argument accessor preserves each kernel's validation order.
Round requests and validates digits before its value, so Round(CVErr(1),-1)
reports Domain. RGB converts and validates red before green and blue, so
RGB(-1,CVErr(1),0) also reports Domain. Rnd requires supplied state before operand
conversion, and failed admission never consumes a draw. Existing numeric-only
APIs and the contextual Text converter share these same kernels. No argument
payload is cloned, cached across calls or coerced to stored NULL.

Host-supplied Empty uses numeric zero for Abs/Sgn/Int/Fix and the existing typed
Double/Long/Single controls of transcendental/color/Rnd functions. Abs/Int/Fix/Sgn
return Integer zero; transcendental results retain Double, including Exp/Cos one
and Log's zero-domain failure. Empty Rnd control repeats rather than advances.
Round's typed digit slot accepts Empty as zero; its Variant value slot remains
explicitly unsupported pending its own qualification. Missing remains non-scalar.
Query Nz(NULL) retains empty Text and supplies no new Empty constructor.

The reviewed [MS-VBAL specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
provides Error Let-coercion refusal (5.5.1.2.9), numeric Empty zero (5.5.1.2.11),
Abs's Integer Empty result and Int/Fix's fallback Integer coercion. Typed controls
retain the existing bounded profiles, including RGB Integer channels and QBColor Integer indexes. Native argument
coercion/evaluation precedence, full Variant behavior and Round Empty value
semantics remain unqualified. No VBA runtime or stored state carrier is introduced.

Math entry checks retain shared work/cancellation and dynamic Rnd repeatability
rules. Operand evaluation stays eager; only already-evaluated data admission is
requested at the relevant validation point. This is not lazy SQL execution.


## Round Long digit controls

Round's optional digit argument now uses the shared explicit Long converter for
all admitted carriers, including legacy DATE. The stored OLE serial is rounded
half-even before checking the nonnegative Long digit range. It does not extract a
calendar component. For example, `Round(2.25,CDate(1))` returns 2.2 and
`Round(2.25,CDate(1.5))` returns 2.25. DATE digit conversion requires no number or
calendar format policy; Text digits retain the explicit NumberFormat requirement.

Digits are still admitted before the value's NULL gate or Error admission;
`Round(NULL,CDate(-1))` and `Round(CVErr(1),CDate(-1))` fail Domain. Written argument
expressions remain eager, including division-by-zero failures. Decimal/Currency
value carriers, existing floating profiles through 28 places, resource controls and the refusal
of DATE in Round's Variant value slot are unchanged. Extended dates remain
unsupported in either slot.

[Access's Round query examples](https://support.microsoft.com/en-us/access/functions/round-function)
establish direct query availability. MS-VBAL revision 3.0 section 6.1.2.10.1.7
declares the digit control As Long; the existing [DATE converter](DATE_NUMERIC.md)
provides its raw-serial conversion. See the [reviewed specification record](VARIANT_ERRORS.md#explicit-numeric-empty-and-cdec-error-rules).
Independent DATE/Decimal controls and query/CLI tests cover this engineering
profile; native Access digit coercion, rounding and error-order qualification
remain pending.

## Round precision beyond Decimal storage scale

Round now accepts every nonnegative Long digit count. The former maximum of 28
was a TableCraft storage-scale restriction, not a qualified Access function
limit. Negative counts still fail Domain after checked Long conversion; NULL,
Error, numeric Text and DATE count admission retain their existing order.
Consequently `Round(NULL,29)` returns NULL, while `Round(CVErr(1),29)` reaches
Error admission and fails TypeMismatch. A negative count still fails before
either source admission.

Fixed integral, Decimal and Currency inputs retain their existing carriers and
scale. rust_decimal already leaves a value unchanged when its stored scale is no
greater than the requested precision; no general decimal implementation was added.
Floating inputs through 28 places retain the prior multiply/half-even/divide
profile. Higher precision uses Rust's locale-independent
[half-even floating formatter](https://doc.rust-lang.org/std/fmt/index.html#precision)
and checked float parsing through a fixed 342-byte stack buffer, covering at most
323 fractional places and 16 integral digits after the existing large-integral
fast path. This avoids overflowing a power-of-ten factor or allocating an output
proportional to an arbitrary Long input. At 324 places or more, half the decimal
quantum is smaller than half the least binary64 subnormal spacing, so the finite
IEEE carrier is unchanged. Signed zero and Single reconstruction retain their bits.

[Access's query examples](https://support.microsoft.com/en-us/access/functions/round-function)
and the reviewed MS-VBAL Round declaration motivate this extension; they do not
establish native results at extreme precision. The original
[independent precision runner](testing/compare_round_precision.py) compares 960
results and Double carriers against exact binary input converted to Python Decimal
with half-even quantization. Subnormals, both signs, signed zero, random finite
bits and maximum Long counts are covered. Native Access precision, admissible
counts, return subtype and error-order qualification remain pending.
