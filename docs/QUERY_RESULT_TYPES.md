# Conservative query result types

`PreparedQuery::output_types()` exposes scalar metadata established during bounded,
metadata-only binding. `QueryResult::output_types()` exposes the same profile beside
buffered output names and rows, including empty results. Both scalar and expression
result APIs retain it. These are current evaluator guarantees, not a recreation of
DAO field codes or complete native Access SQL expression typing.

`OutputType::Scalar { data_type, nullable }` reuses the schema crate's validated
`TypeDescriptor`. It describes the carrier and supported value constraints;
`nullable: true` conservatively allows NULL. `OutputType::Null` establishes that
successful cells are only SQL NULL. It is expression metadata, not a stored field
type or permission to coerce values. `OutputType::Dynamic` establishes no
single scalar descriptor. It can represent varying carriers, expression-only
states or inference that has not been implemented. Dynamic is not stored NULL,
a conversion request or permission to guess a type from a successful row.

## Implemented propagation

| Expression or composition | Metadata |
| --- | --- |
| Direct fields and aliases | Source descriptor, supported constraints and nullability |
| Wildcards, pruning, named and derived SELECT sources | Preserve each output's metadata without inspecting records |
| Outer joins | Nullable side permits NULL, including through nested/derived joins |
| Qualified collection fields | Stored element/member descriptor with conservative NULL allowance |
| Declared parameters | Declared descriptor; NULL allowed by the parameter contract |
| SQL NULL literal | Null-only metadata |
| Integer, decimal/exponent, hexadecimal/octal, Text, Boolean, legacy DATE and GUID literals | Their current materialized carrier; non-NULL |
| Nine numeric/Boolean conversion calls | Their explicit target carrier; successful conversions are non-NULL |
| Stable text functions | Text, Int16, Int32 or Double according to the current evaluator; conservative NULL allowance. Qualified NULL-propagating source arguments retain NULL-only results as described below. |
| Abs/Int/Fix/Round | Supported numeric input carrier, Boolean becomes Int16; input value constraints removed. NULL-only value inputs remain NULL-only. |
| Sgn, transcendental math, RGB/QBColor and Rnd | Int16, Double, Int32 and Single respectively; Sgn/transcendental NULL-only inputs remain NULL-only. No provider is activated during binding. |
| Date components/DatePart/Weekday and DateDiff | Int16 and Int32 respectively; nullability follows the date value operands, including NULL-only results |
| DateSerial/TimeSerial and CDate/CVDate | Non-NULL legacy DateTime on successful evaluation; NULL arguments still fail |
| DateValue/TimeValue | Legacy DateTime with source nullability, including NULL-only results |
| DateAdd | Existing Extended source retains Extended; supported numeric/Boolean/Text and legacy sources produce legacy DateTime. Source nullability is retained; a NULL-only source stays NULL-only; unsupported/Dynamic sources stay Dynamic. No conversion runs during binding. |
| Date/Time/Now | Non-NULL legacy DateTime; no clock read during binding |
| Financial/cashflow functions | Nullable Double |
| IsNumeric/IsDate and value inspection | Non-NULL Boolean, Int16 subtype code or Text type name |
| GUIDFromString / StringFromGUID | Binary GUID-structure bytes / Text; no inferred semantic GUID conversion |
| CurrentUser and CVar | Text session identity / unchanged input descriptor; no host lookup during binding |
| IS NULL and EXISTS | Non-NULL Boolean |
| Comparisons, LIKE, BETWEEN, IN and quantified predicates | Boolean with conservative NULL allowance |
| IIf with matching branch descriptors or a NULL-only branch | Shared descriptor and combined nullability; otherwise Dynamic |
| Switch/Choose | Union of value branches, permitting NULL for no selection; conditions/index do not supply result types |
| Explicit Nz | Known nonnullable input is preserved; NULL-only input uses replacement metadata; otherwise compatible descriptors combine |
| Nz without replacement | Nonnullable input is preserved; NULL-only input yields nonnullable Text; nullable Text retains its constraints without NULL; other nullable carriers stay Dynamic because they can also produce empty Text. |
| `&` concatenation | Unconstrained Text for known supported scalar operands; NULL only if both can be NULL. Two NULL-only operands remain NULL. No formatting during binding. |
| Stable binary arithmetic | Successful fixed carriers and NULL propagation under the profile below; value-dependent widening stays Dynamic |
| Logical NOT | Boolean/exact integer carrier, or Long for floating/fixed/legacy DATE/Text; preserves nullability and removes changed-value constraints. |
| AND / OR / XOR / EQV / IMP | Stable Boolean/integer carrier after accounting for possible NULL substitutions; conflicting successful carriers stay Dynamic. Known admitted NULL-only XOR/EQV results stay NULL-only. |
| Stable unary + / - | Preserve the supported carrier, or select Double for numeric Text and Integer for Byte/Boolean negation; value-dependent Integer/Long negation stays Dynamic |
| Count | Current Int64 accumulator result, non-NULL |
| Sum of integral/Boolean, floating, Currency or Decimal operands | Int64, Double, Currency or Decimal respectively; nullable, without input value constraints |
| Avg of ordinary integral/floating/Currency/legacy DateTime or Decimal operands | Double or Decimal respectively; nullable |
| Var/VarP/StDev/StDevP and statistical domain calls | Nullable Double, including insufficient-population results |
| DCount | Nullable Int64; unresolved criteria fields can return NULL |
| Min, Max, First and Last | Argument descriptor with NULL permitted for empty groups |
| Scalar subqueries | Single output descriptor with NULL permitted for no row |
| UNION | Shared numeric descriptors across nine families, exact fixed-point widening and explicit Double rounding; Text constraint merging and explicit-policy numeric-to-Text results; other mixed families stay Dynamic. See [UNION types](UNION_TYPES.md). |
| Fixed-heading crosstabs | Group-output descriptors and nullable cell descriptors |

