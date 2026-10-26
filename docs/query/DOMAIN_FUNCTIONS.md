# Read-only domain functions

Bound `query run` retrieval implements DAvg, DCount, DFirst, DLast, DLookup,
DMin, DMax, DSum, DVar, DVarP, DStDev and DStDevP. Each accepts two or three
expression arguments: expression text, domain name, and optional criteria text.
This is an implemented partial profile, not complete Access qualification.
Source-free `query eval` and UPDATE do not gain domain access.

DMin/DMax now retain [lossless String extrema](aggregates/EXPRESSION_AGGREGATES.md#retained-string-extrema)
through the shared Min/Max kernel. Raw-aware callers can observe selected bytes;
actual comparisons require the selected String provider. Scalar consumers publish
valid selected Text explicitly, and final unrepresentable String output still
fails. Domain namespace, parameter restrictions, criteria, NULL rules, shared
budgets and required field policies remain in force.

```sh
tablecraft query run --database example.accdb \
  "SELECT DCount('*','Orders','Amount>100') AS OrdersOver100"
```

## Source and evaluation

The domain resolves only through the caller-selected `Source`, as a local table
or a parameterless named SELECT/UNION/crosstab. Transitive declared
query parameters are rejected even if the outer query has matching bindings.
No outer parameters or fields are implicitly captured inside the strings. Outer
argument expressions can explicitly construct the strings using their own bound
fields and parameters. Preparation captures those dependencies without resolving
dynamic domain names or scanning rows.

All supplied arguments evaluate in written order before string validation. This
profile requires Text carriers; explicit NULL is InvalidNullConversion and other
carriers are TypeMismatch. An omitted final argument slot is omission, not NULL;
missing required expression/domain slots remain errors. Omitted or whitespace-only criteria select the whole
domain. Criteria contain a scalar expression without WHERE, with the qualified
[named collection presence exception](collections/COLLECTION_PRESENCE.md#domain-criteria). A missing criteria
field returns NULL; missing expression fields, source failures and unsupported
syntax remain errors. Subqueries and object/member access inside strings are
rejected before binding, so their lookup failures cannot masquerade as missing
criteria fields. SQL and domain aggregates inside expression text are rejected.
Criteria may contain supported domain calls under the same recursion limits.

Runtime parsing, binding, scans, aggregate buffers and results share the query's
cumulative allocation/work/row limits and cancellation callback. Parsing has the
existing syntax ceilings, binding retains its per-plan ceilings, and nested
domain execution is limited to the named-query depth ceiling. Calls do not create
fresh public executions or reset quotas. Through execute_with_schema_discovery
(the CLI query-run service), a runtime-only target can request [execution-stage
schema discovery](NAMED_SCHEMA_DISCOVERY.md). Domain calls reuse a crosstab result
already completed in that operation, including relational dependencies.
They bind its retained names and text policies and read its expression-valued cells,
without rescanning its underlying tables or rerunning volatile cells. This is the
existing operation-local result, not a separate domain cache. Parameters remain
forbidden transitively, including when a completed result exists. Declarations
observed during binding reject the request before its discovery scan; a failure
can follow earlier, unrelated work in the enclosing query. Discovery dependencies
share the operation's active-name cycle checks and sixteen-level depth ceiling;
ordinary nested domain evaluation retains its own sixteen-level guard. Existing
PreparedQuery execution does not initiate this staging and keeps its previous
fixed-schema domain profile. Payload-free
operation spans contain no expression, domain, criteria, parameter or row data.

## Results and text policies

The implementation reuses ordinary aggregate kernels, including the
[wider mean accumulator](aggregates/AVERAGES.md) for DAvg and
[exact binary totals](../testing/FLOATING_SUMS.md) for floating DSum. DCount('*',...) counts
rows; other DCount expressions omit NULL and currently return Int64. Empty counts
return zero. DCount also counts expression Error cells as non-NULL without code
conversion; invalid constructors and operational failures remain failed calls.
DCount also observes [lossless String nullness](aggregates/EXPRESSION_AGGREGATES.md#counting-lossless-string-expressions)
without decoding raw units or odd byte ranges. Its expression and criteria still
evaluate under the shared budget; invalid constructors remain errors.
See the [explicit counting contract](../expressions/VARIANT_ERRORS.md#counting-expression-error-values).
Other empty aggregates return NULL. DFirst/DLookup and DLast use
first/last source encounter, including NULL, and evaluate the expression over all
selected rows. These selectors now retain [expression Error data](../expressions/VARIANT_ERRORS.md#encounter-order-aggregates-and-domains)
without converting it to NULL or a numeric code. Explicit consumers and the
expression-result API can use it; scalar-only publication still rejects it.
An unselected Error value is data, while late operational failures still fail
the entire call. Encounter order is not an Access ordering guarantee. A unique
DLookup criterion avoids depending on it. Numeric and statistical results follow
the existing [aggregate profile](aggregates/STATISTICAL_AGGREGATES.md), including DVarP's
fewer-than-two-values NULL boundary. DStDevP also requires two values and returns
NULL for a singleton, separately from SQL StDevP's singleton-zero profile.

Criteria and DMin/DMax use captured domain field text policies through the supplied
resolver. Returned domain Text keeps Default static metadata because dynamic
string input does not establish a statically bound field lineage. Internal
[execution cells](RUNTIME_TEXT_POLICIES.md) now retain its actually bound result
policy through enclosing predicates, aliases, groups, sorts, sets and named
materialization. Preparation remains inert; no dynamic target is guessed.
Unknown required policies fail without fallback. Native reads retain the existing
stable/offline input, source schema validation and explicit collection opt-ins.
Ordinary scalar subsets of mixed tables work without expanding unused children.
Direct collection/member references inside domain strings remain unsupported.

## References and remaining work

Microsoft's [DCount](https://learn.microsoft.com/en-us/office/vba/api/access.application.dcount)
and [DLookup](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlookup)
references define parameterless domains, criteria, NULL/count and lookup behavior.
Its [DVarP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dvarp)
and [DStDevP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dstdevp)
references define the small-population boundaries. They also describe broader form,
control and supplied-function contexts that this read-only implementation does
not supply. External providers are not opened merely by naming a domain.

Remaining work includes broader criteria/subquery and collection semantics,
applicable explicit callable/host contexts, native Variant coercion and subtype
qualification, optional NULL/empty criteria behavior, error/evaluation order,
encounter order and mixed-result text precedence. Windows comparisons remain
pending at the user's direction. Current Rust/CLI controls establish the stated
profile; they do not establish native Access execution equivalence.


## Stored-query evidence

The original [SavedDomainFixture](../testing/SavedDomainFixture.java) builds fifteen
inert definitions over the existing statistical recipe and one Unicode-named
local table. [compare_saved_domains.py](../testing/compare_saved_domains.py) verifies
raw native metadata and independently rendered SQL, then executes direct, saved
and named forms through TableCraft against rational/getter expectations. All
300 controls pass across original Jet 4, ACE 2007, V2016 and V2019 files, with
unchanged input hashes. These include all twelve functions, omitted criteria,
explicit root parameters, grouping, derived inputs, UNION, fixed crosstabs,
nested criteria calls, Unicode names and recursive/unknown/parameterized-domain
and late-error refusals. This establishes TableCraft integration and independent
model agreement; Jackcess does not execute the queries and Access remains unrun.

Domain expression/criteria parsing now uses the shared strict expression entry
point rather than parsing a second parenthesized guard copy. Existing scalar
shape/name restrictions, shared budgets and typed errors remain in force; a
statement terminator is still refused while quoted semicolons remain data.

Domain expressions now inherit the enclosing explicit [Access session context](../expressions/SESSION_FUNCTIONS.md).
DFirst/DLast/DLookup of CurrentUser materialize that supplied name; they do not
open a workgroup, infer a database login or change any permission.


## Runtime discovery controls

Five runtime-domain tests cover first-use discovery, per-run reuse, nested criteria,
discovery during another crosstab's execution, live cache extension, dependency
cycles, transitive parameter refusal before scans, cumulative row quotas, every
observed cancellation checkpoint and late failures/definition changes. A routing
test checks stored field policies in discovered-domain criteria. Original native
recipes additionally cover runtime-only counts, sums, nested targets, Error/volatile
lookups and buffered refusals across the four priority-family inputs. These do
not qualify native Access optimizer order, snapshot lifetime or volatile semantics.

## Expression extrema and numeric Error admission

DMin/DMax now share the provisional standard Error-only extrema profile, retaining
Error data and ignoring NULL. Error mixed with a non-NULL scalar fails TypeMismatch;
opaque codes are refused. DSum/DAvg and statistical domain aggregates reject Error
without implicit code conversion. The [expression aggregate contract](aggregates/EXPRESSION_AGGREGATES.md)
records shared budgets, failure/publication boundaries and native qualification gaps.


## Return declaration provenance

All twelve implemented domain functions now carry their documented Variant return
category through the explicit `query run --declared-arithmetic` profile. The
[reviewed signatures and controls](../expressions/numeric/DECLARED_ARITHMETIC.md#domain-function-return-declarations)
are separate from successful scalar carrier inference. The cell retains its
original Value/Error state and runtime text policy; no source record establishes
its declaration category. Explicit casts can request Scalar overflow behavior.
Ordinary SQL aggregates retain their separate declaration boundary.
This adds no binding scan, provider activation, conversion, runtime host or new
stored/public Variant carrier. Native SQL type classification and the existing
coercion, locale, encounter-order and aggregate precision gaps remain open.


## Named collection presence in criteria

Qualified local-table named collections now support IS NULL / IS NOT NULL
criteria, Boolean combinations and supported scalar composition in runtime domain
calls. These use the
same typed parent filter as SELECT WHERE, preserving parent cardinality without
expanding attachments into aggregate input rows. Scalar domain value expressions,
member-access refusals, missing-field NULL results and isolated parameters remain
unchanged. Nested domain criteria retain the owning depth/work/row/allocation
budget and eager/complete-read failures. Native attachment calls use the complete
metadata cache without loading FileData, including with decoded-payload opt-in.
See [the exact scope and evidence](collections/COLLECTION_PRESENCE.md#domain-criteria).
