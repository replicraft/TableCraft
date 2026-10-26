# Explicit row-local calculation previews

The native adapter can now prepare and explicitly evaluate a restricted stored
calculated expression. This is an engineering preview using TableCraft's current
expression semantics. It does not make calculated fields ordinary query columns,
coerce results into their declared stored type, certify cached freshness, update
the database or establish Microsoft Access equivalence. The CLI exposes this same
explicit preview through `query preview-calculation`.

Hosts can separately compose selected formulas into buffered queries using
[explicit recomputed calculation views](RECOMPUTED_CALCULATION_VIEWS.md). Those
fresh named views preserve the same formula admission and natural intermediate
carriers without changing original table names or the standalone preview.

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
expand the existing preparation contract. Calls other than the admitted `IIf`
profile and unsupported dependencies
remain refused before user records are read, even when those records are corrupt.
Qualified same-table calculated dependencies are explicitly recomputed through the
bounded dependency profile below.

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
its restricted syntax and resolves dependencies within that same native table.
Qualified calculated dependencies are recursively recomputed; the resulting query
reads only ordinary scalar leaves. It returns ordinary leaf dependency names and
separate calculated dependency names in first-reference order, the original root
declaration, an owned query model and a bound plan. Binding
reads no user records and evaluates no formula, clock, provider or function.
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
ordinary same-table field references, unary plus/minus/Not, IS NULL/IS NOT
NULL, arithmetic `+`, `-`, `*`, `/`, `\`, `Mod`, `^`, explicit Text concatenation
`&`, all six comparisons and And/Or/Xor/Eqv/Imp. Duplicate field references do
not duplicate dependencies.
Query rules still determine operator behavior and eager failures; admitting syntax
does not assert complete Variant promotion or native calculated-field behavior.

Calls other than exact-arity unqualified `IIf`, domain/aggregate functions, clocks,
UDFs, subqueries, identity,
parameters, external/qualified references, collection members, date/GUID literals
and LIKE/IN/BETWEEN forms remain refused before row access. Self/cyclic calculated
dependencies remain refused through explicit active-path detection. Unknown fields
never become implicit parameters. SQL-like text inside
an admitted string literal remains inert data; expressions are moved into a
checked query model rather than interpolated into a SQL statement. Table names
containing closing brackets or control characters remain unsupported by this
preview's current identifier boundary.

## Deterministic IIf calls

Unqualified, ASCII-insensitive `IIf(condition, truepart, falsepart)` is the only
admitted function call. All three arguments must be present expressions; omitted
arguments, wildcard arguments, extra arguments, module qualifiers and `IIf$`
spellings remain refused. Arguments can contain the existing admitted operators,
ordinary fields, qualified calculated dependencies and further admitted `IIf`
calls. The `IS NULL` operator remains available; the `IsNull` function is not
admitted by this profile.

Microsoft's Access 2013 article
[Add functions to Access expressions](https://support.microsoft.com/en-us/office/add-functions-to-access-expressions-72a471a0-4f0e-4d49-923b-be97b0fa4bb0)
explicitly describes a three-argument `IIf` expression as usable in a calculated
field in a table. Its illustrated condition also contains `Date()`. This preview
admits the deterministic `IIf` structure only; it supplies no clock and continues
to refuse `Date()` and every unrelated function. The article establishes a narrow
table-calculation context, not complete ACE 16 formula compatibility.

Preparation recursively visits every argument before reading user rows, even
when a constant condition would select another arm. Hidden missing fields,
unsupported calls, invalid calculated declarations and dependency cycles therefore
fail on populated and empty tables. Argument traversal retains the same expanded
node/depth ceilings, cumulative native accounting and cancellation checks as
operators. No constant-folding shortcut bypasses that preflight.

Execution uses the existing query `IIf` implementation, including its eager
argument evaluation and NULL-condition behavior. Both branches are evaluated;
a division failure in an unselected branch still fails the complete buffered
preview. Selected results keep their natural expression carriers. Intermediate
calculated fields are not coerced to their native ResultType, caches remain
unread as operands, and successful previews establish neither cache freshness nor
native Access result equivalence.

Five focused adapter tests cover direct and transitive calls, NULL conditions,
natural mixed result carriers, corrupt-cache pruning, empty-table preflight,
hidden cycles and unsupported calls, eager late errors, structural limits,
cancellation and execution quotas. Existing declaration and dependency controls
remain active. Original public-writer/getter comparison evidence is recorded
separately; these focused tests do not establish native Access qualification.

## Same-table calculated dependencies

Qualified calculated fields may now reference other calculated fields in the same
table under this explicit preview. Microsoft's native table
[Calculated-data-type training](https://support.microsoft.com/en-us/access/training/video-combine-fields-using-the-calculated-data-type)
specifically permits calculated results as inputs to other calculated fields.
This establishes the same-table dependency boundary; it does not establish a
function whitelist or native intermediate conversion semantics.

Preparation uses the root declaration's captured schema/property set. Every
reachable calculated field must pass the existing native calculated-result,
Expression and typed ResultType gates before any user row is read. Dependencies
use existing ASCII-insensitive identifier resolution and canonical stored names.
Unrelated calculated formulas are not expanded. Cycles, missing fields,
unsupported reachable declarations, complex/nonlocal inputs and unsupported calls fail even
on empty tables or in operands whose value could otherwise hide them.

`PreparedCalculation::dependencies()` reports distinct ordinary leaf names;
`calculated_dependencies()` separately reports distinct referenced calculated
names, both in first-reference order. The selected root remains available through
`declaration()`. A diamond references a shared calculated name once in that
metadata, while the final deterministic expression may contain multiple copies
of its computation. No cached value becomes an ordinary input, including when
the originating source separately enables cached calculation queries.

Expansion allows at most 32 distinct referenced calculated dependencies and an
active dependency path of at most 32 declarations including the root. Visited
expansion nodes, including replaced alias references, are bounded by the caller's
`max_tokens` clamped to `ParseLimits::MAX_TOKENS`; expanded expression depth is
bounded by the similarly clamped `max_depth`. These structural controls prevent
unbounded alias chains and exponential shared-subgraph expansion. Cancellation
and cumulative native work/storage accounting remain active. Each formula parse
also retains its own existing byte/token/depth bounds; temporary parser workspace
remains structurally bounded separately from retained query storage.

Every intermediate uses its **recomputed expression carrier**, with no conversion
to the dependency's native ResultType. For example, a native LONG declaration
`Twice = [Input]*2` can naturally recompute an Int64 in the current default query
profile, and another expression receives that Int64. It receives neither a stale
LONG cache nor an implicitly narrowed replacement. The root likewise retains its
natural result carrier. This is engineering preview behavior; native Access
intermediate conversions, complete calculation semantics and freshness remain
unqualified. No result is written or substituted into an ordinary query source.

Six focused dependency tests cover chain/diamond/alias expansion, canonical name
reporting, declared LONG versus recomputed Int64, corrupt-cache pruning in ordinary
and cached-source modes, constant leaves, unrelated unsupported formulas, cycles,
unsupported transitive inputs, reachable declaration gates, late eager errors and
chain/expanded-node/caller limits. The existing native/query limits, cancellation,
metadata-only preparation and unsupported-call tests continue to pass.

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
[advanced-field contract](../../features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-scalar-calculations)
continues to own cached storage decoding and preservation.

## Independent CLI replay

The earlier [ordinary-getter replay](../../testing/CALCULATION_PREVIEW_REPLAY.md) records 44
controls and 1,018 independently modeled result cells across 20 selected
ACE 2016/2019 declarations. It compares recomputation separately from stale caches,
including ordinary NULL propagation, empty inputs, refusal and corrupted-cache
pruning. Its 26 pinned hashes describe the recorded earlier implementation.
Comparison declarations refused in that record are now admitted by the current
operator preview; function calls remain refused. Native formula evaluation and
freshness are still unqualified.


## Same-row predicate previews

Comparison and logical operators now reuse ordinary same-row inputs through the
same metadata-only preparation path. Every operand is validated and every
ordinary dependency is collected before row access, including operands of False
And or True Or. Calculated/nonlocal dependencies and function calls cannot hide
in an inactive operand. Execution remains eager: a later division error refuses
the complete result even when a Boolean operand could determine the truth value.

Nullable Boolean truth tables and integer bit masks retain the existing query
kernels. A declared Int32 calculation can therefore preview a natural Boolean
result without narrowing it to a stored carrier. Text comparisons still require
an explicit host comparer; the CLI supplies no inferred locale or new comparer
option in this slice, so unsupported Text comparisons fail explicitly. Numeric
and Boolean previews need no ambient locale. Stored caches stay unused and can
be deliberately stale or NULL; successful recomputation proves no freshness or
native Access result equivalence.


The focused predicate batch passes five syntax tests, ten adapter tests and eleven
CLI tests with strict scoped adapter/CLI Clippy. The
[independent predicate replay](../../testing/PREDICATE_PREVIEW_REPLAY.md) passes 248
controls and 312 recomputed cells over fresh ACE 2016/2019 originals. Every
recomputed cell differs from its stale cache, and all twelve pinned hashes match.
These are engineering formula-model checks; native Access execution remains
unqualified.