Top-level dynamic crosstabs return `None` during preparation because their output
width is unknown. Their execution result retains group-output descriptors and
one nullable cell descriptor per discovered heading. This uses the bound cell
expression, not scans to infer a type. Named schema discovery still belongs to
explicit execution staging; cached crosstab result metadata survives that boundary.

Output metadata permits duplicate names just as query results do. It does not
create stored column identities or make a query updateable. Source schemas and
records keep their exact validation contracts. Arithmetic, comparisons, aggregate
accumulation and query execution are not coerced to fit metadata.

## Limits and unfinished work

DateAdd metadata describes the existing source-conversion contract rather than
executing it. An invalid Text value, missing date policy, out-of-range serial or
invalid interval/count still fails at execution, including beside a NULL-only
date source. Named/derived outputs, UNION, Nz and Avg retain these descriptors
without sampling rows or activating a provider. An existing Extended source keeps
its carrier and 100 ns precision. Source-specific value constraints are discarded;
the successful carrier does not establish native Scalar/Variant declaration
categories or native Access result typing. See [interval source rules](DATE_INTERVALS.md).

Metadata vectors use fallible allocation and charge the owning binding/execution
budget. Inference visits share the binder's structure, depth, work and cancellation
limits. Source declarations are never validated by executing expressions or reading
sample records. Closed numeric/DATE/GUID literal spellings reuse the bounded scalar
literal evaluator with no bindings, providers or runtime context. Only literal
forms enter that helper; arbitrary expression evaluation is not part of binding. Query tracing remains payload-free.

Arithmetic that widens by materialized value, unsupported aggregate input/result
combinations, byte-string/StrConv, broader conditional/domain result inference, heterogeneous
UNION/conditional promotion, complete Variant/static overflow rules and native
Access result-type qualification remain unfinished. Dynamic results must stay
visible while these rules are implemented; no implicit Double/Text/NULL placeholder
is supplied. [Materialized promotion](ARITHMETIC_PROMOTION.md) is a separate service.

## Evidence and references

Focused tests use a source whose scan callback panics, constrained Text fields,
parameters, named/derived sources, nested outer joins, subqueries, aggregates,
UNIONs, conversions and fixed/dynamic crosstabs. Runtime controls check that every
claimed scalar descriptor accepts its returned values, with matching nullability,
and that metadata remains available for an empty result. Native adapter controls
bind a deliberately invalid stored DATE field without decoding its payload, then
execute an independently selected valid field with retained metadata. Synthetic
ACE 16-family indexed reads also retain Int32/Int64 metadata with no extra native
data or index reads.

