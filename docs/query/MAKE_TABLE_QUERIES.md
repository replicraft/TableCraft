# SELECT INTO query staging

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
immutable original image. It resolves native UTF-16 Text widths, Binary widths
and Decimal precision/scale from the actual source fields, then runs the moved
retrieval against a reader constructed over that same image. It retains the
complete typed rows and an explicit unadorned manual declaration; no Required,
default, primary/index or generated property is inherited. Current preparation
refuses filters, groups, computed projections and broader source contexts rather
than guessing their physical declarations.

The separate native populated append candidate consumes rows without cloning
text/binary payloads, packs them by actual record/page/slot geometry, allocates
owned/candidate DATA maps (including represented bitmap storage), and checks
complete readback plus the protected original byte complement. Independent
whole-image fixtures cover nullable numbers, Boolean value bits, Text/Decimal and
mixed GUID/date/IEEE fixed values across DATA pages and map windows. There is no
additional aggregate 64-row quota. Per-record capacity, native count/file geometry
and explicit caller budgets still apply.

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
Runtime review and normal SQL Run publication remain separate acceptance
boundaries. An empty CREATE followed by another save is not a substitute.
