# SELECT INTO staging and reviewed native publication

`tablecraft-query::plan_make_table` stages a local make-table query as complete
stored scalar rows and query declarations. It does not create a native table,
open a destination database, issue a save capability or enable DDL in SQL View.
Native publication must use the actual reviewed creation owner and independently
recheck the source, destination namespace and physical declarations.

Microsoft's [Access SELECT INTO contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-into-statement-microsoft-access-sql)
creates a new table from a retrieval over tables or a query. An existing target
name is an error. Fields inherit source data type and field size; other source
field/table properties do not transfer. Primary keys and indexes are not invented.
External destination IN syntax exists, but requires a separate host authorization
and publication contract and is refused by this staging profile.

## Ownership and execution

The planner consumes the original owned `Query`. It moves its SELECT, parameters,
source references, predicates and ordering into an ordinary retrieval without
cloning the AST. The returned `MakeTablePlan::retrieval()` retains that context for
an eventual physical-shape resolver. The plan owns one complete result; `into_rows`
moves its payloads after the publisher has resolved its own declarations.

Destination `Source::columns` and `Source::query` metadata are checked before user
record scans. Only UnknownName/absent query metadata permit staging; other source
errors propagate. A named-query collision does not execute the stored definition.
The host must still revalidate its complete native namespace before publication.

Retrieval uses the same staged schema discovery and captured scalar/Variant,
parameter, numeric/date, collation and execution-provider rules as existing query
execution. It shares the caller's cumulative budget through binding, discovery,
execution, scalar publication and declaration validation. Allocated vectors grow
fallibly; work and cancellation checks continue through output validation. There
is no additional make-table row or byte quota.

## Query declarations are not native fields

`MakeTableColumn` retains the output name, exact established `TypeDescriptor` and
query-output nullability. Empty results retain their known schema; all-NULL values
with a known source descriptor do not trigger row-based type inference. Selected
arithmetic profiles retain their own output descriptors.

`output_nullable` describes the retrieval, not a destination Required property.
No source default, primary key, generated-value setting, index or validation rule
is copied. Actual selected NULLs remain NULL; duplicate source IDs are not
regenerated or made unique. Expression-only states cannot silently become stored
NULL or another scalar.

TypeDescriptor Text lengths use the query model's Unicode-scalar units. They do
not establish native UTF-16 field width, Short Text versus Memo storage, Binary
field size or other unrepresented native declarations. The publisher must resolve
those facts from actual source metadata and the retained retrieval context rather
than treating query descriptors as ready-made native CREATE COLUMN instructions.

NULL-only and Dynamic outputs have no established scalar destination type and
are refused, including when returned rows happen to share one carrier. Whole
lists/recordsets have no ordinary scalar destination declaration. Duplicate output
names are refused under the query profile's ASCII-insensitive comparison; no
unverified native locale-name policy is inferred. Output declaration validation
follows complete retrieval/discovery, so these refusals can follow source reads.
Eager expression or source errors remain errors even when TOP/filtering would
otherwise omit a row. No partial make-table plan is returned.

## Restricted native preparation and atomic adapter save

The Access adapter now has a direct local scalar-projection preparation over one
immutable original image. It resolves native UTF-16 Short Text widths
and Decimal precision/scale from the actual source fields, then runs the moved
retrieval against a reader constructed over that same image. It retains the
complete typed rows and an explicit unadorned manual declaration; no Required,
default, primary/index or generated property is inherited. Current preparation
admits explicit field projections and aliases from one local table, with direct
source-field ordering. It refuses filters, groups, computed/wildcard projections,
parameters, named/joined/external sources, Binary, Long Text and extended date/time
declarations rather than guessing their physical shape. The native-comparison
entry point constructs the database-default and exact field-policy resolver from
that same captured image, using a separate cumulative comparison budget. Unknown
policies never silently become binary comparison.

Publication requires the positively qualified retained ACE 16 source with actual
MSysDb metadata, unencoded pages, complete catalog/page ownership and the existing
constraint, dependency and required-program checks. Header recognition or a
successful query preparation alone grants no native save authority. The
[append-table contract](../access/writes/ADD_TABLE_PERSISTENCE.md) retains the
source and allocation boundaries.

