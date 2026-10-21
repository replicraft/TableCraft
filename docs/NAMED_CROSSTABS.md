# Named crosstabs with literal fixed headings

Named local crosstabs can now supply read-only query sources when their IN clause
declares literal Text or signed integral headings. For example, a saved query
with `PIVOT Quarter IN (3,2,4)` exposes its row headings/totals followed by columns
`3`, `2`, `4` in that order, even when an intersection is missing. Ordinary SELECT
can project/filter/join those columns, use the source in nested reads or UNION,
or feed another crosstab. Explicit top-level crosstab behavior remains available.

Microsoft's [TRANSFORM statement](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql)
defines fixed IN values as result headings. Its
[crosstab guide](https://support.microsoft.com/en-us/access/make-summary-data-easier-to-read-by-using-a-crosstab-query)
notes that an absent fixed value still creates an empty column. Those references
motivate a declared schema; they do not qualify TableCraft's broader native query
execution or edge cases.

## Metadata-only schema

The binder derives names from the already-bound row projection and literal IN
values. Text names are nonempty/control-free and at most 255 UTF-8 bytes; integers
use the same checked decimal spelling as execution, including signed i64 minimum.
Unary plus/minus on integer literals is supported without running an evaluator.
Row headings/totals and pivot names share the 256-column ceiling, plus ordinary
binding node/byte/depth limits. Heading collisions with earlier names use the
existing ASCII-insensitive name policy and fail. No scalar types are invented
for aggregate results; existing derived-value evaluation preserves actual carriers.

The binder calls no row-scan callback, clock, user function or heading-discovery
execution. Dynamic headings and nonliteral computed/parameter-dependent IN values
remain unsupported as named schemas. They still retain their existing top-level
execution profile. Inline `(TRANSFORM ...)` derived syntax is not introduced;
ordinary `(SELECT ... FROM NamedCrosstab)` derived reads work.

Punctuated generated columns can be referenced as literal bracketed dot members,
such as `c.[Sales.Q1]` or `c.[Sales!Q2]`. Object/alias declarations, qualifiers,
callable names and bang-member paths retain their existing guards. This extends
field-reference syntax, not native object naming or application-control authority.
Embedded bracket delimiters still await their own grammar/qualification; wildcard
retrieval does not reinterpret heading text as SQL.

## Composition and execution

Named definitions keep independent parameter scopes, compatible-value merging,
dependency-cycle detection and shared binding/execution budgets. The plan captures
the selected definition; a changed definition needs preparation again under the
existing Source contract. Native saved definitions use the current inert metadata
cache. Inspection, inventory and `query saved` do not activate a dependency.

The private read plan now contains a crosstab node. Its binder already prunes
underlying source fields with pivot dependencies retained; enclosing pruning does
not repeat that transformation or discard inner output cells. All selected
crosstab row totals/cells, HAVING, sorting and TOP execute before outer consumers.
An outer projection, filter or TOP cannot suppress an error in an unreturned inner
cell or excluded inner group. No outer predicate becomes a native-source hint.
The runtime verifies resulting column names against the bound schema before
returning the named result, and existing buffered publication remains atomic.

Heading validation and integer spelling now have one shared implementation for
binding and execution, with charged fallible name allocations. Runtime collation
is still explicit for Text heading comparison; metadata binding does not guess
native locale equality. Cancellation, source failures, byte/row/work budgets and
schema checks remain effective. No package dependency, adapter ABI, storage
format, native writer, GUI or runtime provider was added.

## Evidence and remaining scope

Seven feature tests cover no-scan preparation, pivot-only fields after repeated
outer pruning, projections/joins/UNION/derived/correlated reads and another crosstab,
literal heading punctuation, parameter scopes/cycles, 256/257-column boundaries,
signed headings, dynamic/computed refusals, TOP/ties, hidden-cell errors, budgets
and every measured cancellation checkpoint. A parser regression covers literal
qualified fields while preserving object/function/bang guards. Native authored
layouts exercise fixed named reads across Jet 4 / ACE 2007/2016/2019. CLI tests
cover fixed projections, repeated aliases/joins, inner TOP and buffered cell errors.

The original [SavedNamedCrosstabFixture.java](testing/SavedNamedCrosstabFixture.java)
adds nineteen inert definitions to our fourteen-query crosstab baseline. Four
fresh local files pass 380 independent getter/rendering/direct/saved/named/refusal
controls through [compare_named_crosstabs.py](testing/compare_named_crosstabs.py).
Explicit expected grids control parameterized dependencies, punctuated headings,
correlation, joins, UNION, nested reads, another crosstab, inner TOP/ties and
cycle/later-cell refusals. Raw flags supply stored TOP omitted by the reference
renderer. [Fixture records](FIXTURES.md#named-fixed-crosstab-inputs) retain provenance
and hashes. All source bytes stay unchanged; the reference never executes SQL.

```sh
python3 docs/testing/compare_named_crosstabs.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-named-crosstabs --generate
```

Native Windows Access naming, parameter/aggregate promotion, collation, execution/
error ordering, dependency admissibility and save/reopen qualification remain
unrun. Dynamic named schemas, computed fixed heading schemas, inline TRANSFORM
sources, wider heading carriers and full Access semantics remain backend work.

Fixed named crosstabs also use the shared explicit
[collection membership criteria](CROSSTABS.md#collection-membership-criteria).
Literal headings bind without scanning regardless of parent membership results;
outer consumers retain complete inner cell/decoder evaluation. This does not add
dynamic named schemas or native Access qualification.