Microsoft documents [DAO Field.Type](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-type-property-dao)
and [Field.Required](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-required-property-dao)
as distinct properties. These motivate retaining carrier and nullability separately;
our descriptors are not those numeric DAO codes and these tests are not native
Access comparisons. Native SQL type/promotion evidence remains a separate gate.


The numeric metadata extension follows the current accumulator's successful
carriers and does not silently change arithmetic. [Wider means](AVERAGES.md) now fix the former Currency Avg division/accumulation
gaps; its qualified result metadata is nullable Double.
Input Decimal precision/scale constraints are not reused as sum/mean constraints.
Microsoft's [Sum](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/sum-function-microsoft-access-sql)
and [Avg](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/avg-function-microsoft-access-sql)
references specify numeric expressions and NULL elimination; they do not prove
this implementation's precise carrier/rounding policy. Native comparisons remain
required before claiming those details equivalent.


## Function metadata boundaries

Function metadata describes successful results of the existing evaluator. It does
not validate argument values, infer a locale, advance Rnd, consult a clock or
session, execute financial solvers or suppress later evaluation failures. Most
NULL allowances remain deliberately conservative, even where invalid NULL operands
raise an error. A known result carrier does not make an invalid call successful.
Text-changing functions and numeric transforms drop operand length/precision/scale
constraints. CVar retains them because it preserves the exact input value.

ChrB/LeftB/RightB/MidB and StrConv remain Dynamic because byte-string intermediates
are expression-only states and conversion controls can choose a different result
family. Materialized-value arithmetic widening, mixed conditional/UNION promotion
and runtime domain target typing remain separate work. No new function is enabled
by this metadata change, including deferred VBA-only calls.

Focused controls check more than sixty successful function results against their
claimed descriptors, all nine numeric families for carrier-preserving math,
explicit NULL rows, empty derived results and provider-free preparation. Guid byte
arrays are checked as Binary, not semantic Guid; native DAO result-type/precision
qualification remains unrun.


## NULL-aware composition

Null-only metadata survives named/derived projections, aliases, outer null padding
and scalar-subquery empty results. It also describes unary NULL and numeric or
encounter aggregates over NULL-only operands. UNION combines it with a scalar
without widening/coercing the scalar descriptor. Two NULL-only arms remain
NULL-only; mixing with Dynamic stays Dynamic. No sample records or chosen branch
values are used to derive these rules.

Selectors retain eager evaluation. An unselected expression can still fail;
metadata never suppresses that evaluation. Compatible carriers merge their
supported constraints; different carriers still produce Dynamic.
Switch/Choose allow NULL for no selection. Query Nz uses an ordinary empty-Text
replacement, distinct from host-supplied expression Empty. Error data is not NULL-only and does not acquire a scalar
placeholder. Public consumers of OutputType must handle the added Null variant.

Focused controls check NULL-only named/derived outputs, NULL UNION arms in both
orders, selectors, empty results and fixed crosstab cells against actual rows.
These are current evaluator guarantees, not native Access static type rules or
complete heterogeneous Variant promotion.


## Stable binary arithmetic metadata

The seven arithmetic operators now infer successful result descriptors from operand
metadata when the current evaluator guarantees one carrier. Binding never evaluates
the operands, parses numeric Text, reads a row, consults formatting providers or
chooses a type from a sample result. Both operand inference visits share existing
binder work/depth/cancellation limits. Arithmetic drops input Text length and
Decimal precision/scale constraints; result nullability permits either operand's
NULL. A null-only operand establishes a null-only successful result with a supported
counterpart. A known GUID, Binary or Extended-date counterpart instead stays
Dynamic because the current arithmetic profile rejects it. Inference never turns
an unsupported field into NULL; eager failures in either operand still fail execution.

