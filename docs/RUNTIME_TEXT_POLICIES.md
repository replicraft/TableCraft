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

## Comparison consumers

Predicates, BETWEEN, membership/quantifiers and LIKE combine static provenance with
the compared cells. Groups, DISTINCT/UNION keys, sort/TOP ties and pivot headings/
buckets retain cells until comparison finishes. This prevents a derived alias or
cached named result from silently losing a runtime domain policy.

Sort and set-key columns merge their runtime contributors before key comparison.
Group and discovered-heading representatives retain the policies of matching cells
that are discarded. A neutral literal cannot bridge incompatible field policies
by erasing an intermediate contributor. Grouped output expressions, including
composed and correlated expressions, inherit their group's retained key policy
through the shared metered structural-identity service. Expressions still evaluate;
this is policy propagation, not a new value cache or skipped error path.

Fixed pivot headings are rechecked when execution supplies a policy unavailable
in static metadata. Headings that become equivalent under that policy fail with
AmbiguousName rather than silently producing duplicate buckets. Dynamic heading
ordering and bucket comparisons use the accumulated pivot policy.

Default is neutral; equal identities remain one; differing identities become
Conflict. Required unknown/conflicting policies fail through the existing resolver.
Raw projection remains inert, and explicit global comparison overrides retain their
existing behavior. Numeric/NULL comparisons do not activate a text resolver merely
because a cell has provenance. The host still owns the policy namespace and provider
budgets; no LCID/default locale is inferred.

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

The original [native policy runner](testing/compare_native_collations.py) passes
1,460 model/source controls over the four unchanged registered Jet 4, ACE 2007
and ACE 16-family inputs. DLookup/DFirst/DLast/DMin/DMax with unique row criteria
reuse independently encoded short-key expectations, both sequentially and through
an explicitly selected index. Only TableCraft evaluates SQL.

Microsoft documents [DLookup field/expression results and query usage](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlookup)
and [database/field collating orders](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao).
These motivate retaining provenance; they do not specify this conservative merge
model or prove native domain-result precedence. Native Access qualification, other
locales and concrete Text/LCID providers remain unfinished. This changes no native
writer, encryption codec, dependency, stored value or source-schema contract.
