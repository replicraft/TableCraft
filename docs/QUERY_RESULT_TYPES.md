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
| Nine numeric/Boolean conversion calls | Their explicit target carrier; conservative NULL allowance |
| Stable text functions | Text, Int16, Int32 or Double according to the current evaluator; conservative NULL allowance |
| Abs/Int/Fix/Round | Supported numeric input carrier, Boolean becomes Int16; input value constraints removed |
| Sgn, transcendental math, RGB/QBColor and Rnd | Int16, Double, Int32 and Single respectively; no provider is activated during binding |
| Date components/DatePart/Weekday and DateDiff | Int16 and Int32 respectively; nullable |
| Date constructors/conversions | Nullable legacy DateTime |
| DateAdd | Declared legacy/extended date carrier with constraints removed; otherwise Dynamic |
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
| Nz without replacement | Nonnullable input can retain its descriptor; nullable/NULL-only input stays Dynamic because the replacement is Empty |
| Stable binary arithmetic | Successful fixed carriers and NULL propagation under the profile below; value-dependent widening stays Dynamic |
| Stable unary + / - | Preserve the supported carrier, or select Double for numeric Text and Integer for Byte/Boolean negation; value-dependent Integer/Long negation stays Dynamic |
| Count | Current Int64 accumulator result, non-NULL |
| Sum of integral/Boolean, floating, Currency or Decimal operands | Int64, Double, Currency or Decimal respectively; nullable, without input value constraints |
| Avg of ordinary integral/floating/Currency or Decimal operands | Double or Decimal respectively; nullable |
| Var/VarP/StDev/StDevP and statistical domain calls | Nullable Double, including insufficient-population results |
| DCount | Nullable Int64; unresolved criteria fields can return NULL |
| Min, Max, First and Last | Argument descriptor with NULL permitted for empty groups |
| Scalar subqueries | Single output descriptor with NULL permitted for no row |
| UNION | Exact integer-width and Single/Double widening; Text/Decimal constraint merging; other mixed families stay Dynamic. See [UNION types](UNION_TYPES.md). |
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
metadata never suppresses that evaluation. Compatible descriptors retain their
validated constraints, but different constraints/carriers still produce Dynamic.
Switch/Choose allow NULL for no selection, and implicit Nz replacement remains
expression-only Empty. Error data is not NULL-only and does not acquire a scalar
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
NULL. A null-only operand establishes a null-only successful result, while eager
failures in either operand still fail execution.

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
