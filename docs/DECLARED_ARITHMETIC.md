# Explicit declarations for arithmetic and comparisons

The expression library provides `declared_integral_arithmetic` and the broader
`declared_numeric_arithmetic` with caller-issued `ArithmeticDeclarations`. Query
execution offers this engineering profile through `query run --declared-arithmetic`;
`--declared-integral` remains a visible compatibility alias. The public
`ExecutionOptions::declared_integral` and worker option retain their historical
field names, covering integral/Single-selected and legacy Date-envelope overflow
and the reviewed
both-Variant String/numeric comparison exception and effective numeric predicate
coercion. Default SQL
execution retains its existing materialized profile. This option and these APIs
do not establish complete native SQL compatibility.

Lossless string operands in [overloaded plus](CONCATENATION.md#lossless-string-addition)
now retain their original declaration categories through numeric Text publication.
String/String joins complete without numeric dispatch; numeric cases return to
the same selected-width/agreement paths. Decoding a raw string does not establish
a Scalar declaration or remove an explicit Variant participant.

## Captured decimal integer literals

The same explicit `query run --declared-arithmetic` profile now captures
unsuffixed decimal integer literals as Integer through magnitude 32767, Long
through magnitude 2147483647, and Double above that boundary. Signs do not extend
a token's positive range: `-32768` is Long and `-2147483648` is Double. Decimal
point and exponent spellings retain Double; existing radix tokens retain their
own profile. Unsuffixed large integers never imply LongLong or Decimal. This
policy therefore rounds `9007199254740993` to binary64 before an enclosing cast.
`CLngLng(9223372036854775807)` overflows after that rounding; an explicitly typed
LongLong parameter or `CLngLng('9223372036854775807')` with an explicit number
format retains the exact value. Explicit Decimal casts and stored declared
carriers retain their existing contracts.

This closes a concrete selected-width gap: `32767+1` now overflows as two Scalar
Integer operands, while `CVar(32767)+1` widens to Long. The resulting declared
carriers and Scalar/Variant provenance propagate through derived and correlated
queries, named discovery, runtime domain preparation and fixed crosstab heading
folding. Closed crosstab headings use the same selected-width arithmetic as their
execution, preserving eager errors before source scans. Safe Integer/Long
comparison constants still permit exact integral scan hints; Double constants
explicitly decline those hints and retain their complete residual filter.

Direct library hosts opt into literal typing with
`PreparationOptions::declared_arithmetic`; preparation captures immutable literal
carriers and output metadata. `prepare()` retains the original Int64 integer
literal profile. Executing such a default-prepared plan with
`ExecutionOptions::declared_integral = true` enables existing declaration-sensitive
operators but does not reinterpret its literals. Conversely, executing an
explicitly typed plan with that execution option disabled preserves its captured
carriers while using the existing materialized operator behavior. Staged
`execute_with_schema_discovery` preparation derives both choices from the one
execution option. Source-free CLI `query eval`, structural TOP counts and ORDER BY
ordinals are unchanged; no VBA suffix grammar is added.

The subtype boundaries are an explicit engineering choice grounded in
[Microsoft's Number Tokens contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/685ad840-accb-4bdb-8bfd-f3d88498547a).
The [Access expression syntax guide](https://support.microsoft.com/en-us/access/guide-to-expression-syntax)
establishes numeric constant syntax but does not establish these query subtype
boundaries. No native Access execution comparison has qualified this policy.
Wide decimal Number tokens retain the existing 128-byte literal ceiling;
numeric carriers are captured directly without allocating a normalized spelling
or introducing extra plan depth. Binding and constant
folding continue to share work, depth, byte and cancellation controls.

`numeric_literals` and the retrieval constraint tests cover threshold carriers,
2^53 and signed i64 boundaries, 29-digit values, typed parameters, captured plan
reuse, explicit casts, selected overflow/widening, crosstabs, cancellation and
unsupported literal lengths. Closed-heading negation is also compared directly
with actual query runtime evaluation for Scalar/Variant Integer and Long minima,
raw Date minimum-plus-fraction and upper-envelope overflow, fractional serials
and both signs of input zero. The current Date policy is stored-serial `0 - DATE`;
both input zero signs yield positive zero, checked by raw serial bits. This does
not make Date-to-heading spelling policy-independent or qualify native behavior.
The independent
[`compare_numeric_literals.py`](testing/compare_numeric_literals.py) runner uses
Python integers and binary64 conversions, rather than production imports.
Its 2026-10-24 run passed 191 value/subtype controls and 6 refusals against a
fixed CLI with SHA-256
`cba6dfcc80424b4eb05c743ceb1c9e33c890b80613485aebb6dd4b96a1e636c5`;
the runner verified its binary hash before and after execution. The complete
current declaration runner also passed all 5,550 source-free controls against
that binary. Its existing exact LongLong operator controls now supply numeric
Text with an explicit `en-us` format, preserving exact operands independently of
the new literal policy; the dedicated literal runner separately checks rounding
and cast overflow of bare large integers. No controls were removed.
The [tracked replay report](testing/results/numeric-literals-2026-10-24.json)
retains the actual terminal summaries, runner/binary hashes and explicitly
post-run source observations; per-case stdout was not retained.

## Operations independent of widening categories

Under the current explicit engineering profile, declaration categories are
required only when they can change selected-width overflow behavior. Mod always
fits its converted operand width; Byte/Byte integer division cannot overflow its
unsigned quotient carrier. Int64 integral addition/subtraction/multiplication,
Int64-selected integer division and Int64 negation retain checked Int64 behavior
in both categories. Byte/Boolean negation always fits its existing Int16 result.
These paths now execute even when their aggregate/UNION provenance is unknown.
Other selected-width operations additionally use the
[per-operation agreement check](#binary-arithmetic-with-unknown-provenance).

The same public `uses_declared_overflow` dispatch predicate now selects contextual
rounded quotient operations for the query layer. It neither converts inputs nor
establishes a declaration. Operand range/rounding, zero divisors, Int64 overflow,
numeric Text policies, cancellation and eager failures retain their owning
kernels. Successful operations leave unknown result provenance unknown; a later
narrow operation or declaration-sensitive comparison can still refuse it.

For example, `Count(*)+CInt(1)` and `First(CInt(-32768)) Mod CInt(-1)` now run under
`--declared-arithmetic`, while `First(CInt(32767))+CInt(1)` still needs an explicit
cast/CVar category. The ordinary Count declaration remains unqualified. This
uses equality of the implemented Scalar/Variant outcomes, not inference of a
declaration from a selected value or a native Access type claim. The arithmetic
and quotient/Mod references below remain the implementation sources; complete native typing and
promotion remain acceptance work.

## Integral kernel input and result profile

`OperandDeclaration::Scalar` means the caller established a non-Variant
declaration; admitted runtime carriers determine this profile's effective width.
`Variant` means a declared Variant currently holds that carrier.
The caller must establish this distinction before evaluating operands; equal
runtime values cannot establish it. Defaults select Variant. `result()` propagates
Variant whenever either operand is Variant, otherwise Scalar. It does not infer
an exact declared type or inspect source records.

The kernel accepts nullable Boolean, Byte, Int16, Int32 and Int64 values. Boolean
numeric values retain the existing true=-1 and false=0 profile. All seven existing
arithmetic operators are available:

| Operation | Two declared scalars | A Variant participant |
| --- | --- | --- |
| +, -, * | Retain the selected integral carrier; overflow cannot widen | Existing materialized widening |
| Integer division | Retain selected integral quotient width; MIN/-1 overflow cannot widen | Existing materialized widening |
| Mod | Retain the converted integral remainder carrier | Existing remainder profile |
| /, ^ | Existing finite Double result profile | Same Double profile |

Byte/Byte selects Byte; Boolean/Integer participation selects Integer unless a
Long or LongLong operand selects that width. The existing materialized profile
widens Byte/Integer to Integer/Long/Double and Long to Double where needed.
LongLong overflow still fails in either declaration category. Existing divide-by-
zero, Double conversion/precision and finite-result rules remain active.

Both input families are validated before NULL propagation. Unsupported nonintegral
input fails TypeMismatch even beside NULL. No Text formatting, Decimal/Currency,
Single/Double input, date, unary or expression-state behavior is inferred by this
integral API. Those wider declaration rules remain acceptance work.

The kernel uses bounded primitive arithmetic without allocation or callbacks.
`Evaluator::apply_declared_integral_arithmetic` additionally checks its host
cancellation callback and consumes one shared work step before the operation.
Failures retain consumed work; no partial result or source mutation is returned.
Hosts evaluate operands and propagate their errors before calling it.

## Engineering probe and verification

The original `tablecraft-expressions` example uses clap's Parser derive:

```sh
cargo run -p tablecraft-expressions --example declared_integral -- \
  --operator add --left i16:32767 --right i16:1 \
  --left-declaration scalar --right-declaration scalar
```

This fails with Overflow. Changing either declaration to `variant` prints
`Long:32768`. The probe accepts explicit bounded integral operands and emits
carrier-tagged results; Double results use exact bits. It opens no database,
parses no SQL and executes no project content. It is separate from the CLI query
protocol; query execution uses the separate explicit option described below.

Tests cover all 65,536 Byte pairs for addition/subtraction/multiplication against
independent wide integer arithmetic and literal width bounds, plus signed limits,
Variant positions, LongLong overflow, quotient overflow, zero divisors, NULL/type
admission, generated Long bounds and evaluator work/cancellation behavior.
The original [Python runner](testing/compare_declared_integral.py) passes 7,728
independent controls using unbounded integer quotient/remainder/bounds and exact
Double bits. Its declaration cases are supplied explicitly, not inferred by SQL.

Microsoft's [MS-VBAL arithmetic rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
separate declared and effective operand types, with Variant-dependent overflow.
They motivate this distinction; this bounded API retains TableCraft's current
materialized integral rules where noted. The reference is not proof that ACE/Jet
SQL classifies every literal, cast, field, selector or aggregate the same way.
Broader static declaration inference, declared result selection, other carrier
families and native Access comparisons remain required before claiming completion.

## Explicit query integration

```sh
tablecraft query run 'SELECT CInt(32767)+CInt(1)' --declared-arithmetic
tablecraft query run 'SELECT CVar(CInt(32767))+CInt(1)' --declared-arithmetic
```

The first fails with overflow; the second returns 32768. This option is an
engineering declaration policy, not automatic native compatibility detection.
Literal values, validated source fields, typed parameters and numeric casts carry
Scalar provenance; CDec, CVar, IIf, Nz, Choose and Switch carry Variant provenance.
Arithmetic combines these categories. Integral unary minus checks the same
selected-width overflow, including negation of the minimum signed value.
Aliases, derived/named outputs, scalar subqueries and correlated references retain
cell provenance without changing stored values or public result carriers.

UNION outputs deliberately have unknown declaration, even when one arm is empty.
Ordinary SQL aggregate results and unclassified functions also remain unknown.
Reviewed domain functions carry their documented Variant return category below.
A selected input row cannot establish a function's declaration. Non-NULL integral-result
arithmetic with unknown categories uses the agreement check below. A widening
result still fails with ArithmeticDeclaration unless a known Variant participant
already establishes widening. An explicit cast or
CVar can supply the caller's intended category. NULL propagation, integral
Double-result division/power and the fixed-width cases below do not require this
width decision.

This profile does not introduce general floating, Currency, Decimal or date
promotion rules; their existing rounded integer-division/Mod conversions participate
through the explicit integration below. Dynamic Eval retains its existing materialized evaluation and
unknown output declaration. Binding performs no row scans to infer categories.
Both written operands retain eager evaluation; division errors, cancellation and
resource exhaustion remain visible. Text comparison policy is tracked separately.
The optional SQLite worker forwards this option and the individual Text allowance
per request, with defaults restored for each subsequent request.

The query suite checks direct/derived/named/correlated values, typed parameter
reuse, unknown UNION/aggregate outputs, eager failures and cancellation. The
original [SQL comparison runner](testing/compare_query_declarations.py) passes
4,954 independent source-free SQL controls with result carrier checks, including
185 fixed-width/range controls. Its optional `--native-directory` mode checks the
registered 20,000-row Jet 4, ACE 2007, ACE 2016 and ACE 2019 fixture images; that
mode was not repeated for the fixed-width dispatch change. These
controls qualify the explicitly selected engineering policy, not native Access.


## Reviewed function return declarations

Function result carriers are not declaration categories. Under the explicit
engineering profile, the following reviewed functions now issue categories from
their documented signatures, independently of their input or selected record:

| Query functions | Declaration | Primary reference |
| --- | --- | --- |
| Len, LenB | Variant containing Long | [MS-VBAL Len/LenB](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/a4d9985a-e2cb-489c-a20a-5da09d77a59e) |
| Asc, AscB, AscW | Scalar Integer | [Asc](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/asc-function) |
| VarType | Scalar Integer | [VarType](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/vartype-function) |
| InStrRev | Scalar Long | [MS-VBAL InStrRev](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9d45a100-3a6d-4cb6-aa09-772ccffe9706) |
| RGB, QBColor | Scalar Long | [RGB](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/rgb-function), [QBColor](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/qbcolor-function) |
| InStr, InStrB | Variant containing Long | [InStr](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instr-function) |
| StrComp | Variant containing Integer | [StrComp](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strcomp-function) |
| Sgn | Variant containing Integer | [Sgn](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/sgn-function) |
| Year | Variant containing Integer | [Year](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/year-function) |
| Month, Day | Variant containing Integer | [Month](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/month-function), [Day](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/day-function) |
| Hour, Minute, Second | Variant containing Integer | [Hour](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/hour-function), [Minute](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minute-function), [Second](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/second-function) |
| Weekday, DatePart | Variant containing Integer | [Weekday](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/weekday-function), [DatePart](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datepart-function) |
| DateDiff | Variant containing Long | [DateDiff](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datediff-function) |
| CDate | Scalar Date | [MS-VBAL CDate/CVDate](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/1f287742-e07f-4169-8ce7-5ddfe0f951fb) |
| CVDate | Variant containing Date | Same CDate/CVDate reference |

These are functions already exposed to queries; no VBA runtime is introduced.
For example, `Asc('A')+CInt(32767)` overflows under this policy, whereas
`Sgn(1)+CInt(32767)` widens to Long. Converting either result to CVar explicitly
requests Variant behavior. NULL results and eager operand errors keep their
existing behavior. The classifications survive derived outputs and do not reuse
successful-carrier inference as declaration inference. Other functions retain
unknown categories until reviewed; this table does not claim native ACE SQL
classification has been established by the VBA language references.

Reverse search and color results retain Scalar even when their arguments carry
Variant provenance; wrapping the result itself with CVar requests widening.
Date function results retain Variant through derived outputs, while an explicit
CInt/CLng result cast replaces that category. Native-field controls check actual
Long ID carriers, Scalar overflow, CVar widening, derived function arithmetic and
unchanged defaults. Every input hash is checked before and after these queries.
The runner executes no writes or database content and does not invoke Access.


## Rounded integer-division and Mod operands

`Evaluator::apply_declared_rounded_arithmetic` accepts the existing contextual
integer-division/Mod input families: integral, Single/Double, Currency/Decimal,
legacy DATE and numeric Text. It selects the existing effective integral width
from the original carriers, converts both operands with checked half-even rounding,
then applies declaration-aware integral arithmetic with their original categories.
This is separate from the pure `declared_integral_arithmetic` kernel, whose input
admission remains nullable integrals only.

Numeric Text uses the evaluator's explicit NumberFormat and exact lexical integer
conversion, without a Double or bounded Decimal intermediate. Legacy DATE uses
its numeric serial and needs no display-format policy. Variant participation does
not relax operand conversion ranges or turn a divisor rounded to zero into a valid
operation. Contextual NULL propagation and written-order eager expression errors
are retained. Unsupported native carriers do not become implicit numbers.

`query run --declared-arithmetic` now uses this service for qualified rounded
operands as well as ordinary integrals. For example,
`CInt(-32768) \ CDbl(-1)` overflows, but either operand wrapped in CVar permits
Long 32768. Operand order retains the existing effective-width distinction:
a narrow Integer on the left with a fractional right operand selects Integer,
whereas a fractional left operand selects Long unless LongLong participates.
Unknown declaration categories still require an explicit caller cast/CVar rather
than inference from the converted value.

Microsoft's [integer division/Mod tables](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/09fdf266-a9d5-4ecb-b7d8-ffd1361cb349)
and [arithmetic overflow rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
distinguish effective width, operand coercion and Variant-dependent overflow.
These references motivate the explicit engineering profile; broader declared SQL
classification, promotion and native Access comparison remain pending.

The metered API checks shared work/cancellation before conversion and retains
charges on failure. Tests cover all admitted carrier families, exact lexical ties,
operand range/zero failures, context reuse and every observed conversion
cancellation checkpoint. The SQL model independently rounds Decimal lexemes to
integers and computes truncated quotient/remainder with unbounded integers.
Native controls also check rounded arithmetic using unchanged stored Long fields.


## Single-selected overflow and the numeric kernel

`declared_numeric_arithmetic` accepts nullable Boolean/Byte/Integer/Long/LongLong,
Single/Double and Currency/Decimal operands. Family validation precedes NULL
propagation. Integrals reuse the existing declaration kernel; rounded numeric
quotient/remainder reuse the existing checked conversion targets. Text remains a
contextual evaluator operation; legacy Date has its separate checked profile.

For +, -, *, and / selecting Single, two Scalar operands cannot widen on overflow.
A Variant participant retains the materialized widening to finite Double. A Single
with Byte/Boolean/Integer/Single selects this branch; Long/LongLong or Double
participation selects Double and does not require a Single-width declaration
decision. Power retains the existing Double profile. Currency, Decimal, LongLong
and Double retain their checked range rules regardless of declaration category.
Signed zero, finite Single rounding and underflow retain their existing behavior.
Floating-selected zero/zero now fails with Overflow independently of declaration
category; Decimal and rounded integer zero divisors retain DivisionByZero. See
[error selection](ARITHMETIC_PROMOTION.md#zero-divisor-error-selection).

`numeric::uses_declared_overflow` shares this dispatch decision with the query
layer; it is not source admission or type inference. The metered
`Evaluator::apply_declared_numeric_arithmetic` checks cancellation and charges
shared work before its allocation-free kernel. Unknown categories fail only when
the qualified selected-width branch requires them; Double-selected operations
retain their existing evaluation. The SQLite worker forwards the same request-local
profile and remains reusable after a refused Scalar overflow.

```sh
tablecraft query run 'SELECT CSng(3.4028234663852886e38)*CInt(2)' --declared-arithmetic
tablecraft query run 'SELECT CVar(CSng(3.4028234663852886e38))*CInt(2)' --declared-arithmetic
```

The first fails with Overflow; the second returns Double. Replacing CInt with CLng
also selects Double independently of the declaration categories. The result
category follows existing CVar/selector/nested provenance, without changing core
stored values or the public integral API. General declared-expression typing,
Error/state promotion, native IEEE/error behavior and full Access qualification
remain acceptance work.

Microsoft's [arithmetic overflow rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
and [division effective types](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
motivate the selected-width distinction. The original
[floating runner](testing/compare_declared_floating.py) passes 1,300 controls:
1,280 exact IEEE carrier/bit checks across signed zeros, subnormal/normal values,
overflow rounding boundaries, four operators and all declaration combinations,
plus 20 controls over raw Jackcess Single getters in unchanged registered native
priority-family files. Only TableCraft evaluates SQL; independent numeric evidence
is separate from native Access execution and locale/build qualification.


## Date conversion return declarations

The resolver now retains separate CDate and CVDate function identities. Their
materialized legacy DATE conversion is identical, but the reviewed language
signatures declare Date and Variant respectively. The query engineering profile
therefore marks CDate Scalar and CVDate Variant, independently of the input's
category. CDate(CVar(x)) is still Scalar; CVar(CDate(x)) requests Variant.
This does not infer native ACE SQL declaration classification or change stored
DATE bits, date/text policies, output types or default arithmetic.

For example, under `--declared-arithmetic`, `CInt(-32768)\CDate(-1)` now reaches
the checked narrow quotient and fails with Overflow; replacing CDate with CVDate
permits Long 32768. Previously both calls had unknown declarations. Explicit CVar
can still request widening; unknown unreviewed functions remain refused when a
width decision needs them. Derived, grouped and correlated results retain the
function category. Both conversions retain their existing NULL failure and eager
argument errors; a zero rounded divisor still produces DivisionByZero.

The initial date-conversion qualification passed 930 independent controls (886 scalar and 44
native source integrations). New cases use independent Decimal half-even rounding
and exact quotient/remainder arithmetic across both functions, all four operand
Variant combinations and explicit narrow overflow/error classes. Registered input
hashes remain unchanged. A separate CVDate replay passes 2,470 Gregorian/Automation
conversion/refusal controls, without new binary fixtures or native Access execution.
Broader declaration inference, other date-returning functions and native
qualification remain P0 work.


## Domain function return declarations

All twelve implemented domain functions have documented Variant returns. The
explicit engineering declaration profile now assigns that category from the
function identity after successful domain evaluation. It does not infer a category
from the selected row, numeric value, source column or underlying SQL aggregate.
The returned cell keeps its existing scalar/Error carrier and text policy.

| Function | Return declaration | Primary reference |
| --- | --- | --- |
| DAvg | Variant | [Microsoft DAvg](https://learn.microsoft.com/en-us/office/vba/api/access.application.davg) |
| DCount | Variant | [Microsoft DCount](https://learn.microsoft.com/en-us/office/vba/api/access.application.dcount) |
| DFirst | Variant | [Microsoft DFirst](https://learn.microsoft.com/en-us/office/vba/api/access.application.dfirst) |
| DLast | Variant | [Microsoft DLast](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlast) |
| DLookup | Variant | [Microsoft DLookup](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlookup) |
| DMin | Variant | [Microsoft DMin](https://learn.microsoft.com/en-us/office/vba/api/access.application.dmin) |
| DMax | Variant | [Microsoft DMax](https://learn.microsoft.com/en-us/office/vba/api/access.application.dmax) |
| DSum | Variant | [Microsoft DSum](https://learn.microsoft.com/en-us/office/vba/api/access.application.dsum) |
| DVar | Variant | [Microsoft DVar](https://learn.microsoft.com/en-us/office/vba/api/access.application.dvar) |
| DVarP | Variant | [Microsoft DVarP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dvarp) |
| DStDev | Variant | [Microsoft DStDev](https://learn.microsoft.com/en-us/office/vba/api/access.application.dstdev) |
| DStDevP | Variant | [Microsoft DStDevP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dstdevp) |


For example, `DMin('Small','T')+CInt(1)` can widen an Integer maximum under
`--declared-arithmetic`; `CInt(DMin('Small','T'))+CInt(1)` still fails with Scalar
overflow. Byte and Single results follow the same distinction. Derived/correlated
outputs and explicit execution-stage discovered domains retain the category.
All domain functions still use their current materialized result profiles: this
does not qualify native Count widths, exact aggregate carriers or encounter order.
Ordinary SQL aggregate declarations remain unknown until separately established.

NULL results still propagate. Error data stays explicit data for its existing
consumers and cannot become an arithmetic error code/NULL. Written operand,
provider, criteria, budget and cancellation failures remain failures. No domain
provider is resolved or scanned merely to classify a result during binding;
required runtime discovery remains explicit. Text-policy provenance is independent
of this return declaration and keeps its existing routing/refusals.

The domain-return qualification passed 1,042 controls: 886 scalar cases and
156 native integrations. New native cases exercise all twelve functions, explicit
cast overflow, NULL and selector returns from Byte/Integer/Single fields. Each
outer read selects ID 1 while selector domains request ID 2, retaining the
separate complete/selected snapshot boundary. Optional `--jar` checks the reviewed
Jackcess artifact hash and independently reads those source carriers; all database
hashes remain unchanged. Only TableCraft executes SQL. These references/models do
not establish full native ACE SQL declaration, precision or optimizer behavior.


## Math signatures and Len/LenB correction

All fifteen implemented MathFunction identities now have reviewed return
categories. The exhaustive math match requires a new function to receive an
explicit declaration decision. Categories describe the function return,
independently of its argument or successful materialized carrier.

| Functions | Declaration |
| --- | --- |
| Abs, Int, Fix, Round, Sgn | Variant |
| Sqr, Exp, Log, Sin, Cos, Tan, Atn | Scalar Double |
| Rnd | Scalar Single |
| RGB, QBColor | Scalar Long |
| Len, LenB | Variant containing Long |

The primary reference is the [published MS-VBAL specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74),
revision 3.0 dated 18 August 2026: sections 6.1.2.10.1.1–11 (math),
6.1.2.3.1.16/18 (Fix/Int), 6.1.2.11.1.22 (Len/LenB), and the existing RGB/QBColor
references above. The reviewed dated DOCX SHA-256 is
`5a844dc540fd44903019c0e2c5dc90d1b402eef642ae41386478add3b591c262`.
The document stays outside the repository; no Microsoft text/fixture binary is
bundled. The explicit [Sqr](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/46c6000d-c501-44f7-9270-60d9a13fc8d4)
and [Rnd](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d6cdff3d-627b-4203-9ea3-bca5def01444)
pages give the Scalar signatures separately.

This corrects the earlier Len/LenB classification: a Long result does not imply a
Scalar Long function declaration. Under this engineering policy,
`Len('a')+CLng(2147483647)` now widens to Double 2147483648;
`CLng(Len('a'))+CLng(2147483647)` retains explicit Scalar overflow. No ordinary
length, byte-string or stored carrier changes. Earlier implementation entries
record the superseded classification, not the current policy.

Likewise, `Abs(CInt(32767))+CInt(1)` can widen, while casting the Abs result to CInt
prevents widening. A CVar argument to Sqr does not change Sqr's Scalar return;
CVar applied to its result does. Rnd returns Scalar Single without adding a draw,
choosing a seed or changing sequence ownership. Grouped, derived and correlated
outputs retain the categories. Existing intrinsic argument conversion, NULL,
domain/range, Error and operational-failure behavior remains in its owning profile.
These signature references do not establish native ACE SQL declaration or full
math/aggregate/Variant compatibility; the engineering flag remains explicit.

## Financial, date and inspection signatures (23 October 2026)

The reviewed declaration policy now covers these already implemented query
functions. Metadata is assigned from the function identity after successful
evaluation, independently of the argument declaration and materialized result.
Feature-local exhaustive matches require a decision when an identity is added;
no rows or providers are consulted to infer the declaration.

| Function identities | Return declaration | Primary reference |
| --- | --- | --- |
| Pmt, PV, FV, NPer, IPmt, PPmt, SLN, SYD, DDB, Rate | Scalar Double | MS-VBAL 3.0 §6.1.2.6.1, individual function declarations |
| NPV, IRR, MIRR | Scalar Double | Same financial declarations, including cashflow-array signatures |
| EuroConvert | Scalar Double | [Microsoft Access Application.EuroConvert](https://learn.microsoft.com/en-us/office/vba/api/access.application.euroconvert), Return value |
| DateAdd, DateSerial, TimeSerial, DateValue, TimeValue | Variant containing Date | MS-VBAL §6.1.2.4.1.1/4/5/11/12; [DateAdd](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/dateadd-function), [DateSerial](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/dateserial-function) |
| Date, Time, Now | Variant containing Date | MS-VBAL §6.1.2.4.2.2–4 |
| Timer | Scalar Single | MS-VBAL §6.1.2.4.2.5; [Microsoft's existing query example](https://support.microsoft.com/en-gb/access/functions/timer-function) |
| IsNumeric, IsDate, IsArray, IsEmpty, IsError | Scalar Boolean | MS-VBAL §6.1.2.7.1.2–5/8 |
| TypeName | Scalar String | MS-VBAL §6.1.2.7.1.12 |

The dated specification and hash are recorded in the preceding math review.
DateAdd and DateSerial omit an As clause in the formal declaration, which means
Variant; their individual Microsoft references also explicitly identify Variant
returns. No function, array operation or VBA runtime is newly exposed by this
change. Timer was already admitted under its documented query profile; its
Scalar Single signature does not establish native tick resolution or invocation
timing. CDate remains Scalar Date, CVDate remains Variant Date, and existing
date components retain their reviewed Variant signatures. VarType remains
Scalar Integer. Explicit result casts replace the returned category as before.

Under `--declared-arithmetic`, `CInt(-32768)\(-Pmt(0,1,-1))` now reports
Overflow rather than an unknown declaration. Applying CVar to the result permits
Long 32768; a CVar argument alone does not change Pmt's Scalar result signature.
Conversely, `CInt(-32768)\DateSerial(1899,12,29)` yields Long 32768; an explicit
CDate cast retains Scalar overflow. TypeName still produces Text: its signature
does not grant a numeric-format provider or make nonnumeric text numeric.

Focused tests retain these distinctions through derived/grouped/correlated
outputs, eager failures, NULL date extraction and one read per clock expression.
At the signature-review checkpoint, the independent runner passed 1,272 controls (1,036 source-free
and 236 native integrations), using fixed clocks, known financial roots and raw
registered native getters with unchanged original hashes. Only TableCraft
executes SQL. The VBA/Access signature references and independent observations
do not establish native ACE SQL overflow/coercion equivalence. Ordinary SQL
aggregate declarations, unreviewed functions/operators, complete Variant
promotion and native text acceptance remain open; defaults retain their existing
materialized arithmetic profile. No dependency or numerical-kernel change.

## Logical operator declaration propagation (23 October 2026)

The explicit declaration profile now retains Scalar/Variant categories through
Not, And, Or, Xor, Eqv and Imp. The primary source is
[MS-VBAL logical operators](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/2d70780d-6e99-47d6-9759-2b3a46b8e862),
§5.6.9.8 in the dated specification recorded above. Its static tables give unary
Variant operands a Variant result and binary operators a Variant result when
either operand is Variant. Qualified concrete scalar operands yield Scalar.
This tracks the category, not full declared subtype inference.

For example, `(Not CByte(0))+CByte(1)` now raises Overflow under the flag, while
`(Not CVar(CByte(0)))+CByte(1)` produces Integer 256. Likewise,
`(CByte(255) And CByte(255))+CByte(1)` retains Scalar overflow; a CVar on either
logical operand permits widening. The categories survive derived, grouped and
correlated outputs. Default arithmetic remains the existing materialized profile.

An unknown category stays unknown unless a known Variant operand establishes the
binary result. Logical operations do not infer declarations from their returned
integer carrier or a selected row. In particular, ordinary Count remains unknown
through Not/And; explicit CVar supplies a category when the caller requests it.

The existing logical kernels still own exact widths, checked fractional/Text
conversion, Boolean/NULL tables and Error refusal. Logical declarations do not
relax Long conversion bounds, invent a number-format policy, promote LongLong
overflow or suppress eager operand failures. Both operands evaluate once; shared
work/cancellation and volatile-state ownership remain in the executor. No new
dependency, second evaluation pass or native format change. Complete runtime
Variant effective-type tables, unknown relational/other expression categories
and native Access SQL qualification remain unfinished.

The extended independent declaration runner now passes 1,644 controls (1,384
source-free and 260 native integrations). Python signed bit operations and
explicit Byte masks model the six operators across Byte/Integer/Long boundaries,
with Scalar and either-side Variant categories. Registered Jet 4/ACE 2007/2016/2019
fields also check logical result overflow, explicit widening and unchanged
defaults. The runner verifies JAR/input hashes and original bytes. Only
TableCraft executes SQL; native Microsoft Access remains unrun.

## Relational result categories (23 October 2026)

The six ordinary comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`) now retain the
reviewed Scalar/Variant result category through bound query composition.
[MS-VBAL §5.6.9.5](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
declares a Boolean result for concrete non-Variant operands and Variant when
either operand is Variant. The existing conservative declaration merge implements
that category: two known Scalars yield Scalar; either known Variant yields
Variant; otherwise the result remains unknown. No row scan or returned Boolean
value supplies a missing declaration.

For example, `CInt(-32768)\(CInt(1)=CInt(1))` now raises Overflow under the
flag. Replacing either comparison operand with CVar permits Long 32768.
Applying CBool to the comparison result restores Scalar overflow. These categories
survive derived, grouped and correlated outputs and subsequent Not/logical
operators. Count and other unreviewed aggregates remain unknown; comparing Count
with a known Scalar does not manufacture a declaration.

CVErr now retains its formal Variant return category (MS-VBAL §6.1.2.3.1,
[CVErr](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/7a7facef-2f52-4d08-918e-63bf6a9261ed)).
A qualified standard Error/Error comparison consequently returns a Variant
Boolean category. Error data remains expression-only; its code never becomes an
implicit arithmetic operand, SQL NULL or stored scalar. Mixed Error/scalar
comparisons and operational failures retain their existing errors.

The existing comparator still owns numeric values, Error pairs, NULL, coercion
and runtime field/text-policy resolution. Both operands evaluate once and a
comparison calls the selected comparer once. Full Variant effective-type tables remain P0. The separately reviewed
both-Variant String/numeric exception below now complements this result-category
policy. LIKE, membership, Between,
SQL-only predicates and other expression categories remain outside this newly
reviewed comparison dispatch. Native Access SQL qualification remains separate.

The independent declaration runner now passes 2,210 controls: 1,806 source-free
and 404 native integrations. Python numeric comparisons model all six operators,
true/false outcomes, both Variant positions, subsequent Not and explicit CBool
casts. Standard Error-code comparisons use the independent unsigned-code model.
Native ID fields exercise the same category decisions across the four registered
Jet 4/ACE families with unchanged original hashes/bytes. Only TableCraft runs SQL.

## Both-Variant String/numeric comparison exception (23 October 2026)

Under `--declared-arithmetic`, when both operands have known Variant declarations
and exactly one is String while the other has a numeric value type, the numeric
operand precedes the String regardless of its spelling. Thus
`CVar(1)=CVar('1')` is False and `CVar(1)<CVar('bad')` is True without a numeric
format or text comparer. Reversing the operands reverses the ordering; equality
is always False and inequality True. This is the specific exception in
[MS-VBAL §5.6.9.5](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8).
The numeric-type list in §5.5.1.2.1 comprises Byte, Integer, Long, LongLong, Single,
Double, Currency and Decimal; Boolean and Date are separate. No text contents are
parsed or reformatted for this exception.

The standalone `comparison::variant_text_numeric` helper requires both caller-issued
declarations, returns None for other pairs and uses no provider/allocation.
Non-finite floating inputs retain explicit NonFinite rejection rather than
acquiring an ordering. Query integration lives at the shared comparison boundary,
so ordinary comparisons, filters, joins, domain criteria, Between and membership/
quantified comparisons use the same rule when their actual cells retain both
Variant categories. Comparisons still evaluate each written operand once. Work
and cancellation are checked before and after publishing an exception result.

One Variant operand alone, Scalar operands and unknown categories cannot
establish this category-order exception. They now use the explicit effective
numeric profile described below for admitted numeric pairs. Unknown UNION/
aggregate declarations are not inferred from returned values. Default queries
keep the existing materialized profile even with two CVar operands. Either date
family, NULL, Error, Text/Text and nonnumeric stored carriers retain their owning
comparison services. The category-order exception adds no set/group/sort key
ordering, locale provider, implicit Error conversion or native write permission. Full
Variant effective-type tables, wider SQL predicate declarations and native
Access SQL comparison qualification remain open.

The independent runner now passes 3,044 controls (2,296 source-free and 748 native
integrations). New cases use independent category ranks across all eight numeric
carriers, both directions, all six operators and empty/numeric/malformed/regional
String spellings. Native cases cover seven field carriers in the registered four
Jet 4/ACE families. Domain criteria count the full 20,000 rows after the outer
query selects ID=1, then correctly find zero numeric/String equalities. Registered
JAR/input hashes and original bytes remain checked. Only TableCraft runs SQL.
A request-local async worker regression alternates the flag without leaking it
between requests, preserving its read-only source. These are engineering/reference
model results, not Microsoft Access execution evidence.

## LIKE and NULL-check declarations (23 October 2026)

LIKE and NOT LIKE now retain the reviewed result category through query
composition. [MS-VBAL §5.6.9.6](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/792d4f80-72c1-49da-98a7-37d5783a8615)
gives two concrete Scalar operands a Boolean result and either Variant operand
a Variant result. Unknown categories retain the existing conservative merge.
For example, `CInt(-32768)\('abc' LIKE 'a*')` reports Overflow under the flag,
whereas wrapping either LIKE operand in CVar permits Long 32768. An explicit
CBool on the LIKE result restores Scalar overflow. This is independent of
ANSI-89 versus ANSI-92 matching and survives derived/grouped/correlated outputs.

IsNull has a Scalar Boolean signature regardless of its argument declaration
(MS-VBAL §6.1.2.7.1.7 and Microsoft's
[Access IsNull reference](https://support.microsoft.com/en-us/access/functions/isnull-function)).
Its existing normalized node also represents SQL IS NULL/IS NOT NULL, so those
forms use the same Scalar policy. Consequently an argument wrapped in CVar does
not grant Variant widening; CVar must wrap the NULL-check result. This remains an
explicit query-profile decision, separate from native Access overflow evidence.
Error data is still distinct from NULL, and argument failures propagate.

The feature-local LIKE adapter reuses the existing matcher and field-policy
routing, evaluating each operand once without a second matcher pass. Required
wildcard modes, explicit collations, NULL propagation, invalid-pattern behavior,
quotas and cancellation remain unchanged. The both-Variant String/numeric
comparison exception does not apply to LIKE: its owning string/pattern service
still evaluates matching. Unknown First/Last or other declarations are not
invented from a returned Text value. No new coercion, provider, dependency or
native-format change. Broader membership/Between/EXISTS declaration policies,
complete Variant tables and native text qualification remain open.

All-feature query tests pass 702 tests including the doctest. Four focused
predicate tests cover both modes, negation, query scopes, explicit casts, unknown
First, NULL/Error distinctions, eager failures, wildcard/pattern refusals and
cancellation. A single-character callback control confirms one comparer call.
The independent runner now passes 3,278 controls (2,474 source-free and 804 native
integrations). Independently chosen ASCII pattern/truth cases and native Label
fields exercise both modes, either Variant position and unchanged defaults;
NULL checks cover the shared function/operator node. Existing JAR/input hashes
and original bytes remain verified. Only TableCraft executes SQL, without native
Microsoft Access execution or new pattern/locale compatibility claims.

## List, range and subquery result policies (23 October 2026)

The explicit declaration profile now propagates categories through IN/NOT IN
lists and BETWEEN/NOT BETWEEN. Their result category composes the categories
of every written operand using the existing relational and logical rules. Two
(or more) known Scalars remain Scalar; any known Variant establishes Variant;
otherwise unknown stays unknown. This is a composition-derived engineering
policy, not a captured native Access declaration signature. Microsoft's
[IN reference](https://support.microsoft.com/en-us/access/in-operator) defines
equality-based list membership, the
[BETWEEN reference](https://support.microsoft.com/en-us/access/between-and-operator)
defines inclusive ranges and NULL propagation, and the reviewed MS-VBAL
comparison/logical tables above supply the category rules.

For example, `CInt(-32768)\(CInt(1) IN (CInt(1),CInt(2)))` retains Scalar
overflow. Making even the nonmatching final item CVar permits Long 32768. A
Variant in any written BETWEEN operand likewise establishes Variant, including
reversed bounds. Explicit CBool on either predicate restores Scalar overflow.
The existing equality accumulator and four range comparisons retain their order,
NULL/Error behavior and eager failures. Operands evaluate once, including random
calls; declaration accumulation retains no extra row/value payload.

EXISTS/NOT EXISTS use a fixed Scalar Boolean policy, independently of projected
value categories or whether any rows are returned. Microsoft's
[Access subquery reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
defines the existence test and IN/ANY/SOME/ALL syntax. It does not establish native
Scalar/Variant overflow behavior; the fixed category is an explicit engineering
policy. Existing bounded materialization remains eager, including projection
errors; this does not add an EXISTS short circuit or query optimizer.

For IN subqueries and quantified comparisons, the left operand category now
combines with the statically bound right output category, including empty
results. Any known Variant establishes Variant; two known Scalars establish
Scalar; remaining cases stay unknown. No returned row supplies declaration proof.
The shared adapter retains every comparison, runtime text policy and NULL/empty
identity. The static metadata implementation below supersedes the earlier
known-Variant-left-only admission; complete Variant semantics and native SQL
qualification remain open.

The independent runner passes 4,200 controls (3,308 source-free and 892 native
integrations), including all operand-category combinations for list/range
predicates, reversed/NULL bounds and independently modeled empty/populated
ANY/SOME/ALL sets across all six comparisons. Native cases reuse the registered
four-family inputs, testing existence, list/range categories and known-Variant
subquery predicates without changing original bytes. Four focused query tests
retain eager late failures, Error data, unknown categories, query scopes, empty
results, once-only random operands and cancellation. Native Access declaration/
optimizer equivalence remains unqualified.


## Static query-output categories (23 October 2026)

Binding now records an optional declaration category for each relational slot
and result column. A bounded expression walk combines stored scalar metadata,
explicit conversions and the shared reviewed function signatures. Derived,
named, joined and correlated sources retain these categories. Preparation charges
metadata allocation and expression visits against existing limits; it evaluates
no values, reads no records and invokes no clock or formatting providers.

Scalar subqueries use their static output category whether they return one row
or NULL for an empty result. IN/ANY/SOME/ALL use the same right output metadata.
For example, `CInt(-32768)\(CInt(1)=ALL(SELECT CInt(1) WHERE False))`
reports Scalar overflow, while changing the projected expression to CVar permits
Long 32768. The empty-set truth identity is unchanged. Declaration categories
remain separate from runtime carriers, nullability and text policy.

UNION outputs and unreviewed SQL aggregate/function returns remain unknown;
no populated arm or observed result supplies a guessed category. Named crosstab
group columns retain their underlying static categories, including empty inputs;
pivot aggregate cells remain unknown. Binding and execution share one pure
reviewed-signature classifier under the plan domain.

Four focused query tests cover empty/populated, derived/joined/named/correlated
sources, empty crosstab groups and conservative UNION/aggregate outputs. A scan
counter verifies that preparation reads no table records. CLI controls exercise
both flag names and buffered failures. The independent runner passes 4,258
controls (3,350 source-free and 908 native integrations), retaining original
registered hashes and bytes. These are engineering category policies and
independent results; native Access qualification and full effective-type tables
remain open. Default arithmetic remains the existing materialized profile.


## Declared legacy Date overflow (23 October 2026)

The explicit numeric kernel now admits legacy Date carriers. For Date-selected
addition and subtraction, two Scalar declarations prohibit widening outside the
stored Automation Date envelope; a Variant participant permits the existing
finite Double result. Unary minus applies the same rule. This follows the
[MS-VBAL arithmetic overflow table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
and the unary/addition/subtraction rules in the dated specification recorded
above. Date-minus-Date selects Double directly, so an out-of-Date-range difference
is not a Date overflow. Decimal participation keeps the existing Decimal path.

For example, `CDate(2958465)+1` and `-CDate(2958465)` now report Overflow
under `--declared-arithmetic`; replacing CDate with CVDate permits Double
2958466 and -2958465 respectively. In-range results retain their Date carrier.
Defaults retain the existing materialized widening policy. Numeric kernels remain
allocation-free, evaluator integration checks shared cancellation/work and
operands remain eager. Stored values, calendar conversion and formatting are
unchanged. Text coercion, Extended Date and complete declared/effective-type
selection remain separate work; this does not establish native Access behavior.

Kernel tests cover all four operand-category pairs, operand order, both range
ends, unary-equivalent subtraction, in-range results, NULL and Date-minus-Date.
Query tests cover CDate/CVDate, CVar on either operand, scalar subqueries and
unchanged defaults. CLI tests retain buffered failures and both flag names.
The independent runner adds 16 controls using exact integer serial arithmetic
and explicit carrier checks, without deriving expected results from TableCraft.


## Contextual numeric Text and Date declarations (23 October 2026)

Date-selected addition/subtraction with numeric Text now enters the explicit
arithmetic profile before contextual conversion. The dispatch predicate admits
these pairs; the standalone numeric kernel continues to reject Text. The
`Evaluator::apply_declared_numeric_arithmetic` boundary shares the existing
operator-specific conversion path with default arithmetic and preserves original
caller-issued declaration categories after conversion. No host locale is inferred.

With `--number-format de-de --declared-arithmetic`,
`CDate(2958465)+'1,0'` reports Overflow, while
`CDate(2958465)+CVar('1,0')` returns Double 2958466. Both operand orders retain
the same rules. Missing number-format policy remains an explicit error; invalid
numeric Text remains TypeMismatch. Contextual NULL propagation, eager argument
failures, cancellation and conversion budgeting retain their existing boundaries.
Non-Text calls retain standalone kernel validation; defaults keep their previous
arithmetic profile. The public method is a numeric operation API, not the SQL
string-concatenation dispatcher.

The existing [MS-VBAL arithmetic tables](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
include Date/String effective Date and declaration-sensitive overflow. The local
number-format admission remains explicit TableCraft policy. Added tests cover
category retention, both operand orders, missing/invalid policies, NULL,
cancellation, query composition and CLI buffering. Twenty independent controls
use exact integer serials under two explicit numeric formats, checking all four
category pairs, missing-policy refusals and unchanged defaults. Extended Date,
general effective-type tables and native Access qualification remain open.


## Effective numeric predicate types (23 October 2026)

[Retained String predicates](RAW_PREDICATES.md) now preserve NULL truth without
decoding. The both-known-Variant String/numeric exception also recognizes raw
String identity without inspecting contents; declarations remain caller-issued.
Other raw operands publish strictly before the existing effective-type service.

The explicit query profile now selects the reviewed effective numeric type before
ordinary predicate comparison, rather than always parsing an integral/Text pair
as Double. `Evaluator::compare_effective_numeric_values` exposes the policy
without requiring a SQL parser or inferring declaration categories. It uses
existing checked conversions and explicit NumberFormat, then compares converted
values. NULL yields Unknown before conversion. The owning
[MS-VBAL relational table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
is separate from arithmetic type selection.

| Participating ordinary numeric types | Selected comparison type |
| --- | --- |
| Decimal, including legacy Date paired with Decimal | Decimal |
| Legacy Date and admitted numeric types without Decimal | Date |
| Currency, without Decimal | Currency |
| Single and Long/LongLong | Double |
| Single and Byte/Boolean/Integer/Single/Double/Text | Single |
| Double without Single/Currency/Decimal | Double |
| LongLong without floating/fixed types | LongLong |
| Long without wider types | Long |
| Integer, or Boolean paired with Byte | Integer |
| Boolean paired with Boolean/Text | Boolean |
| Byte paired with Byte/Text | Byte |

Numeric Text converts directly to the selected carrier, retaining half-even
integral rounding and range checking. Thus `CInt(2)='1.5'` is True in this profile,
while `CByte(1)='256'` reports Overflow. Int64/Text comparisons retain exact integer
precision. Single/Double comparisons check Single range; Single/Long selects
Double. Missing numeric policy, invalid Text and non-finite values retain checked
errors. These are observable changes only under the explicit flag.

The existing both-known-Variant String/numeric exception runs before effective
coercion; it neither parses Text nor invokes a provider. Unknown declaration
categories cannot establish that exception. The effective numeric table itself
uses runtime carrier types independently of declaration categories. Scalar
comparisons, filtering, joins, domain criteria, list/range predicates and
quantified subqueries share the predicate path. Eager operand/list evaluation,
Error/state handling, text policy routing and cancellation remain separate.

Date/Text and Date/Date, Extended Date, GUID, binary and Text/Text pairs retain
their owning comparison profiles. The Date/numeric extension below adds checked
Date selection and Decimal precedence; this table does not admit Empty.
Default comparisons, DISTINCT/group/sort key equivalence and stored values are
unchanged. Complete Variant/static type inference, native mixed text policies
and Access qualification remain open. Comparison work and contextual conversion
consume the existing execution budget; binding performs no evaluation.

Three focused evaluator tests cover selected-width rounding/range, exact Int64,
floating/fixed cases and policy/NULL/cancellation boundaries. Two query tests
cover shared consumers, declaration exceptions, eager failures and unchanged
defaults/key refusals; CLI tests cover both flags and buffered overflow. The
independent runner adds exact Decimal half-even integral/Text boundary models
for all six comparisons in both operand orders, independent IEEE narrowing
controls and stored native field checks. No external code or fixture bytes are
added.


## Effective legacy Date comparison types (23 October 2026)

The explicit effective comparison path now admits legacy Date paired with ordinary
numeric carriers. Without Decimal, both operands use the existing CDate conversion
and temporal comparator. A numeric value outside the Date envelope reports
Overflow even if its relative order could otherwise be guessed. Decimal takes
precedence: both operands convert to Decimal, comparing the Date's raw serial
rather than normalizing its calendar representation. These selections follow the
[MS-VBAL relational effective-type table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8).

For example, `CDate(-0.25)=CDbl(0.25)` is True under the existing temporal
Date model; `CDate(-0.25)=CDec(0.25)` is False under Decimal selection.
`CDate(1)<CLng(2958466)` reports Overflow, while replacing CLng with CDec
compares successfully. Numeric conversion requires no Text policy. Date/Text
retains its existing CDate parser, explicit date format and optional numeric
fallback; Date/Date retains temporal ordering without new conversions. Neither
the table nor these controls establishes native Access negative-serial ordering.

A feature-local Date adapter composes existing CDate/Decimal conversion and
comparison services. It neither reconstructs timestamps nor changes stored bits,
calendar rounding or core representational equality. Extended Date is not
implicitly truncated; unsupported mixed carriers retain their existing refusal.
NULL, errors, eager operand/list evaluation, shared budgets and cancellation retain
their boundaries. The same Date/numeric routing now also serves ordinary
materialized predicates because its effective-type selection does not depend on
declaration categories. Set/group/sort key equivalence remains unchanged.

Three focused evaluator tests cover all ordinary numeric families, operand order,
negative fractions, both Date range ends, Decimal precedence, missing date policy,
NULL, non-finite values, Extended Date refusal and cancellation. Query/CLI tests
cover composition, joins, membership/quantifiers/ranges, defaults and buffered
overflow. The independent runner adds 936 source-free and 16 native controls,
using exact Decimal day/fraction tuples or raw serials according to effective
type. Only TableCraft evaluates SQL. Full Extended Date, Variant/state and native
text semantics remain open.


## Text built-in return categories (23 October 2026)

Every currently admitted TextFunction now has a reviewed return category in the
shared plan signature classifier. This uses the formal function declaration,
not prose saying that a function returns a String or a particular numeric value.
The dated MS-VBAL specification recorded above supplies numeric-conversion and
String function signatures, including the typed `$` distinctions.
[HyperlinkPart](https://learn.microsoft.com/en-us/office/vba/api/access.application.hyperlinkpart)
has an Access-specific String return and an explicit query example.

| Query calls | Declaration |
| --- | --- |
| Asc/AscB/AscW, InStrRev | Scalar Integer/Long, unchanged |
| Val | Scalar Double |
| CStr, Chr$, Replace, StrReverse | Scalar String |
| FormatDateTime, MonthName, WeekdayName | Scalar String |
| FormatNumber, FormatPercent, FormatCurrency, HyperlinkPart | Scalar String |
| Len/LenB, StrComp, InStr/InStrB | Variant containing their existing numeric result, unchanged |
| Chr/ChrW/ChrB, Left/Right/Mid and their B forms | Variant |
| LCase/UCase, LTrim/RTrim/Trim, StrConv | Variant |
| Space, String, Hex, Oct, Str, Format, Partition | Variant |

The previously admitted Chr$ spelling now resolves to a distinct ChrString
identity. It shares Chr's checked construction, code-page, arity and NULL rules,
but its Scalar String category survives binding/execution. Other `$` spellings
are not newly admitted. This is query expression functionality; no VBA runtime
or VBA-only function is introduced.

For example, `CInt(-32768)\Left('-1x',2)` widens to Long 32768 in the explicit
profile with an explicit numeric format; replacing Left with Val('-1') reports
Scalar overflow. `CVar(1)=Chr$(49)` can numerically compare equal under that
format, while `CVar(1)=Chr(49)` uses both-Variant String/numeric ordering and is
False. Equal returned characters do not establish equal declaration categories.

Two focused query tests cover all 43 admitted text identities through empty
subquery output metadata, plus actual direct/derived/scalar-subquery results.
Provider-requiring calls in empty results are never executed during preparation.
Character kernel tests retain resolution, byte-code boundaries and NULL behavior.
CLI checks retain both flag names and buffered errors. The independent runner
adds 97 source-free and 16 native controls, reusing the unchanged registered
four-family native inputs. Unreviewed aggregates/UNION remain unknown; full
Variant coercion, typed-function/native query qualification and native text
precedence remain open. Default result values and byte publication are unchanged.


## Session, numbered Error and GUID return categories (23 October 2026)

The shared classifier now records these reviewed categories for existing query
calls, independently of provider contents or returned value carriers:

| Query call | Category and primary reference |
| --- | --- |
| CurrentUser | Scalar String; [Access return declaration](https://learn.microsoft.com/en-us/office/vba/api/access.application.currentuser) |
| Error(number) | Variant containing String; dated MS-VBAL Error/Error$ declaration recorded above |
| GUIDFromString | Variant; [Access return declaration](https://learn.microsoft.com/en-us/office/vba/api/access.application.guidfromstring) |
| StringFromGUID | Variant; [Access return declaration](https://learn.microsoft.com/en-us/office/vba/api/access.application.stringfromguid) |

These calls already execute in the query profile. No omitted-number Error state,
new alias, VBA runtime or automatic session/catalog discovery is added. The existing
GUID byte/string representation and publication boundaries remain unchanged;
Variant provenance does not make binary bytes an arithmetic operand.

Empty subquery metadata retains categories without consulting a session, catalog
or GUID converter. Actual direct/derived/scalar-subquery Error results retain
Variant; a supplied test catalog returning numeric Text can widen an integral
quotient. A host-issued CurrentUser name with the same Text retains Scalar and
reports selected-width overflow. Providers still run once per written lookup;
missing providers and late operational failures remain explicit. Session metadata
still grants no authentication or database permission.

Two query tests and CLI controls exercise metadata and actual execution, provider
call counts, declaration-sensitive overflow and buffered failures. The independent
runner adds ten source-free and eight native controls, including GUID/String and
numbered-Error comparisons through original native fields. Original hashes/bytes
remain checked. Native Access declaration/host behavior remains unqualified.

The [Count reference](https://support.microsoft.com/en-us/access/functions/count-function)
and [UNION guide](https://support.microsoft.com/en-us/access/use-a-union-query-to-combine-multiple-queries-into-a-single-result)
describe their operations and output fields, but do not establish this profile's
Scalar/Variant return categories. Aggregate and UNION declarations therefore remain
unknown rather than being guessed from result carriers or a populated arm. Their
existing type-promotion/execution support is separate from declaration qualification.


## Distinct numbered Error$ return (23 October 2026)

The already admitted Error$ spelling now has a distinct bound identity and Scalar
String category; Error retains its formal Variant category. Both use the same
signed numbered-message lookup. For a supplied numeric-Text catalog result,
`CInt(-32768)\Error(-1)` can widen to Long while the Error$ form reports Scalar
overflow. Empty subquery metadata preserves the difference without calling the
catalog. Default message values are unchanged; signed input support and checked
provider migration are documented in [Error messages](ERROR_MESSAGES.md#signed-numbers-and-typed-error-identity-23-october-2026).
This supplies no latest-runtime-error state or native Access qualification.

## Unary negation with unknown provenance

Under the explicit declared-arithmetic policy, Integer, Long and legacy Date
negation now allows an unknown input declaration when the existing materialized
result retains the input carrier. For these three admitted families, Scalar and
Variant rules give the same successful result within that carrier. Negation runs
once, after ordinary eager operand evaluation. This does not infer a declaration:
the output retains unknown provenance for every later consumer.

For example, `-First(CInt(2))` now returns Integer -2. Later binary arithmetic
also uses agreement where available, but an operation requiring widening still
needs declaration provenance; wrapping the result in CVar explicitly supplies
Variant provenance.
`-First(CInt(-32768))`, `-First(CLng(-2147483648))` and an out-of-envelope Date
negation still report ArithmeticDeclaration: Scalar would overflow, while Variant
would widen. Known Scalar inputs retain Overflow and known Variant inputs retain
widening. Default execution is unchanged.

This is an agreement check within the existing reviewed
[unary arithmetic and overflow profile](ARITHMETIC_PROMOTION.md#unary-minus-and-division-family-selection),
not native Access aggregate/UNION declaration inference. Unary minus and its Date
coercion/Variant exception are specified in MS-VBAL section 5.6.9.3.1, using the
dated reference recorded above. Prepared executions recheck each input; a prior
successful value, failure or cancellation never changes the plan's category.
No extra source scan, operand evaluation, conversion provider or allowance is used.

## Binary arithmetic with unknown provenance

The explicit declared-arithmetic policy now evaluates unknown-category arithmetic
when the implemented Scalar and Variant branches agree for the evaluated operands.
The shared numeric service computes one materialized result and checks whether it
retains the selected integral, Single or legacy Date carrier. A retained result
is usable without assigning either category. Widening reports ArithmeticDeclaration
when provenance is unknown; known Scalar inputs still report Overflow and a known
Variant participant still permits widening. Double/fixed-point and other existing
category-independent operations retain their profiles.

For example, `First(CInt(2))+CInt(1)` returns Integer 3, while
`First(CInt(32767))+CInt(1)` still requires a declaration. The successful sum retains
unknown provenance: a comparison using it does not gain a Scalar/Variant category,
and a later operation that needs widening still refuses. Empty UNION arms and
selected rows never manufacture static declarations. Prepared execution checks
actual operands on each run without changing metadata or the plan.

`numeric::arithmetic_agreement` returns Some(value) for agreement, None for a
successful materialized result whose Scalar branch would overflow, or the common
validation/conversion/arithmetic error. NULL is Some(NULL), not ambiguity.
`Evaluator::apply_arithmetic_agreement` reuses contextual Text conversion and the
same metered rounded-operand service as declared integer division. Each operand
expression runs once, Text is normalized once and no second evaluation or fresh
allowance is used to compare branches. Standalone numeric kernels remain policy-free.

An unknown category no longer masks a common division-by-zero, invalid-number,
conversion-range, cancellation or resource failure. Those failures retain their
ordinary typed errors; this specifies the engineering policy rather than native
Access error precedence. The existing reviewed MS-VBAL arithmetic/overflow tables
remain the reference. Default execution and native declaration qualification are
unchanged; this does not complete P0 promotion or native SQL compatibility.
