# Named crosstabs with fixed headings

This document describes metadata-only preparation. Explicit query runs can also use
[execution-phase discovery](NAMED_SCHEMA_DISCOVERY.md) for qualified parameter/context/
data-dependent relational sources without putting scans into binding.

Named local crosstabs supply statically prepared read-only sources when their IN clause
declares Text, Boolean or integral headings from literals or the policy-independent constant
profile below. For example, a saved query
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

The binder derives names from the already-bound row projection and qualified fixed
IN values. Text names are nonempty/control-free and at most 255 UTF-8 bytes; integers
use the same checked decimal spelling as execution, including signed i64 minimum.
Boolean uses `True`/`False` without formatting context. Floating, fixed-point and
date headings require execution-phase discovery with explicit formatting policies;
preparation never invokes those policies or scans to infer names.
Unary signs, computed arithmetic/casts and qualified pure numeric calls reuse
existing scalar kernels without any host/source access; their profile is below.
Row headings/totals and pivot names share the 256-column ceiling, plus ordinary
binding node/byte/depth limits. Heading collisions with earlier names use the
existing ASCII-insensitive name policy and fail. No scalar types are invented
for aggregate results; existing derived-value evaluation preserves actual carriers.

The binder calls no row-scan callback, clock, user function or heading-discovery
execution. Dynamic, parameter-dependent and policy-dependent IN values remain
unsupported by metadata-only prepare. Qualified calls outside this constant profile
can now use the separate execution-phase discovery service when admitted by the
existing top-level crosstab executor. They still retain their existing top-level
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
signed headings, dynamic/context-dependent refusals, TOP/ties, hidden-cell errors, budgets
and every measured cancellation checkpoint. A parser regression covers literal
qualified fields while preserving object/function/bang guards. Native authored
layouts exercise fixed named reads across Jet 4 / ACE 2007/2016/2019. CLI tests
cover fixed projections, repeated aliases/joins, inner TOP and buffered cell errors.

