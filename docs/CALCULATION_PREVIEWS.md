# Explicit row-local calculation previews

The native adapter can now prepare and explicitly evaluate a restricted stored
calculated expression. This is an engineering preview using TableCraft's current
expression semantics. It does not make calculated fields ordinary query columns,
coerce results into their declared stored type, certify cached freshness, update
the database or establish Microsoft Access equivalence. The CLI exposes this same
explicit preview through `query preview-calculation`.

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

## Explicit CLI preview

```sh
tablecraft query preview-calculation --database copy.accdb Items Score
tablecraft --format tsv query preview-calculation --database copy.accdb Items FullName \
  --number-format invariant
```

The required TABLE and COLUMN select one formula; the only result heading is
`Recomputed`. Values follow the existing evaluator and output formatter. A native
Int32 declaration can therefore produce Text or a widened numeric result in this
preview. Row order follows the ordinary source's sequential snapshot. The command
requires the qualified ACE calculated-field declarations; it does not supply
calculated fields to formats that lack them.

This differs from `query run --stored-calculations`: that explicit policy reads
stored caches, while this command recomputes from ordinary same-table inputs and
never opens the cached-value source mode. Stale/NULL or malformed unselected
caches do not become dependencies. Neither command changes ordinary query-column
eligibility or certifies cache freshness. No output database or save option exists.

Global `-p`/`--password` uses the ordinary native opening path, including its hidden
terminal prompt when credentials are required and omitted. Missing passwords in
noninteractive use and incorrect passwords fail before formula execution. Source
bytes remain unchanged. Global `--format` selects the existing table/TSV transport,
including its CSV-crate quoting of displayed Text values.

`--number-format` and `--date-format` explicitly supply the conversion/concatenation
policies used by the admitted expression profile. There are no clock/random,
collection, index, cached-source or declared-arithmetic options: these would not
expand the existing preparation contract. All calls and calculated dependencies
remain refused before user records are read, even when those records are corrupt.

Resource controls retain their existing defaults and independent ceilings:

| Stage | Explicit controls |
| --- | --- |
| Native metadata/ordinary dependencies | `--native-bytes`, `--native-rows`, `--native-steps` |
| Encoded and decoded long values | `--native-value-bytes`, `--native-text-bytes` |
| Query evaluation/intermediates | `--execution-bytes`, `--execution-rows`, `--execution-steps`, `--text-bytes` |
| Buffered display and final transport | `--output-bytes` |

The parser keeps its existing bounded default syntax policy. Results are fully
evaluated and rendered before stdout. A later arithmetic/read/format failure or
quota refusal produces no successful prefix; final TSV quoting and table layout
also remain inside the selected output allowance. No limit is raised implicitly.

Ten focused CLI tests cover both qualified ACE 16-family headers, stale/NULL
caches versus recomputed values, corrupt-cache pruning, natural Text output under
an Int32 declaration, explicit numeric formatting, late division failure, refused
calls/nonlocal/calculated inputs, invalid user records, native/query/Text/output
ceilings, table/TSV formatting, final transport expansion, required arguments and
encrypted password paths. Every authored database is checked byte-for-byte after
use. These are engineering controls; native Access formula results and freshness
remain unqualified.

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
