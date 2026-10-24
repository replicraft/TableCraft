# Explicit row-local calculation previews

The native adapter can now prepare and explicitly evaluate a restricted stored
calculated expression. This is an engineering preview using TableCraft's current
expression semantics. It does not make calculated fields ordinary query columns,
coerce results into their declared stored type, certify cached freshness, update
the database or establish Microsoft Access equivalence. No CLI command is added.

```rust,no_run
use std::fs::File;
use tablecraft_access::NativeSource;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::ExecutionOptions;
use tablecraft_sql::ParseLimits;

let source = NativeSource::new(
    File::open("copy.accdb")?,
    CatalogScope::new(1),
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000),
    &|| false,
)?;
let preview = source.prepare_calculation("Calculations", "Twice", ParseLimits::default())?;
let original_expression = preview.declaration().expression()?;
let declared_stored_type = preview.declaration().data_type();
let ordinary_dependencies = preview.dependencies();
let result = preview.execute(ExecutionOptions::default())?;
assert_eq!(result.columns(), &["Recomputed"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Explicit preparation and execution

The engine's `inspect_calculation_declaration` checks the existing ACE 16-family
calculated-field geometry, native result carrier, matching typed ResultType and
nonempty Expression properties without reading user records. The returned
`CalculationDeclaration` retains exact original expression text and native
metadata. It does not parse the formula or inspect a cached wrapper.

`NativeSource::prepare_calculation` then parses one complete expression, checks
its restricted syntax and resolves every dependency to an ordinary scalar field
of that same native table. It returns the dependency names in first-reference
order, the original declaration, an owned query model and a bound plan. Binding
reads no user records and activates no formula, clock, provider or function.
`PreparedCalculation` borrows its originating source; callers cannot substitute
another source at execution. Existing prepared-query schema checks remain active.
This lifetime relationship does not create a coherent live-file snapshot or a
revision guarantee: the source must remain unchanged/offline, as for other native
queries.

Execution is a separate explicit operation. The output column is named
`Recomputed`; its metadata and carriers follow the current query evaluator's
expression-promotion rules, separately from the declared stored carrier. A
database may contain stale or internally inconsistent expression/result metadata;
a successful preview does not prove that Access accepts that declaration or that
its stored cache is fresh. The original caches are neither loaded as inputs nor
overwritten. Corrupt unselected cache payloads can therefore coexist with a valid
preview over ordinary fields; row geometry and selected dependencies still must
validate normally.

## Admitted expression profile

The current profile admits NULL/Boolean/integer/numeric/Text literals, unqualified
ordinary same-table field references, unary numeric plus/minus, IS NULL/IS NOT
NULL, arithmetic `+`, `-`, `*`, `/`, `\`, `Mod`, `^` and explicit Text
concatenation `&`. Duplicate field references do not duplicate dependencies.
Query rules still determine operator behavior and eager failures; admitting syntax
does not assert complete Variant promotion or native calculated-field behavior.

Calls of every kind, domain/aggregate functions, clocks, UDFs, subqueries, identity,
parameters, external/qualified references, collection members, calculated-field
dependencies, date/GUID literals and other operators remain refused before row
access. This also prevents self/cyclic calculated dependencies in the current
profile. Unknown fields never become implicit parameters. SQL-like text inside
an admitted string literal remains inert data; expressions are moved into a
checked query model rather than interpolated into a SQL statement. Table names
containing closing brackets or control characters remain unsupported by this
preview's current identifier boundary.

## Controls and evidence

Native declaration/source work and retained syntax/dependency storage consume the
source's cumulative native budget. The parser additionally has its own checked
input-byte, token and depth ceilings. Its temporary tokenizer/parser workspace is
bounded by those structural limits, not accounted as native retained allocations;
it has no mid-parse cancellation callback. Cancellation is checked before/after
parsing and during syntax traversal. Execution retains the normal independent
query byte/row/work limits, native source controls, per-Text limits and optional
explicit policies. Preparation and execution failures publish no partial plan or
result. Parsing or evaluation is never triggered by ordinary file inspection.

Focused adapter tests cover both qualified ACE 16-family profile codes,
metadata-only preparation, a deliberately stale/NULL cache versus recomputation,
watched preparation over corrupt cached payloads, unchanged stored results,
ordinary dependency discovery, unsupported active/nonlocal/self references,
strict statement-terminator refusal, parser/native/query limits and cancellation.
A separate syntax test executes the documented Text concatenation shape and
checks dependency deduplication. These are independent original synthetic and
TableCraft evaluator controls; native Access comparison remains unrun.

Microsoft's [Field2.Expression contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-expression-property-dao)
describes read-only calculated results from same-table fields and demonstrates
`[FirstName] & " " & [LastName]`. It excludes fields from other tables/queries.
This supports the row-local boundary; it does not qualify every query function,
promotion rule or physical cached representation. The existing
[advanced-field contract](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-scalar-calculations)
continues to own cached storage decoding and preservation.
