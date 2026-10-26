# Runtime text-policy provenance

Query execution now carries an internal cell containing the existing expression
value and a host-issued TextPolicy. Stored Value/ExpressionValue and public result
rows retain their existing representation. The cell is private to the query
implementation; it has no Debug/Serde implementation or engine-specific locale
encoding. Backend-neutral query code still imports no concrete adapter.

## Static metadata and runtime results

PreparedQuery::output_text_policies remains metadata-only static provenance.
A domain target can depend on parameters or row expressions, so its static policy
remains Default. Execution enriches its returned cell with the policy of the
actually bound domain expression. Default in prepared metadata therefore does not
promise that every runtime Text result uses the host default.

DLookup/DFirst/DLast/DMin/DMax retain result policy through derived and named rows,
scalar subqueries, aliases, UNIONs and operation-local crosstab materialization.
Domain arguments, criteria and parameter scopes retain their earlier validation
and execution order. No domain scan or provider activation is added to preparation.
No result or formatted argument survives into a later execution.

CVar preserves cells. IIf, Nz, Switch and Choose merge contributing result policies;
conditions and selector indexes do not supply output provenance. Concatenation and
text-capable addition retain contributing policies. Encounter/extrema aggregates
merge policies from the argument cells they evaluate, even when a different cell
becomes the selected value. Fresh text built-ins/conversions retain their existing
Default policy; this extension does not infer native Left/LCase/CStr precedence.

### Numeric UNION rendering

An actual Boolean or numeric cell promoted to Text by UNION now establishes fresh
Default policy after successful CStr rendering. A numeric cell can retain a latent
policy from an unselected String branch; that field did not contribute characters
to the rendered number. Keeping its identity caused otherwise valid set/group/sort
comparisons to fail. Explicit CStr of the same numeric expression already used the
fresh policy.

The rendered cell marks its runtime provenance complete so that its own arm's
conservative static metadata cannot reintroduce the discarded numeric-source
policy. Actual Text/raw String cells, NULL cells and other expression states are
unchanged. Per-arm policy merging still retains actual String contributors and
static fallback for empty arms, including empty String declarations. NULL-origin
policies remain conservative. Written-prefix DISTINCT stages keep their existing
order; no later arm retroactively changes an earlier stage.

Rendering still happens once through the existing metered CStr kernel before
comparison. Failed conversion, missing formatting, nonfinite values, limits and
cancellation do not publish a completed result. This corrects the existing host
policy model; it does not qualify native Access UNION collation precedence.

## Comparison consumers

Min/Max and their domain equivalents likewise defer uncertain String selection
until all argument policies are known. A later runtime field order can otherwise
make an already discarded value the actual extreme. Only initially Default
String populations with a resolver retain candidate cells; stable policies and
non-String populations keep their one-cell path. NULL contributors still supply
policy. Arguments execute once, strict mixed-family/state failures remain
explicit, and all retained storage/comparisons use cumulative controls. This
fixes the existing host-policy model, not native mixed-policy precedence.

Predicates, BETWEEN, membership/quantifiers and LIKE combine static provenance with
the compared cells. Groups, DISTINCT/UNION keys, sort/TOP ties and pivot headings/
buckets retain cells until comparison finishes. This prevents a derived alias or
cached named result from silently losing a runtime domain policy.

Sort and set-key columns merge their runtime contributors before key comparison.
For DISTINCT/DISTINCTROW, already evaluated ORDER BY keys merge their contributors
before duplicate rows are discarded. A duplicate may carry the only nondefault
policy even when its retained representative has the same value; removing that row
must not change the ordering or TOP selection of surviving values. This merge does
not reevaluate expressions or activate a provider, and fresh conversion/function
results retain their separate Default policy.
With a field resolver, GROUP BY evaluates each row's group keys once and retains
all keyed rows until key-column runtime policies have been merged. Membership is
then computed under those complete policies: a late field identity can both merge
neutral groups and separate values that the default comparer would equate.
Each tuple column retains its own identity, including across partitions separated
by other numeric keys. Retained group representatives carry the resulting policy
through output expressions and named/crosstab consumers. Additional keyed-row
headers use fallible allocation under the existing cumulative byte/work limits;
no source is rescanned and no key is reevaluated to repartition. A fixed allowance
can therefore exhaust sooner. Without a field resolver, global-comparer grouping
keeps its existing incremental key storage.

Group and discovered-heading representatives retain the policies of matching cells
that are discarded. A neutral literal cannot bridge incompatible field policies
by erasing an intermediate contributor. Grouped output expressions, including
composed and correlated expressions, inherit their group's retained key policy
through the shared metered structural-identity service. Expressions still evaluate;
this is policy propagation, not a new value cache or skipped error path.