Working cases include LongLong integral addition/subtraction/multiplication;
fixed Decimal/Currency selection; ordinary Double results, contextual numeric Text
and Text+Text addition; ordinary `/` with integer operands; exponentiation; legacy
DATE subtraction from DATE; and converted-width Mod remainders. Rounded integer
quotients retain fixed Byte or checked LongLong descriptors. Other Integer/Long
quotients, narrow integer arithmetic, small Single arithmetic/division, and DATE
addition/subtraction that can leave its envelope remain Dynamic. Unsupported or
unknown operand families remain Dynamic rather than acquiring a conversion.

These descriptors flow through ordinary aggregate inference, empty and derived
results and fixed crosstabs. They also supply the existing UNION common-carrier
service: `SELECT CDbl(1)+1 UNION ALL SELECT CSng(4)` now publishes two Double cells.
Arithmetic itself retains its existing materialized values, conversions, overflow,
precision guards and eager errors. This implements successful-carrier metadata;
it does not implement static non-Variant overflow or native Access SQL typing.
Microsoft's [arithmetic operator specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
separates declared and effective types, and its
[division exceptions](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
make a single numeric precedence ladder insufficient. These are references,
not native SQL qualification.

Six focused tests cover a metadata-only source whose scan panics, all 847
operand-family/operator combinations with nullable rows, dropped constraints,
empty/derived/set/aggregate/crosstab consumers, quotient widening and eager failure
preservation. The independent UNION runner now passes 109 controls including
arithmetic arms in both orders and original native stored integers across four
priority headers. Windows result-type comparisons remain unrun.


Legacy Date Avg inputs now infer nullable Double, matching the implemented
[serial mean profile](AVERAGES.md#legacy-datetime-means). This is independent of
source nullability because an empty group still returns NULL. Domain target
metadata remains conservative; native Access result typing is unqualified.


## Logical result carriers and nullable alternatives

Metadata-only binding now describes the existing evaluator's NOT and five binary
logical operators. NOT keeps Boolean, Byte, Integer, Long and LongLong carriers;
floating/fixed-point, legacy DATE and contextual Text select Long. Changed-value
constraints are discarded. NULL-only input stays NULL-only. GUID, Binary,
Extended dates and unknown expression-state outputs remain Dynamic.

Binary inference considers each admitted input's declared carrier and possible
NULL substitution, with at most four bounded combinations and no record scans.
Matching successful carriers combine their NULL allowances; conflicting carriers
remain Dynamic. For example, nullable Byte/Boolean AND can produce Integer,
Byte or Boolean, so it cannot claim one scalar descriptor. XOR/EQV return NULL
when either operand is NULL, allowing the same nullable pair to retain Integer
metadata. Two NULL-only operands stay NULL-only; admitted NULL XOR/EQV operands
also establish NULL-only successful results. These descriptors never suppress
conversion, overflow, policy or eager-expression failures.

Nonnullable input types follow the existing materialized width ladder: Boolean
pairs retain Boolean, Byte pairs retain Byte, mixed smaller integers use Integer,
Long/LongLong retain their wider width, and rounded operands select Long or
LongLong from original carriers before conversion. This documents the current
[logical profile](INTEGER_LOGIC.md), not complete native Variant typing.

Named/derived sources, aggregates, subqueries and empty results retain these
descriptors. Stable logical outputs can now participate in the existing numeric
UNION common-type conversion: `Not CByte(1)` combined with `CInt(-2)` produces
Integer 254/-2. This is metadata-driven UNION widening, not a change to NOT's
standalone Byte result. Unknown Scalar/Variant declarations remain unknown;
metadata does not infer them from a successful carrier.

Focused tests cover all eleven admitted families, nullable alternatives, inert
named/Text binding, removed constraints, unsupported families, buffered failures
and UNION widening. [MS-VBAL logical operators](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/2d70780d-6e99-47d6-9759-2b3a46b8e862)
guide the existing explicit operator contract; these tests establish TableCraft
metadata consistency, not native Access SQL conformance.

## NULL-only math inputs

The twelve existing unary math functions Abs, Int, Fix, Round, Sgn, Sqr, Exp,
Log, Sin, Cos, Tan and Atn now retain NULL-only metadata when their value operand
is itself known to return only NULL. This follows their existing successful-value
behavior without evaluating operands, controls, records or providers in binding.
Derived projections, scalar subqueries, selectors and aggregates retain that
information. Rnd, RGB and QBColor keep their separate non-NULL result contracts;
a NULL argument does not turn those functions into successful NULL results.

This affects the existing materialized UNION conversion profile. For example,
`SELECT Sqr(NULL) AS N UNION ALL SELECT CByte(2)` retains NULL and Byte values,
with a nullable Byte descriptor; the NULL-only arm no longer imposes Double on
the other arm. Both arm orders use the same rule. This is metadata about the
implemented evaluator, not qualification of native Access declared UNION types.

NULL-only output metadata never replaces execution. Round still evaluates its
supplied digit operand, converts it to Long and checks its supported range before
returning NULL. Eager failures, nested subquery failures, cancellation and shared
budgets remain in force. An invalid later UNION arm still prevents buffered
publication, even when an earlier arm succeeded.

## NULL-only text inputs

Text result inference now follows the existing NULL propagation of 23 functions.
It examines only their designated source/count arguments in the normalized owned
call; it does not evaluate arguments, scan records, format values or invoke a
collation/language provider during preparation.

| Functions | Argument positions that establish a NULL-only successful result |
| --- | --- |
| Len/LenB, LCase/UCase, LTrim/RTrim/Trim, Left/Right/Mid, LeftB/RightB/MidB, StrConv, Space, Hex/Oct/Str | First argument |
| String, StrComp, InStrRev | Either of the first two arguments |
| Two-argument InStr/InStrB | Either string argument |
| InStr/InStrB with an explicit/defaulted start slot | Second or third argument; the start and compare controls do not supply result nullness |

An argument must itself have NULL-only metadata. A nullable Text parameter or
field does not qualify, even when an earlier execution supplied NULL. Optional
written slots use the same normalization as binding. Named/derived projections,
scalar subqueries and nested NULL-propagating calls retain the result without
execution-stage schema discovery or provider activation.

Within the existing materialized UNION profile, a NULL-only text-function arm
no longer forces Text/Long conversion on the other arm. For example,
`SELECT Len(NULL) AS N UNION ALL SELECT CByte(2)` retains NULL and Byte, with a
nullable Byte descriptor. This describes current evaluator guarantees; native
DAO declared field codes and Access UNION typing remain unqualified.

Metadata never skips runtime controls or written expressions. Invalid byte-slice
positions, invalid compare/conversion settings and eager division errors still
fail, including after an earlier successful UNION row. CStr, Val, StrReverse and
character-code functions keep their own NULL-refusal contracts; the inference
rule does not turn those failures into successful NULL results. Text and math
metadata now have separate feature modules under the query binder.

## Date value nullability and successful constructors

Year, Month, Day, Hour, Minute, Second, Weekday and DatePart retain NULL-only
metadata from their date source. Otherwise their Int16 carrier uses that source's
nullability. DateDiff similarly returns NULL-only when either date source is
NULL-only; its Int32 result permits NULL when either source is nullable or Dynamic.
Unknown date sources retain a fixed successful carrier with conservative NULL
allowance, without guessing their value or declaration.

DateValue and TimeValue follow source nullability with legacy DateTime as the
successful carrier. CDate and CVDate instead promise a non-NULL DateTime on
success, because a NULL source raises InvalidNullConversion. DateSerial and
TimeSerial likewise produce non-NULL dates on success; NULL components remain
errors. These descriptors summarize the existing evaluator, without changing its
conversion, component validation, precision or policy contracts.

Named/derived outputs, UNION and Nz now retain these guarantees. For example,
`SELECT Year(NULL) UNION ALL SELECT 'kept'` has nullable Text metadata, because
its first arm contributes only NULL. It no longer imposes an Int16 possibility
on the union. A nullable date field remains nullable across preparations and
executions; its currently observed value cannot narrow the descriptor.

Six focused tests cover component families, both DateDiff source positions,
nullable/nonnullable fields, Dynamic input, named/derived compositions, UNION/Nz,
constructor/conversion errors and eager/control failures. Preparation checks a
scan counter before execution; it does not inspect rows, parse Text, consult
locale/clock providers or suppress failing controls. This is successful-value
metadata, not proof a call will succeed or native Access result-type qualification.
