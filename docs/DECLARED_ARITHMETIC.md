# Explicit declarations for numeric arithmetic

The expression library provides `declared_integral_arithmetic` and the broader
`declared_numeric_arithmetic` with caller-issued `ArithmeticDeclarations`. Query
execution offers this engineering profile through `query run --declared-arithmetic`;
`--declared-integral` remains a visible compatibility alias. The public
`ExecutionOptions::declared_integral` and worker option retain their historical
field names, with integral and Single-selected overflow now covered. Default SQL
execution retains its existing materialized profile. This option and these APIs
do not establish complete native SQL compatibility.

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
arithmetic requiring an unknown category fails with ArithmeticDeclaration unless
a known Variant participant already establishes widening. An explicit cast or
CVar can supply the caller's intended category. NULL propagation and integral
Double-result division/power do not require this width decision.

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
948 independent scalar SQL controls with result carrier checks. Its optional
`--native-directory` checks add 188 controls on the registered unchanged
20,000-row Jet 4, ACE 2007, ACE 2016 and ACE 2019 fixture images. These
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
quotient/remainder reuse the existing checked conversion targets. Text and dates
remain contextual evaluator operations, not numeric-kernel inputs.

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
The independent declaration runner now passes 1,272 controls (1,036 source-free
and 236 native integrations), using fixed clocks, known financial roots and raw
registered native getters with unchanged original hashes. Only TableCraft
executes SQL. The VBA/Access signature references and independent observations
do not establish native ACE SQL overflow/coercion equivalence. Ordinary SQL
aggregate declarations, unreviewed functions/operators, complete Variant
promotion and native text acceptance remain open; defaults retain their existing
materialized arithmetic profile. No dependency or numerical-kernel change.
