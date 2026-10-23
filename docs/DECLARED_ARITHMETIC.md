# Explicit declarations for integral arithmetic

The shared expression library now provides `declared_integral_arithmetic` with
caller-issued `ArithmeticDeclarations`. Query execution now offers an explicit engineering profile through
`ExecutionOptions::declared_integral` and `query run --declared-integral`. Default
SQL execution retains its existing materialized arithmetic profile. Neither this
option nor the standalone API establishes complete native SQL compatibility.

## Working input and result profile

`OperandDeclaration::Scalar` means the supplied carrier is the caller's declared
scalar type. `Variant` means a declared Variant currently holds that carrier.
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
tablecraft query run 'SELECT CInt(32767)+CInt(1)' --declared-integral
tablecraft query run 'SELECT CVar(CInt(32767))+CInt(1)' --declared-integral
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
Aggregate/domain results and unclassified functions also remain unknown;
a selected input row cannot establish a function's declaration. Non-NULL integral
arithmetic requiring an unknown category fails with ArithmeticDeclaration unless
a known Variant participant already establishes widening. An explicit cast or
CVar can supply the caller's intended category. NULL propagation and integral
Double-result division/power do not require this width decision.

This profile does not introduce wider floating, Currency, Decimal or date
promotion rules. Dynamic Eval retains its existing materialized evaluation and
unknown output declaration. Binding performs no row scans to infer categories.
Both written operands retain eager evaluation; division errors, cancellation and
resource exhaustion remain visible. Text comparison policy is tracked separately.
The optional SQLite worker forwards this option and the individual Text allowance
per request, with defaults restored for each subsequent request.

The query suite checks direct/derived/named/correlated values, typed parameter
reuse, unknown UNION/aggregate outputs, eager failures and cancellation. The
original [SQL comparison runner](testing/compare_query_declarations.py) passes
217 independent unbounded-integer controls with result carrier checks. These
controls qualify the explicitly selected engineering policy, not native Access.


## Reviewed function return declarations

Function result carriers are not declaration categories. Under the explicit
engineering profile, the following reviewed functions now issue categories from
their documented signatures, independently of their input or selected record:

| Query functions | Declaration | Primary reference |
| --- | --- | --- |
| Len, LenB | Scalar Long | [Len](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/len-function) |
| Asc, AscB, AscW | Scalar Integer | [Asc](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/asc-function) |
| VarType | Scalar Integer | [VarType](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/vartype-function) |
| InStr, InStrB | Variant containing Long | [InStr](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instr-function) |
| StrComp | Variant containing Integer | [StrComp](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strcomp-function) |
| Sgn | Variant containing Integer | [Sgn](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/sgn-function) |
| Year | Variant containing Integer | [Year](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/year-function) |

These are functions already exposed to queries; no VBA runtime is introduced.
For example, `Asc('A')+CInt(32767)` overflows under this policy, whereas
`Sgn(1)+CInt(32767)` widens to Long. Converting either result to CVar explicitly
requests Variant behavior. NULL results and eager operand errors keep their
existing behavior. The classifications survive derived outputs and do not reuse
successful-carrier inference as declaration inference. Other functions retain
unknown categories until reviewed; this table does not claim native ACE SQL
classification has been established by the VBA language references.