The scalar and lossless string paths now share that result-policy walk. A
composed grouped key, such as a runtime domain result joined with empty Text,
retains its accumulated policy when reused inside another `&`/`+` or CVar
operand, including correlated reuse. Previously the string-operand path could
re-evaluate the neutral first row and lose a policy contributed by later rows in
the group. Metadata matching now reattaches that retained policy after evaluation.
Domain-expression arguments do not become output provenance, even when their own
field policy is unsupported. Fresh CStr/Left results keep the existing Default
contract; this does not infer native conversion-function precedence.

Fixed pivot headings are rechecked when execution supplies a policy unavailable
in static metadata. Headings that become equivalent under that policy fail with
AmbiguousName rather than silently producing duplicate buckets. A dynamic pivot
with initially Default provenance and an explicit resolver retains its non-NULL
pre-WHERE candidate cells until their final accumulated policy is known. Heading
comparisons then use that final policy once: a later field can make earlier values
equivalent or distinguish values the default comparer would have merged. The first
representative remains and all matching rows enter its one bucket. Candidates are
validated before WHERE and copied under cumulative byte/work limits, with no early
schema-width cutoff; only final distinct headings use the 256-column ceiling.
Additional candidate retention can exhaust a fixed query allowance sooner. Pivot
expressions are not reevaluated and sources are not rescanned. Without a resolver,
or with an initially fixed field identity, existing incremental admission remains
applicable. Explicit fixed headings retain their declared width and still fail on
equivalent headings. Dynamic heading ordering and bucket comparisons use the
accumulated pivot policy.

Default is neutral; equal identities remain one; differing identities become
Conflict. Required unknown/conflicting policies fail through the existing resolver.
Raw projection remains inert, and explicit global comparison overrides retain their
existing behavior. Numeric/NULL comparisons do not activate a text resolver merely
because a cell has provenance. The host still owns the policy namespace and provider
budgets; no LCID/default locale is inferred.

## Crosstab cell expressions and row-group policies

A crosstab cell now receives the complete row group's retained key policies,
just as its SELECT projection and HAVING expression do. Previously the cell
execution frame omitted those keys: a repeated grouped domain expression could
then compare under the first bucket row's default policy instead of the final
required field policy. A grouped SELECT could produce 4 while the corresponding
admitted TRANSFORM expression produced 0. This was an engineering wrong-answer
bug inside the existing explicit provider profile.

Bucket aggregates still receive only their own member rows. Their operand
frames still clear group-key reuse and evaluate each input under its original
policy. Passing the complete group context to an expression outside an aggregate
does not turn every per-row operand into the group's selected key. No expression
is cached or skipped, and the change adds no scan or provider inference.

Paired grouped SELECT/TRANSFORM controls cover multiple pivot buckets,
correlated group-key reuse, parameterized named execution with changed inputs,
NULL groups, conflicting policies, required-only providers and explicit global
overrides. Per-row aggregate controls distinguish this context from a blanket
policy override. Prefix cancellation, shared limits and late eager cell failures
still return no partial result. These controls do not establish native Access
TRANSFORM syntax, mixed-policy precedence or optimizer evaluation counts.

## Memory, work and publication

Cells, key rows and correlated scope frames charge their actual type widths under
shared byte ceilings.
Copies retain fallible Text/Binary allocation and copy only the existing expression
payload plus policy. Runtime provenance walks, comparisons, domain binds/reads,
sorting, pivoting and result publication check the shared work/cancellation budget.

Public expression results unwrap cells only after evaluation and named discovery
finish. This requires another fallible, metered row-header/value-vector allocation;
payloads move without cloning. Public scalar results retain their checked scalar
conversion boundary. Neither API exposes partial output on failure. Internal
materialized results keep their cells until the operation ends. Extra provenance
and publication accounting can exhaust a fixed byte/work allowance earlier; no
limit is silently raised and no performance improvement is claimed.

## Evidence and qualification

Focused controls cover runtime-selected target fields, policy changes between
executions, selectors, aliases, groups, ordering/TOP ties, unions, membership,
unknown/conflicting identities, inert projection and named-crosstab materialization.
Metadata preparation performs no scans. Byte limits and cancellation at every
observed domain/publication checkpoint remain enforced.

The original [native policy runner](../testing/compare_native_collations.py) now passes
1,844 model/source controls over four freshly regenerated Jet 4, ACE 2007
and ACE 16-family inputs, each unchanged by the run. DLookup/DFirst/DLast/DMin/DMax with unique row criteria
reuse independently encoded short-key expectations, both sequentially and through
an explicitly selected index. Added controls reuse grouped composed domain keys
inside nested string operators/CVar, with General Legacy/2010 fields, database
policy and global overrides. Source hashes are recorded in
[fixture provenance](../testing/FIXTURES.md#grouped-string-policy-provenance-controls).
Only TableCraft evaluates SQL.

Microsoft documents [DLookup field/expression results and query usage](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlookup)
and [database/field collating orders](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao).
These motivate retaining provenance; they do not specify this conservative merge
model or prove native domain-result precedence. Native Access qualification, other
locales and concrete Text/LCID providers remain unfinished. This changes no native
writer, encryption codec, dependency, stored value or source-schema contract.
