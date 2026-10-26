# Execution-phase named crosstab schemas

`tablecraft-query::execute_with_schema_discovery` now executes read queries that
use named crosstabs whose columns require parameters, execution policies or data.
The CLI `query run` command uses this service for direct and saved reads. Ordinary
`prepare` remains metadata-only and refuses schemas it cannot derive without
execution. Inspection, inventory, parsing and `query saved` never invoke discovery.

| Phase | Behavior |
| --- | --- |
| Metadata binding | Bind names, declarations and supported expressions without source scans. Request a named crosstab schema when required. |
| Explicit discovery | Execute that crosstab completely under the current parameters/providers and shared budget. Discover its names and retain all result cells. Resolve its own named dependencies first. |
| Final binding | Bind outer sources against the discovered schema, preserving independent parameter scopes and text-policy provenance. |
| Execution | Read the completed per-run crosstab result. Repeated aliases reuse that result; they do not evaluate its cells again. |

Each materialization must finish successfully before becoming available. Errors,
limits, cancellation or later inner-cell failures abort the operation without a
partial published result. An outer projection, WHERE or TOP cannot suppress an
error in the materialized crosstab. Existing fixed-heading views that bind without
discovery keep their ordinary execution behavior; the service does not proactively
materialize every named query.

## Context, parameters and result identity

Parameters retain exact declared carriers, compatible-name merging and isolated
named scopes. A discovered child receives the applicable subset of supplied values;
the final plan still checks the complete parameter list. Date/number/text policies,
clock and caller-owned random state are shared through the operation. Copying
ExecutionOptions copies provider references, not their state. No execution policy,
parameter value, discovered schema or result is cached between calls.

A completed crosstab is a per-run result snapshot. This avoids rerunning volatile
cells solely to discover their names. Repeated references to a discovered query
see its same values. This is an explicit TableCraft execution policy; native Access
optimizer/volatile evaluation equivalence remains unqualified.

Expression-valued cells remain ExpressionValue, including CVErr data. They do not
become stored NULL or a scalar source schema. Row-heading and aggregate-cell text
policy identities propagate through the discovered columns to outer comparisons.
No aggregate carrier type is guessed from rows. Existing heading admission,
name-collision, column-width and collation rules remain authoritative.

```sh
tablecraft query run 'SELECT Region,[2] FROM CrossDynamic' --database sales.accdb
```

This example assumes a stored CrossDynamic crosstab with that discovered heading.
If the current result has no column 2, binding reports the missing name. A dynamic
empty table exposes its row-heading columns without inventing pivot columns;
a contextual fixed IN list retains its declared headings even with no data.

## Budgets and source contracts

One clamped QueryLimits budget covers binding attempts, discovery, retained data,
repeated result copies and final execution. Binding has its existing independent
node/depth/byte ceilings as well. Named dependency nesting is capped at fifty;
per-run discovered results are capped at 256. Composed runtime reads and suspended
named syntax also share the [active interpreter allowance](QUERY_BINDING_LIMITS.md#composed-interpreter-nesting).
A domain-triggered discovery runtime inherits the caller's active read count;
retries and completed materialization do not reset that allowance or the separate
sixteen-call domain nesting counter. Direct crosstab execution enters the read
scope once, including when the staged runtime has no outer SELECT wrapper. The dynamic
fifty-definition probe reaches a checked structural refusal, so the larger named
path ceiling does not qualify every discovery chain. These are resource policies,
not native Access format limits. Increasing an I/O allowance does not reset these
query quotas. Every stage retains cancellation and fallible charged allocations.

The selected Source must keep its definitions, schemas and data stable for the
entire operation. Discovered definitions are checked again during binding; changed
bodies or retrieval families fail with SchemaChanged. This is not coherent live
Access/shared-file coordination, a transaction across providers or persistent
result caching. Discovery can read data before a later outer binding/parameter
failure; results remain buffered and unpublished on failure.

The library service owns staging. CLI code composes its existing selected source,
parameters/providers and output presentation. Native I/O stays in ACE/Access;
backend-neutral query code imports neither adapter. No new crate, registry
library, serializer, scripting/GUI host or write behavior is introduced.

## Remaining scope

Discovery covers relational named-source dependencies and runtime-only domain
targets. Domain calls can request first-use discovery and reuse completed results,
preserving their names, policies and cells; they still reject parameterized domains.
Nested requests share the same cache, active dependency path and cumulative budget.
No expression evaluation runs while a cache metadata borrow is held. The synchronous
state is internal to this operation; providers cannot access or mutate it. Inline TRANSFORM sources, additional heading carriers,
full native naming/collation/coercion and native application comparison remain
unfinished. Unknown functions/providers stay refused rather than activated.
The full read/query and backend-before-UI roadmap is not complete.

Microsoft's [TRANSFORM reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql)
describes fixed and data-derived pivot headings; [QueryDef.OpenRecordset](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/querydef-openrecordset-method-dao)
is a separate explicit execution operation. These references motivate the
inspection/execution distinction; they do not establish TableCraft's native
snapshot, volatile-state or error-order equivalence.

## Evidence

Seven discovery feature tests cover repeated aliases/runs, nested dependencies,
parameter and number policies, cycles, Error/volatile cells, empty inputs, cumulative
row quotas, every observed cancellation checkpoint, changing definitions and late
source failures. A separate routing test proves that discovered Text cells retain
an explicit field policy through outer comparison. Existing metadata-only no-scan
and constant-schema controls remain passing.

The original [DiscoveryCrosstabFixture.java](../testing/DiscoveryCrosstabFixture.java)
adds five inert definitions to the original forty-two-query computed fixture.
The [independent runner](../testing/compare_schema_discovery.py) passes 100 getter/
explicit-grid/direct/saved/named/inert/refusal controls over unchanged Jet 4,
ACE 2007 and ACE 16-family files. The existing computed-heading recipe also passes
204 controls with its contextual CLI case updated to this execution path. Reference
getters/rendering never execute SQL. Native Windows Access remains unrun.


Domain integration adds two tests covering repeated aggregates, missing criteria,
volatile identity, Error cells and transitive parameter refusal. Five later
runtime-domain controls and a field-policy routing control cover first-use and
nested staging, cache extension, cycle/parameter failures, quotas, cancellation
and late failures. The extended original runner passes 148 controls, including domain
aggregation and Error/volatile reuse across all four unchanged native inputs.
