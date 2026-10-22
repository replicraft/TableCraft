# Conservative query result types

`PreparedQuery::output_types()` exposes scalar metadata established during bounded,
metadata-only binding. `QueryResult::output_types()` exposes the same profile beside
buffered output names and rows, including empty results. Both scalar and expression
result APIs retain it. These are current evaluator guarantees, not a recreation of
DAO field codes or complete native Access SQL expression typing.

`OutputType::Scalar { data_type, nullable }` reuses the schema crate's validated
`TypeDescriptor`. It describes the carrier and supported value constraints;
`nullable: true` conservatively allows NULL. `OutputType::Dynamic` establishes no
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
| Integer, Text, Boolean, legacy DATE and GUID literals | Their current materialized carrier; non-NULL |
| Nine numeric/Boolean conversion calls | Their explicit target carrier; conservative NULL allowance |
| IS NULL and EXISTS | Non-NULL Boolean |
| Comparisons, LIKE, BETWEEN, IN and quantified predicates | Boolean with conservative NULL allowance |
| IIf with matching branch descriptors | Shared descriptor and combined nullability; otherwise Dynamic |
| Count | Current Int64 accumulator result, non-NULL |
| Min, Max, First and Last | Argument descriptor with NULL permitted for empty groups |
| Scalar subqueries | Single output descriptor with NULL permitted for no row |
| UNION | Matching descriptors combine nullability; differing descriptors remain Dynamic |
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
sample records. Query tracing remains payload-free.

Arithmetic that widens by materialized value, non-integer numeric literal typing,
Sum/Average/statistical results, most function-return inference, heterogeneous
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