The original [SavedNamedCrosstabFixture.java](../testing/SavedNamedCrosstabFixture.java)
adds nineteen inert definitions to our fourteen-query crosstab baseline. Four
fresh local files pass 380 independent getter/rendering/direct/saved/named/refusal
controls through [compare_named_crosstabs.py](../testing/compare_named_crosstabs.py).
Explicit expected grids control parameterized dependencies, punctuated headings,
correlation, joins, UNION, nested reads, another crosstab, inner TOP/ties and
cycle/later-cell refusals. Raw flags supply stored TOP omitted by the reference
renderer. [Fixture records](../testing/FIXTURES.md#named-fixed-crosstab-inputs) retain provenance
and hashes. All source bytes stay unchanged; the reference never executes SQL.

```sh
python3 docs/testing/compare_named_crosstabs.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-named-crosstabs --generate
```

Native Windows Access naming, parameter/aggregate promotion, collation, execution/
error ordering, dependency admissibility and save/reopen qualification remain
unrun. Qualified dynamic and parameter/policy-dependent relational schemas now have
[separate execution-stage evidence](NAMED_SCHEMA_DISCOVERY.md), including parameterless
runtime domain targets. Inline TRANSFORM sources, wider heading carriers and full
Access semantics remain backend work.

Fixed named crosstabs also use the shared explicit
[collection membership criteria](CROSSTABS.md#collection-membership-criteria).
Literal headings bind without scanning regardless of parent membership results;
outer consumers retain complete inner cell/decoder evaluation. This does not add
dynamic named schemas or native Access qualification.

## Policy-independent computed fixed headings

Fixed names can now come from literal scalar inputs, unary/binary operators,
numeric conversions, CVar and non-Rnd numeric math calls over constant numeric
operands. Text concatenation/addition accepts Text/NULL operands. The final value
must still be Text or an integral carrier: Boolean, NULL, floating, fixed decimal,
date and GUID headings remain refused unless an explicit supported conversion
produces an admitted carrier. This changes named schema composition; top-level
crosstab expression admission and runtime heading rules are unchanged.

For example, `IN (1+1,CLng(4/4),CInt(2^2))` declares columns 2, 1 and 4 in that
order. `IN ('Sales'&'.Q1','Sales'+'!Q2')` declares literal punctuated names that
ordinary qualified bracket references can select. Missing intersections and empty
tables retain the complete fixed column schema. Static collisions, invalid names,
arithmetic errors and binding limits fail before source scans.

The constant classifier rejects fields, parameters, subqueries, Rnd/clocks/session
calls and other providers. Text comparisons and numeric Text conversions also
remain refused here because their result or validity depends on execution policies.
A policy-dependent fixed heading may still execute as a top-level crosstab under
its existing explicit execution context. Binding does not choose that context,
consume volatile state or scan records to invent a schema. Other pure function
families remain separate extension work. Constant eager choices are supported
under the bounded profile below.

Operators/conversions/math call the existing expression kernels, with NoBindings
and no provider context. Source access is absent from the evaluator, even beyond
the closed-node classifier. Literal copies and possible Text results reserve
storage in the cumulative binding byte ceiling before allocation; recursive nodes
and operations consume binding work/depth, including a caller-owned binding budget
where supplied. Numeric operations get no Text-allocation allowance. Cancellation
is checked at every observed binding/kernel boundary. No alternate arithmetic,
formatting or query evaluator is introduced.

The original heading expressions stay in the executable plan and evaluate again
under the existing crosstab path. Runtime verifies result names against the bound
schema; metadata computation does not fold away inner cells, filters, HAVING,
sorting/TOP or later errors. Existing output text-policy provenance and parameter
scopes remain intact. There is no public provider, package dependency, native write
path or runtime host added by this slice.

Four focused composition tests cover integer/math/cast and Text names, no-scan
preparation, missing/empty results, repeated execution policies, punctuation,
collisions and contextual/volatile refusals. Three kernel-boundary tests exercise
no-source guards, parent byte/work budgets, structural ceilings/depth and every
observed transient cancellation checkpoint. The original
[ComputedCrosstabFixture.java](../testing/ComputedCrosstabFixture.java) adds nine inert
definitions to the existing original thirty-three-query Sales fixture. The
[independent runner](../testing/compare_computed_headings.py) passes 204 getter/grid/
reconstruction/inspection/direct/saved/named/refusal controls over four unchanged
priority-family files. Explicit expected grids, not the reference renderer, supply
query result expectations. Jackcess never executes stored SQL. These controls do
not qualify computed IN syntax, native expression promotion, optimizer behavior
or error precedence in Microsoft Access; actual native Access comparisons remain
pending.


Explicit [date-token preparation context](SQL_DATE_LITERALS.md#explicit-default-year-context)
can resolve omitted-year dates used by integral fixed IN headings, such as
CLng(#Mar 7#), before establishing named output schemas. Raw DATE heading names
still require runtime formatting and the existing execution staging.
The resulting constants are stored in the plan; metadata binding still scans no
rows and invokes no clock or formatting provider. Plain preparation has no implicit
year. Runtime text-date conversions remain execution dependent.

## Policy-free constant String headings

Named fixed schemas also admit the existing `Len`, `LenB`, `AscW`, `AscB`,
`Left`, `Right`, `Mid`, `LeftB`, `RightB`, `MidB`, `LTrim`, `RTrim`, `Trim`,
`StrReverse`, `ChrW` and `ChrB` kernels over closed constant operands. String
inputs must be Text, retained String bytes or NULL; constructor codes and slice
controls must be numeric, Boolean or NULL. Existing conditional controls retain
their ordinary non-Text carrier admission, including Date conversion and typed
invalid-carrier failures. Existing arity and kernel failures remain authoritative. This admission does not add
query functions or change their execution contracts; see
[byte String functions](../expressions/text/BYTE_STRING_FUNCTIONS.md),
[character functions](../expressions/text/CHARACTER_FUNCTIONS.md) and
[retained Strings](../expressions/text/RETAINED_STRINGS.md).

For example, `IN (Trim('2 '),ChrW(49),Right('x4',1))` declares names 2, 1 and 4
without scanning. Raw UTF-16 intermediates stay intact through these kernels,
Text `+`/`&`, CVar and eager conditional selection. `Len(ChrW(55296))` can
therefore declare integral heading 1 without publishing an isolated surrogate;
`ChrW(55357)&ChrW(56832)` publishes the valid final name 😀. Only the final
heading is strictly converted to Unicode. Odd-byte or unpaired-surrogate final
names remain unsupported by metadata-only preparation, with no replacement
characters or guessed encoding.

Every operand is evaluated eagerly, including unused selection branches. Literal
copies, encoding buffers, kernel outputs and final publication reserve bounded
allocation allowances before work. Kernel work receives at most the remaining
shared binding Steps allowance and is debited on success or failure; recursive
nodes retain the binding node/depth limits. Cancellation remains checked by both
binding and the original kernels. Focused tests exercise shared small budgets,
all observed cancellation checkpoints, invalid final names, eager errors and
no-scan named/derived/UNION schema composition.

Numeric/date formatting, Text-to-number or Text-to-control conversion, casing,
code pages, locale/search providers and source/parameter operands remain outside
this constant profile. No provider or source is activated during preparation.
The original expressions still execute under the normal query evaluator; this
is an engineering schema-admission improvement, not native Access qualification
of these computed pivot forms.

## Constant conditional headings

Named fixed schemas now also admit IIf, Nz, Choose and Switch when every operand
belongs to the existing policy-independent constant profile. For example,
`IN (IIf(True,2,99),Choose(1,1,99),Nz(Switch(False,99),4))` declares columns
2, 1 and 4 without scanning Sales or discovering a result. Those columns can be
referenced through named SELECT wrappers, joins and UNION dependencies using the
ordinary metadata-only planner. Execution retains the original heading expressions;
this is schema derivation, not a rewrite or a persistent result cache.

All operands are visited eagerly in written order, including unselected branches,
unused Nz replacements and later Switch pairs. The existing expression kernels
select and move the chosen Value without cloning its Text payload. Their numeric
condition/index admission, NULL selection and deferred control-conversion errors
remain authoritative. An omitted query Nz replacement yields empty Text. Existing
heading naming, length, duplicate and carrier admission still apply; selecting NULL
or an unsupported heading carrier does not invent a column name.

Text condition/index coercion remains contextual and is refused by metadata-only
preparation. Unselected fields, parameters, providers, volatile calls and Text
numeric conversions also remain contextual. These cases can use the existing
explicit execution-phase discovery service; constant selection does not suppress
them. Arithmetic failures in unselected constant branches still fail preparation,
even when the underlying source has no rows. Every branch consumes the same
cumulative binding byte/work/depth allowance and cancellation checks. No new
source, provider, dependency, storage or write contract is introduced.

The function availability and eager-selection contracts follow Microsoft's Access
[IIf](https://support.microsoft.com/en-gb/access/functions/iif-function),
[Nz](https://support.microsoft.com/en-gb/access/functions/nz-function),
[Switch](https://support.microsoft.com/en-us/access/switch-function) and
[Choose](https://support.microsoft.com/en-us/access/functions/choose-function)
query references. These contracts motivate the existing scalar kernels; they do
not establish native Access acceptance of every constant pivot expression or
TableCraft's preparation/error timing. Native Access qualification remains unrun.

Focused tests cover nested choices, named SELECT/UNION composition without scans,
Text names, empty input, unselected arithmetic failures, contextual refusal,
duplicate/NULL headings, charged unselected payloads, deferred error precedence
and every observed conditional cancellation checkpoint.

## Inline TRANSFORM is not an established Access requirement

Named crosstabs remain the supported composition boundary. Microsoft's
[TRANSFORM contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql)
places TRANSFORM at the beginning of the SQL string and documents subqueries in
crosstab WHERE predicates. Its
[subquery contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
defines nested statements as SELECT and explicitly excludes subqueries from
crosstab output expressions. Neither contract establishes a TRANSFORM statement
inside a parenthesized FROM source. That distinction is not proof that every
Access version rejects every such form, but it provides no basis to claim this
inline syntax as required native compatibility.

Any earlier roadmap wish for inline TRANSFORM therefore needs specific native
query evidence before parser/model expansion. Existing named crosstab references,
including execution-stage discovery of dynamic schemas, provide working
composition without introducing a speculative SQL extension or scanning during
metadata-only preparation.