The separate native populated append candidate consumes rows without cloning
text/binary payloads, packs them by actual record/page/slot geometry, allocates
owned/candidate DATA maps (including represented bitmap storage), and checks
complete readback plus the protected original byte complement. Independent
whole-image fixtures cover nullable numbers, Boolean value bits, Text/Decimal and
mixed GUID/date/IEEE fixed values across DATA pages and map windows. There is no
separate aggregate row-count quota. Per-record capacity, native page/count/file
geometry and explicit caller budgets still apply.

Preparation remains inert. With journaling enabled, its consuming `finish` invokes
the native populated planner on the same original image and materialized rows.
The offline adapter then requalifies that reviewed plan against its exclusively
held actual source, comparing complete BEFORE and AFTER images before setup.
One V3 journal saves the schema and all rows atomically, including an empty result.
Its distinct `TCPT0001` intent retains the original declarations, cells and source
binding; recovery reconstructs those bytes without rerunning SQL or defaults.
Unknown versions and builds without creation authority do not retry an old action.

Applied receipts report the table addition and inserted rows, with no matched-row
count. Recovery retains its actual outcome without inventing affected-row counts.
The [qualification record](../../tests/results/queries/ace-sql-parameters-and-atomic-make-table-2026-11-11.json)
covers same-file saves, zero rows, stale/cancelled admission and cold reconstruction.

## Runtime review and SQL View

Normal SQL View Run now classifies SELECT INTO as a schema action and sends the
runtime-issued inspection through `ReviewSchemaSql`. Private source identity and
document revision are checked before parsing or native reads. The consuming
`ReviewedTableCreation` owns the selected populated plan, not an empty-table
profile or synthetic Design draft. Review holds no file lease; save requalifies
that complete plan under the exclusive source owner without rerunning SQL.

The review shows the destination, declared field count and selected record count.
No preserves the source and previous results; Yes uses the same retained save
worker, terminal acknowledgment and guarded whole-image reopen as other schema
operations. The SQL tab/draft survives. The host publishes fresh catalog state
without rerunning the action or treating inserted counts as query result rows.
Zero selected records still create the declared empty table atomically.

`CreatedTableReopen::inserted_rows()` distinguishes populated creation, including
zero, from an empty CREATE request. An applied receipt must match that exact
inserted count and table-added schema, with no old-row matched count. Recovery
and confirmation keep their actual count-free receipts. Complete AFTER comparison
remains mandatory before fresh publication.

New Text declarations use the destination's masked ROOT database default, not a
projected field's custom sort policy. Native construction currently admits exact
General Legacy and General 2010 defaults; zero, unknown and other defaults,
including French Legacy, refuse new Text construction. Non-Text construction
does not acquire that restriction. Fresh canonical databases retain their authored
General 2010 default. Two native whole-image checks pass for empty and populated
construction, including a source field whose custom sort differs from the ROOT
default, and for unsupported-default refusal without blocking non-Text creation.
This follows [Jackcess's unspecified textual-column default selection](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/TableMutator.java#L98-L102);
it does not claim native Windows/Access qualification for other sort families.

The [four-frame presentation record](../../tests/results/queries/make-table-review-presentation-2026-11-11.json)
records English/German light/dark offscreen review frames and their reference gaps.
The connected workflow checks passed 10 adapter preparation cases, 391 runtime
cases with one ignored, six focused UI cases and 32 selected desktop cases,
plus strict lint across seven affected packages and minimal-feature checks.
After the ROOT-default correction, 21 adapter creation/recovery cases, 34 selected
runtime creation cases and a creation-only engine library check also pass. The
final selected native followup passes 384 cases with five ignored, including
both destination-default checks and old creation/TCPT regressions; the full
runtime rerun passes 391 with one ignored, and the strict followup passes.
These are local checks of the stated source/profile, not complete ACE SQL or
Windows/Access parity. The earlier checkpoint remains immutable historical evidence.
