# Progressive scalar query execution

`PreparedQuery::execute_progressive` is an explicit borrowed-row query service.
It visits one strictly published scalar output row at a time, returning checked
scanned/emitted counts only after successful source EOF. Ordinary execution
remains buffered. Working consumers are `MemorySource` and the explicit cursor-backed
[`NativeSource`](NATIVE_PROGRESSIVE_QUERIES.md). The
[explicit CLI host](STREAMING_QUERIES.md) supplies TSV batching and publication;
the API itself never writes stdout.

## Admitted query shape

The service accepts a scalar table input or a static named/derived SELECT input chain, including already authorized external namespaces. A flat
UNION ALL combines such SELECT arms, with optional aliases, declared parameters, row-local WHERE, scalar
projections and positive unordered TOP counts. Wildcards use the ordinary qualified
scalar metadata. Global COUNT(*), COUNT(expr), SUM(expr), AVG(expr), MIN(expr)
and MAX(expr) projections
over the same admitted input are also supported. Proven Boolean/integral/Currency/Decimal/NULL
GROUP BY tuples use the same aggregates and retained final key values. Proven
Text tuples with those key families use a separate complete-cohort partition
service. Aggregate/grouped HAVING filters use the deferred projection service described
below. Original source order and eager
predicate/projection kernels are retained. Scalar conversion, arithmetic, text and inspection services
use the same explicit execution policies as buffered queries. Prior explicitly
named row-local projections can feed later projections through the ordinary
[projection alias](PROJECTION_ALIASES.md) references. Each definition executes once
at its written position, and consumers preserve its retained carrier/declaration
and text policy. Alias references do not bypass the complete arm admission check
or introduce new WHERE/aggregate/subquery scopes.

Admission rejects ordering, TOP PERCENT, DISTINCT/active DISTINCTROW, other
grouping key families, nonaggregate HAVING,
duplicate-eliminating UNION/crosstabs, joins/products, inner set stages,
subqueries, domains, collections, Eval and volatile
clock/random calls. Rejection
occurs before scanning or visiting output, including false filters and empty
sources. This is a progressive execution profile, not a change to parser or
ordinary query availability. Single-table DISTINCTROW remains inactive and
preserves duplicate rows, as in buffered execution. Metadata-only `output_names()`
supplies ordinary read-plan names; top-level crosstabs return None. Output type/policy metadata
retains its existing conservative contracts.

## Global COUNT

A global aggregate SELECT whose only aggregate nodes are `Count(*)` or
`Count(expr)` can scan an admitted scalar input progressively. Optional scalar WHERE
predicates use the normal eager evaluator. The query retains one checked Int64
counter per written aggregate occurrence, not a complete input snapshot. Each
native row still passes the source's selected-read and EOF validation. For a direct table, pure
`Count(*)` needs no scalar columns; other counts request fields needed by WHERE
and their operands. A pipeline additionally retains every selected inner
projection, even if the outer count does not inspect its value.

```sql
SELECT Count(*) AS Population, Count(Label) AS PresentLabels,
       PresentLabels + CLngLng(1) AS NextValue
FROM Items
WHERE Active = True;
```

`Count(expr)` uses the same nullness/state admission as buffered Count. NULL is
excluded; empty Text, retained odd-byte/unpaired-surrogate Strings and Error data
are non-NULL. Empty and Missing retain their existing refusal rather than becoming
NULL. This does not add stored/parameter states or SQL constructors. An operand
can use the already admitted row-local scalar profile, but not nested aggregates,
subqueries, domain/Eval calls or volatile clock/random calls. Literal Text remains
a value: `Count('MissingField')` does not resolve a field or parse executable text.

Each qualifying row evaluates every Count operand in written projection/tree
order, once for each syntactic occurrence. Repeated identical counts are not
deduplicated; prior output aliases still reuse their defining count result.
Operand failures occur during scanning, before the next qualifying row. This is
an explicit progressive error-order contract: buffered aggregation evaluates one
aggregate across its whole materialized group before the next projection's
aggregate, and can therefore report another failure first. Final scalar projection
expressions still execute only after successful EOF, including on empty or wholly
filtered inputs. For example, `Count(1/0)` is zero on no matching rows, whereas
`1/Count(*)` fails on the resulting zero count.

Completed results are borrowed only by the final projection's environment. Full
immutable aggregate-node identity selects the corresponding occurrence even when
a scalar function requests its arguments in another order. No raw pointers or
mutable Runtime count state are retained. The existing evaluator computes final
scalar expressions, aliases and strict publication. Every count keeps its existing
Int64 result carrier and declaration metadata. Positive TOP retains the one
aggregate result row; SQL TOP 0 remains refused during preparation.

Count arms and ordinary row arms can share a flat UNION ALL. Written arm order,
whole-UNION active-profile promotion, all-arm admission/schema preflight and
per-arm schema rechecking remain unchanged. A count arm publishes nothing before
its own EOF; a later count/source/projection failure can still follow output from
an earlier arm in explicit progressive mode. The first failure remains sticky,
including when a broken source swallows a rejected input callback. Final count
sink callbacks occur after the source cursor has returned, and a failed sink
prevents later arms from starting.

Byte/row/work allowances remain cumulative, including operand evaluation and
per-row copies, so bounded live memory does not grant an unlimited scan. Slot
registration, counter initialization, per-row occurrences and final identity
lookups are metered and cancellable. Counter storage is prepaid, fallible and
bounded by the existing binding-node ceiling. No complete cache is published.
SUM/AVG and MIN/MAX can share this execution service as described below. HAVING
uses its separate suppression path; other aggregates and wider sources remain
outside this progressive slice. Ordinary
buffered aggregate availability is unchanged. Microsoft's
[Access SQL Count contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
establishes field/calculation operands and the asterisk's inclusion of records
with NULL fields. Our Int64 carrier, publication/error timing and resource ceilings
remain explicit engineering contracts, not native Access execution qualification.

## Global SUM and AVG

Global `Sum(expr)` and `Avg(expr)` use the existing buffered numeric aggregate
kernels over the same admitted progressive inputs. They can share a SELECT
with Count occurrences, scalar expressions and prior output aliases, and can
share a flat UNION ALL with other admitted aggregate or row arms. No complete
input group is retained. Arguments remain ordinary row-local expressions; nested
aggregates, domains, subqueries and volatile calls remain refused before scanning.

```sql
SELECT Sum(Amount) AS Total, Avg(Amount) AS Mean, Count(Amount) AS Present,
       Total / Present AS Ratio
FROM Items
WHERE Active = True;
```

Numeric operands follow the existing strict scalar admission before reaching
Sum/Avg. NULL is omitted; an empty or wholly NULL population returns NULL. This
is distinct from Count's nullness observation: retained malformed Strings and
Error values cannot silently become numeric or NULL. The kernels retain their
existing Boolean/integral/Currency admission, checked prefix overflow, exact
floating accumulation, bounded wide Decimal totals, Decimal rounding and Date
handling. In particular, Avg uses admitted Date serials; this does not grant Sum
new Date behavior. The default and declared-arithmetic result descriptors and
UNION conversions remain unchanged.

Every written occurrence has its own state and non-NULL counter. WHERE runs
first, then every aggregate argument runs in written projection/tree order on
each matching row. Repeated occurrences are not deduplicated. After checked EOF,
the service finalizes states in occurrence order, then evaluates surrounding
scalar projections and aliases using borrowed immutable node identities. An
empty population skips argument evaluation but still executes final scalar
expressions. No aggregate output is published before its source returns EOF.
A later argument failure can precede an earlier occurrence's finalization error;
ordinary buffered execution instead completes one aggregate across its group
before evaluating the next. These are explicit progressive phases, not native
Access error-order claims.

State vectors, result slots, operand copies, exact numeric scratch and identity
lookup work remain charged to one cumulative query allowance. Each live floating
state acquires its own metered scratch. Buffered aggregates can release and reuse
one scratch sequentially, whereas concurrent progressive Sum/Avg occurrences may
require multiple live scratch buffers and therefore fail a smaller byte allowance
that buffered execution satisfies. No quota is reset or refunded, and no failed
prefix is cached as a complete result. Cancellation and source failures prevent
final aggregate publication; a prior UNION arm's published output remains visible
only in this explicit progressive mode.

Microsoft's [Access SQL Sum contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/sum-function-microsoft-access-sql)
and [Avg contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/avg-function-microsoft-access-sql)
establish numeric field/calculation operands, NULL omission and exclusion of
nested SQL aggregate arguments. Retained carriers, exact accumulation, resource
ceilings and streaming phases are TableCraft engineering contracts. These checks
do not qualify native ACE execution or optimizer behavior.

## Global MIN and MAX

Global `Min(expr)` and `Max(expr)` use the same strict retained-cell extrema
service as buffered queries. They can appear alongside Count/Sum/Avg, feed prior
projection aliases and participate in admitted flat UNION ALL arms. Every
qualifying row evaluates each syntactic aggregate operand once in written order.
The source must complete checked EOF before an aggregate result is published.

```sql
SELECT Min(Amount) AS Low, Max(Amount) AS High, High-Low AS Spread,
       Count(*) AS Population
FROM Items WHERE Amount IS NOT NULL;
```

NULL operands are excluded; empty/all-NULL populations return NULL. Equal values
keep their first encounter representative, carrier and exact String bytes.
Numeric/date/Boolean and other non-String populations use the existing strict
per-comparison rules, including mixed binary widening and numeric-family refusals.
Standard Error codes remain expression data; opaque singleton codes and
Error/non-NULL scalar mixtures retain their existing refusals. No early exit at
a possible minimum/maximum bound skips later operands or errors.

The final results remain retained cells. Raw-aware outer expressions such as
`LenB(Min(ChrW(55296)))` can consume a singleton without unused comparison or
Unicode publication; directly publishing its unpaired result still refuses.
Similarly, explicit conversion can consume an Error extremum while the scalar
visitor itself cannot publish Error data. The result does not inherit the selected
field's Scalar declaration: aggregate provenance remains unknown under the
existing [declared profile](DECLARED_ARITHMETIC.md).

With no collation resolver, String selection uses the existing fixed global
comparison path and retains one selected cell. With a resolver, progressive
execution conservatively retains every non-NULL String candidate per occurrence
until EOF, including direct fields. It does not assume that static policy identity
proves future runtime provenance. All actual policies and completion markers,
including NULL contributors, merge into the final cohort; both metadata components
apply to every retained candidate before comparison. No provider activates during
input for these deferred comparisons, and singleton/all-NULL populations perform
no unused comparison. Unknown/conflicting required policies still fail when used.

This supports the complete existing
[retained String and Error extrema contract](EXPRESSION_AGGREGATES.md), rather
than a numeric-only subset. Retained candidate headers/payloads consume the
caller's cumulative query allowance, separately for repeated aggregate occurrences.
Only operand cells are retained, not complete source rows or a native snapshot.
Resolver-dependent String extrema can therefore require memory proportional to
their population. Existing numeric and Count state/result widths are also charged
at their actual sizes; no quota is reset or refunded.

The shared buffered service retains its existing stable-policy proof, selection,
comparison and allocation timing. Progressive input interleaves aggregate operand
work by row. Non-String/global-comparer selection errors can consequently precede
later input errors; resolver-dependent String comparison happens after checked
EOF, even for a stable field. A source/EOF failure prevents that final comparison.
Finalized states are available before final projection, including kernels that
request aggregate operands out of written order. Projection/conversion errors and
sink failures retain the ordinary sticky stream boundary; no following UNION arm
starts after failure. These explicit phases do not qualify native error priority.

## Progressive grouped aggregates

A grouped SELECT over an admitted input can use Boolean, Byte, Int16, Int32, Int64, Currency, Decimal and NULL
key columns, including tuples and pure row-local computed expressions. Every
active result descriptor must prove one of those carriers. Binding captures both
existing arithmetic profiles using the ordinary metadata inference; execution
selects its own matching profile without rescanning, rebinding or guessing from
the first value. Other key carriers and Dynamic descriptors refuse before any
cursor, including empty/false-filtered later UNION arms. Actual keys are checked
against the admitted profile before directory comparison; an unexpected carrier
fails rather than inventing a new policy cohort or reinterpreting its value.

```sql
SELECT Id Mod 17 AS Bucket, Count(*) AS Population,
       Sum(Score) AS Total, Avg(Score) AS Mean,
       Min(Label) AS FirstLabel, Max(Label) AS LastLabel
FROM Items
GROUP BY Id Mod 17;
```

The first encountered key tuple represents each group. NULL keys group under
the existing SQL key equality contract. A bounded exact-key directory narrows
candidates; the existing relational comparator remains authoritative. Selected
source rows and key temporaries are evaluated and discarded without HAVING. Each group retains
its key cells and one existing accumulator for every written aggregate occurrence,
without a representative source row, complete input relation or result cache.
Currency fingerprints use their exact scaled integer. Decimal fingerprints
remove fractional trailing zeros from a local coefficient/scale view, including
zero normalization; retained keys keep their original coefficient/scale and
declarations. Full relational equality remains authoritative after fingerprint
matches. See [typed key proof and fallback](EXACT_QUERY_KEYS.md).
Group-only projections without aggregates use the same key service. Empty or
completely filtered input yields zero grouped rows.

WHERE runs before the key expressions. Qualified keys run once in written GROUP
BY order, followed by each aggregate operand in written projection-tree order.
Repeated aggregate syntax remains separate work. Count, Sum, Avg, Min and Max
keep their full current argument and state contracts, including raw String/Error
Count operands and resolver-dependent String extrema candidates. String Min/Max
can retain a candidate per input value under the query budget; grouped execution
does not promise constant memory. Concurrent numeric scratch, keys, candidate
payloads, directories, state/results vectors and final outputs are charged at
their actual widths. Discarding input rows does not refund cumulative allowances.

Key cells retain first values and declarations, and merge incoming policy and
completion within each group. With a field-policy resolver, a compact cohort also
accumulates across the entire qualified key column, including numeric and NULL
cells. Checked EOF precedes applying that final cohort to all retained keys,
matching the conservative buffered grouping contract. This exact membership
proof supplies no Text equality rule; Text grouping uses the separate service
below. No native mixed-policy precedence is inferred.

Final projections explicitly borrow completed key cells. Metered structural
identity and charged cell copies preserve their value, declaration, policy and
completion through scalar/string/byte builtins and prior aliases. Keys are not
re-evaluated against an invented row. The substitution is inactive for ordinary
buffered evaluation, per-row keys and aggregate operands. Aggregate results keep
their existing full-node occurrence identities and declaration boundaries.

No group output escapes before successful source EOF. Groups then finish their
states and final projections in first-encounter order. An admitted TOP count caps
publication only: omitted groups still finish and evaluate eager final errors,
while skipping enclosing UNION conversion and strict output publication. A late
finalization/projection error may follow an earlier accepted group; input-operand,
source or EOF failures prevent every output from that arm. Sink failure stops
later groups and arms. These explicit progressive phases are separate from
buffered load/group/aggregate error order and native Access qualification.

## Progressive Text grouping

Terminal GROUP BY tuples with at least one statically proven Text column can
combine Text with the current Boolean/integral/Currency/Decimal/NULL families.
The active paired group descriptors supply admission; actual rows never invent
an observed-type profile. Successful retained scalars are checked against each
column's promised carrier, with NULL allowed and raw String bytes restricted to
Text. Standard expression Error data retains the existing strict-key validation
and pair-refusal path; it is not reinterpreted as a stored scalar or NULL. Date,
floating, GUID/Binary and Dynamic key profiles remain outside this slice. Static named/derived inputs, authorized external
namespaces, all five current aggregate kernels and terminal HAVING retain their
existing services. Text does not acquire a hash or a guessed stable provider.

Every qualified key expression executes once at its input position. Complete
retained keys are held until successful physical EOF. With a resolver, the final
policy/completion cohort for the entire key column is stamped on every key
before any group-membership comparison. This can change the actual partition:
a late provider that equates early distinct strings must merge their populations.
Stamping a final policy onto already formed groups would not preserve that
contract. Without a resolver, the supplied global comparer retains its existing
fallback behavior; no locale is inferred.

After EOF the existing strict tuple comparator partitions all keys in first
encounter order, preserving NULL equality, column short-circuiting, singleton
validation and raw String/provider refusal boundaries. No aggregate operand or
final result runs until the entire partition succeeds. A singleton does not
activate an otherwise unused provider. Identical byte payloads still undergo the
ordinary comparison when duplicate rows require it; they are not pre-collapsed
by representation. First spelling, raw carrier, scale in mixed fixed tuples,
declaration and merged key provenance remain retained cells.

Count(*)-only and key/scalar-only consumers retain keys and checked group counts,
with no retained source rows or row-assignment capacity. Aggregate operands in
either HAVING or SELECT require paid moved source-row cells; a post-partition
assignment sequence preserves original global input order for their replay.
No source cell is coerced or strictly published between these phases. Key cohorts
and per-group aggregate operand cohorts remain separate populations. In
particular String Min/Max still use their complete per-group operand policy and
completion markers rather than the grouping provider.

Without HAVING, aggregate states are constructed after partition and operand
rows replay in original input/written occurrence order before group finish.
With HAVING, only its states run first; selected states and operands remain
suppressed until that group passes its predicate. Count(*) result slots can use
checked group counts while preserving full syntactic occurrence identity and
Int64 carriers. Prior aliases and final group-key observers use the existing
retained key bridge, with no fabricated representative input row.

This profile has an explicit EOF/partition barrier before aggregate work. Failed
source EOF or a later key comparison therefore precedes aggregate operands and
prevents every result from the arm. Once replay starts, evaluated operands remain
eager and follow the progressive row-major/written occurrence contract, rather
than buffered aggregate-at-a-time access. An explicit comparison inside a key
expression still executes at its original input position and can use its provider
there; the barrier concerns membership comparisons and aggregate work. TOP never
cuts capture/partition or later accepted final work. First typed failures and
lexical owner restoration retain the existing stage/arm rules.

Live storage is honestly O(retained key storage), including backing buffers
outside a raw String's active range, plus optional operand rows, assignments,
group states and String extrema candidates. Count/key-only avoids
retaining wide input rows, but still retains every qualified key; many distinct
categories can require quadratic comparisons under the supplied work limit.
All capture, growth, cohort, partition and replay work is paid and cancellable.
Cumulative Bytes also charge each input projected-row/key copy even when it later
drops, so this is a live-payload and snapshot-avoidance capability under explicit
allowances, not a claim of constant memory or free scans. No prefix is cached as
a complete source or named result, and no hidden spill/fallback is introduced.
Existing no-Text exact groups and global/ordinary row plans keep their allocation
paths and phases, including the small-budget Count(*) controls.

## Terminal HAVING and deferred selected operands

Already admitted global and proven-key grouped SELECTs can filter their final
populations with HAVING. The current Count/Sum/Avg/Min/Max expression profile is
available in the predicate, including aggregates not selected for output. The
ordinary binder keeps projection aliases unavailable to HAVING: writing an output
alias does not create a new predicate scope. This terminal service also supplies
the admitted [stateful retained stages](#stateful-retained-stages), with the same
predicate suppression and group-key families.

HAVING executes after checked source EOF and complete group-key policy cohorts.
A False or Unknown predicate suppresses every selected scalar expression and
selected aggregate operation. For example, `SELECT Sum(1/0) FROM Items HAVING
False` does not evaluate the projected division. A rejected group does not
activate projection-only comparison providers, apply number formatting, or
strictly publish a raw String. WHERE and selected native-carrier validation
remain unconditional input boundaries; malformed selected source data and failed
EOF cannot be hidden by HAVING.

For global/exact-key grouping, the scan accumulates a separate registry of
HAVING aggregate occurrences. Text grouping defers these operands until its
checked EOF and full partition barrier, as described above. When
selected aggregates require operands, the operation moves complete retained input
rows into paid per-group replay vectors. Only after a group passes HAVING are its
selected aggregate states constructed, its operands replayed in input/written
occurrence order, and its states finished. Final scalar projections and prior
aliases then run normally. Identical-looking aggregate text in HAVING and SELECT
still has separate syntactic identity and evaluation work. Pure key/scalar outputs
and Count(*)-only selected aggregates need only checked group counts, with no
per-input replay rows. This shortcut preserves the ordinary Int64 Count carrier.

Replay retains original field indices, raw/Error cells, declaration provenance
and complete text-policy cells without intermediate publication or stored-schema
coercion. It can retain complete selected input widths and is bounded by the
cumulative query allowance; this is not a constant-memory profile. Capture,
capacity growth, replay, state/result allocation and identity lookup remain
metered and cancellable. No source or named-result cache is published. Queries
without HAVING retain their existing state layout and allocation path.

HAVING aggregate operands retain the explicit streaming scan phase: multiple
occurrences run row by row before final predicate evaluation, rather than the
buffered evaluator's aggregate-at-a-time access order. Resource/cancellation and
operand failures can therefore precede final scalar predicate failures. Rejected
SELECT projections nevertheless stay entirely suppressed. Accepted projection
replay retains the existing progressive operand-then-final-expression phase;
cross-row error precedence is not claimed identical to buffered or native Access
execution. Branch operands within evaluated expressions remain eager.

TOP counts HAVING-accepted groups. Later rejected groups still evaluate HAVING;
later accepted groups still replay operands and run final projections even after
the output count is reached. A later accepted-group failure may follow earlier
published output. First query/sink failure stays sticky, and a failed sink prevents
later groups or UNION arms. Empty global input still has one population to test;
empty keyed input has no groups.

## Static named and derived input pipelines

A static SELECT chain can combine row-local WHERE/projection/positive unordered
TOP layers with the admitted global aggregate, grouping and HAVING services. Each chain ends in one explicitly
progressive scalar table cursor. Root flat UNION ALL arms may use independent
chains. Input definitions and output schemas must already be static at metadata
preparation; no data/schema discovery or buffered fallback supplies a stream.

```sql
SELECT Count(*) AS Population, Sum(Score) AS Total
FROM (SELECT TOP 100 Id, Score FROM Items WHERE Active = True) Q;
```

Every selected inner projection executes in written order, including outputs
unused by an outer filter or projection. An inner `1/0` therefore still fails
under outer `WHERE False`. Prior aliases use each stage's own output cells;
they do not leak into another stage's WHERE or named parameter scope. Complete
retained cells move between stages, preserving raw UTF-16, Error data, actual
declarations, text policies and completion markers. Intermediate rows are not
stored scalar schemas and are never strictly published or coerced through
`Value`. Only the final public visitor receives published scalar values.

TOP belongs to its SELECT layer. Its counter advances before downstream filters,
so inner TOP 2 followed by an outer filter can return no rows even when later
source rows would match. Omitted inner rows skip downstream processing while
still evaluating the inner projection and all upstream work. No TOP count stops
the physical cursor or hides late selected-field/EOF failures. The outer TOP
retains its existing eager omitted-projection behavior. Repeated named uses in
separate UNION arms retain separate counters and fresh cursors under the same
cumulative allowances.

Bound named/inline plans retain their paired metadata and remapped parameter
indices. Each stage evaluates under its captured lexical owner, independently
of the physical leaf's namespace. Success, query errors and typed sink errors
restore caller ownership; terminal accumulator construction, input and finish
use the same discipline. Named syntax remains the prepared snapshot supplied
through `Source::query`; changing it requires preparing again. There is no new
runtime named-definition reload or revision API.

Explicit external bindings still resolve only through the operation root's
metadata-only registry. Captured immutable binding keys are re-resolved at
execution. Provider identity is pinned within that operation and rechecked
outside each selected cursor. Every actual table leaf schema is preflighted before the first arm, then checked again
before its cursor. No registry borrow or schema/definition lookup remains held
across an exclusive callback. Stored tokens never authorize paths or providers.
Source-owned progressive capability/read failures retain their existing stream
error behavior; the query layer never silently materializes an unsupported source.

### Stateful retained stages

Inner SELECT stages support the same global Count/Sum/Avg/Min/Max, exact and
Text grouping, and HAVING services as the terminal SELECT. A stage receives
retained cells and finishes through a borrowed receiver; it does not create a
Source, snapshot, queue of completed output or named cache. For example:

```sql
SELECT Count(*) AS Categories, Sum(Population) AS Total
FROM (SELECT Label, Count(*) AS Population
      FROM Items GROUP BY Label HAVING Count(*) > 1) Q;
```

Physical source EOF starts a sequence of logical stage completions. A finishing
inner stage can deliver results to later stages, but those downstream aggregate
or grouped states finish only after the entire upstream stage succeeds. This
includes accepted upstream projections omitted by TOP. A late inner error can
therefore follow an outer row prefix while preventing an outer Count, Sum,
grouping or HAVING result. Empty global stages deliver one aggregate row;
empty keyed groups deliver none. A failed sink immediately stops pending inner
finalization and later UNION arms.

Intermediate output descriptors do not overwrite actual cell declarations.
In particular, Min/Max retain unknown aggregate provenance even when their
successful result has a known scalar carrier. Outer declaration-sensitive
arithmetic therefore uses the same retained declaration as buffered execution.
Only the terminal public boundary applies enclosing UNION conversion and strict
scalar publication.

Every inner UNION, crosstab, sort, TOP PERCENT and active duplicate elimination
still refuses during full-tree preflight, even in a later empty arm. Inner UNION
requires its own active conversions and complete policy cohorts before outer
consumers; merely flattening its arms would not preserve the current contracts.

Pipeline evaluation interleaves layer work per row rather than materializing a
whole inner read before evaluating the outer layer. Consequently a later upstream
error can follow earlier accepted output, and cross-row error precedence need not
match buffered execution. EOF/input failures and unsuccessful upstream logical completion prevent
downstream aggregate/grouped publication. The first query or sink failure stays sticky even if a broken source
continues callbacks; no more frame/provider/visitor work follows it. No complete
named/result cache is published, even after successful EOF.

Frame capacities, row vectors, payload copies, comparisons and terminal states
remain prepaid, fallible and cumulatively metered. A direct input has zero frame
capacity and no boxed continuation/counter allocation. Source scanned counts refer
to physical leaf rows only; emitted counts refer to final accepted output, not
intermediate layer deliveries. String extrema retain their existing bounded
candidate populations; this is not an arbitrary-size or constant-memory claim.

## Progressive UNION ALL

Flat UNION ALL arms execute sequentially in their written order, with no complete
arm/result snapshot or implicit buffered fallback. Every arm must satisfy the
same admitted expression profile; source-free arms and inner UNION plans remain
outside this slice. Static named/derived chains, including the admitted stateful stages, are supported. The complete shape and every
selected schema are checked before the first progressive record callback. Each
arm's schema is checked again before starting its cursor; no schema callback is
introduced inside an exclusive cursor callback.

The enclosing UNION's active result descriptors drive the existing per-cell
result conversions before strict scalar publication. Default and explicit
declared-arithmetic profiles keep their separate descriptors and conversions.
Numeric-to-Text conversion still requires the existing number-format policy.
Projections retain each arm's own predicate/field policies; there is no cross-arm
comparison or inferred native policy precedence. The output names come from the
first arm, as in buffered UNION. Later conversion/read/evaluation failures can
follow an already published prefix in this explicit progressive mode.

Every arm reaches checked EOF before the next one starts. TOP counts apply
independently to each arm; after its count is satisfied, WHERE/projection still
execute on later rows, while omitted cells skip enclosing UNION conversion and
final publication just as they do in buffered execution. A sink/query/source
failure prevents later arms from starting. The first sink error stays typed even
if a broken provider swallows it or continues callbacks. Scanned/emitted counts,
allocation/work controls and native allowances accumulate across all arms without
resetting. No complete cache is published.

## Unordered TOP counts

Positive `TOP n` limits emitted rows to the first n qualifying source-order rows.
The service continues reading, validating and evaluating WHERE/projection through
checked EOF after reaching that count. It can still fail on a later row, native
count check, cancellation or quota; TOP is not a read limit or permission to hide
eager errors. Empty or shorter inputs emit their complete qualifying results.
Unordered duplicate values do not extend the count. TOP 0 retains the binder's
existing refusal, while PERCENT and ordered TOP remain outside this service.

Every qualifying row computes its selected expression cells before the output
count is checked. Cells discarded by TOP do not undergo final strict scalar
publication, matching buffered execution: an omitted retained raw String is not
newly forced into Unicode. Its construction and all eager expression failures
still run under the shared controls. Only rows actually sent to the visitor
consume final scalar-publication allocations.

Microsoft's [TOP predicate reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql)
permits an arbitrary qualifying set without ORDER BY. Source-order prefix choice
and complete eager validation are TableCraft's existing engineering contracts;
this change does not qualify native ACE encounter order or error precedence.

## Explicit source capability

`Source::scan_projected_progressive` visits checked projected rows. Its default
is Unsupported; it never falls back to a complete snapshot or ordinary scan.
Selected indices retain the existing sorted/unique projection contract.
MemorySource implements it over its already validated rows. NativeSource uses
a fresh sequential cursor and leaves data snapshots untouched. Other providers
must implement it explicitly before they can be used with this service.

Before scanning, execution validates parameters, captured scalar/unavailable/
policy metadata and admitted structure. It uses one Runtime and one query budget
through the complete operation. Providers own their separate read limits and
cancellation; callbacks must propagate rejection and report late read/EOF failure.
No progressive prefix is cached as a complete relation.

A provider may hold exclusive cursor access during callbacks. Consumers and
expression providers must not reenter that source. The admitted expression
profile cannot initiate nested source reads. This does not establish native
shared-file snapshot coherence or transferable cursor positions.

## Publication, errors and limits

Both filter operands and every selected expression retain ordinary eager order
within a row. Complete selected rows pass strict scalar publication before the
visitor sees them. Raw Strings can survive intermediates but must publish valid
Unicode; final expression-only states are not silently converted to stored NULL.
A rejected row does not visit the output sink.

`StreamError<E>` distinguishes query failure from the original sink error. The
first callback failure is sticky: a broken provider that swallows rejection or
attempts later callbacks cannot turn it into success or publish more rows.
Earlier accepted rows remain visible if later evaluation, native decoding,
cancellation, limits or checked EOF fails. Progressive encounter order can expose
a different competing cross-row error than buffered load-before-evaluation.

Live retained memory includes current-row work, per-stage aggregate/group
states, retained grouping keys, optional HAVING/Text replay rows and
resolver-dependent String extrema candidates. Each stateful layer pays for its
own live populations. Allocation charges remain cumulative after buffers
are dropped. Query byte/row/work limits never
refill. Raising limits is explicit and does not change provider or output budgets.
Transport framing, batching, output quotas and stdout behavior belong to hosts;
this library does not write a header or open files/providers.

## Verification and remaining work

The stateful named/derived extension passes 550 distinct query tests, including
23 new controls: 220 progressive/value/policy tests, 38 runtime units and 292
related buffered metadata, policy, named, crosstab and key regressions. Strict
query library/tests Clippy and standalone minimal-feature checking pass. The
[scoped query record](testing/results/read-query-progressive-stateful-stages-2026-10-25.json)
retains exact commands, log hashes and excluded compile/Decimal representation
probes. Logical-EOF tests cover late TOP-omitted inner errors, retained unknown
extrema declarations and downstream provider suppression. Existing direct Count
under 8 KiB passes unchanged. Native/CLI integration and original replay are
separate checkpoints.

Parent native/CLI integration adds 107 distinct passing controls, including five
new tests. The Text-group total is 632 unique tests with 27 new controls. Stored
comparison policies on the original Access example, four priority synthetic
profiles, exact late-error prefixes and raw observers pass, alongside strict
query/native/CLI linting, minimal query features and repository formatting. The
[original native/CAB replay](testing/PROGRESSIVE_TEXT_GROUPS.md) also passes
96 commands, comparing 1,272 successful rows/4,296 cells and six exact earlier
error-prefix rows. Six rich Ordinal controls use explicit 10M query work; other
limits and all SQL/independent numerical goldens remain unchanged.

The Text grouping extension passes 525 distinct query tests, including 22 new
controls: 198 progressive/exact/value/policy tests, 35 runtime units and 292
related buffered key/raw/policy, named, crosstab, UNION and metadata regressions.
Strict query library/tests Clippy and the standalone minimal-feature check pass.
The [scoped record](testing/results/read-query-progressive-text-groups-2026-10-25.json)
retains exact commands, log hashes, the separately committed ampersand metadata
prerequisite and the new-test raw-construction correction established first by
buffered execution. Private probes establish zero Count/key-only retained source
rows/assignment capacity even after selected wide inner projections, and validate
actual carrier versus retained Error boundaries. Native/CLI integration and
original replay remain separate checkpoints.

Parent integration adds 153 distinct controls, including four new CLI tests and
one independent actual-policy conflict test. The total is 554 unique tests with
23 new controls; the repeated 51-test query target is counted once. Strict
native/CLI/policy-target linting and repository formatting also pass. A separate
[original native/CAB replay](testing/PROGRESSIVE_HAVING.md) passes 96 commands,
comparing 336 successful rows/696 cells and six exact earlier error-prefix rows.

The terminal HAVING extension passes 401 distinct query tests, including eighteen
new controls: 148 progressive/value/policy tests, 28 runtime units and 225 related
buffered grouping/HAVING, text, crosstab, named and alias regressions. Strict query
library/tests Clippy and standalone minimal-feature checking pass. The
[scoped record](testing/results/read-query-progressive-having-2026-10-25.json)
records exact commands and log hashes, the initial compile integration correction,
and obsolete negative-control updates. All executed tests passed on their first
run. Native/CLI integration and original replay remain separate checkpoints.

The static named/derived pipeline extension passes 404 distinct query tests,
including 28 new controls: 130 progressive/value/policy tests, 28 runtime units
and 246 related metadata/named/alias/UNION/text regressions. Strict query
library/tests Clippy and standalone minimal-feature checking pass. The
[scoped record](testing/results/read-query-progressive-derived-2026-10-25.json)
retains exact commands, log hashes, excluded setup probes and the prepare-only
depth diagnostic. Existing generated Count under 8 KiB passes unchanged. Native
and CLI integration adds 134 passing controls, including twelve new facade/host
checks. The complete scoped total is 538 distinct tests with forty new controls;
strict native/CLI linting also passes. The [original native/CAB replay](testing/PROGRESSIVE_DERIVED.md)
passes all 96 controls, comparing 420 rows and 1,410 cells against independent
formulas. Actual Access qualification remains separate.

The grouped extension passes 385 distinct query tests, including 25 new tests:
104 progressive/value/policy controls, 26 runtime units and 255 related buffered
metadata/grouping/crosstab/named/alias/key regressions. Strict query library/tests
Clippy and standalone minimal-feature checking pass. The
[scoped record](testing/results/read-query-progressive-grouped-2026-10-25.json)
retains exact commands, log hashes and excluded new-test authoring/diagnostic
attempts. No production answer correction was required. Native/CLI integration
and original getter replay evidence are recorded separately.


The global Min/Max extension passes 269 focused query tests: 82 progressive and
aggregate controls, 147 buffered extrema/policy/group/domain regressions and 40
buffered numeric-kernel regressions. Sixteen controls are new. They include full
retained String/Error behavior, final policy/completion cohorts, EOF before
provider activation, unknown aggregate declarations, exact representative
carriers, ExtendedDate, GUID/Binary admission parity, mixed binary precision,
sticky failures and every observed cancellation checkpoint. Existing Count/Sum/Avg
limits pass unchanged, including the 8 KiB generated Count(*) control. Strict
query library/tests Clippy and the standalone minimal-feature library check pass.
The [scoped record](testing/results/read-query-progressive-extrema-2026-10-25.json)
separates these checks from native/CLI integration and native Access qualification.

The Sum/Avg extension passes 153 focused query tests: 66 progressive/count/numeric
controls, 54 composition regressions and 33 existing buffered numeric-kernel
controls. Strict query library/tests Clippy passes. Numeric controls cover exact
floating cancellation, Decimal rounding/finish failures, Date averaging, unchanged
integral/Currency prefix overflow, both arithmetic profiles, per-occurrence aliases,
empty/filtered inputs, EOF before publication, late UNION failures and separate
concurrent scratch charges. A generated 100,001-row input verifies progressive
execution without buffered fallback; cancellation checkpoints, cumulative quotas
and sticky source/sink failures are exercised. Initial failures in new test
constructors and representation goldens were corrected without runtime changes.
These are engineering checks, not native ACE aggregate qualification.

At the Count operand checkpoint, 99 query tests passed: 29 progressive service tests,
13 generated Count(*) controls, 11 Count operand controls, one shared nullness
unit and 45 existing aggregate/group/alias/UNION regressions. They cover buffered
parity, exact parameters, field policy routing, strict publication, eager failures, unsupported shapes before scans,
sticky sink/query failures and checked EOF. UNION controls retain written arm
order, per-arm TOP/EOF, default/declared result descriptors, conversion failures,
late schema mutation, unused comparison policies and shared cumulative limits.
Each existing query quota has a one-row successful control followed by cumulative
exhaustion; every observed cancellation checkpoint is exercised. TOP controls
also cover omitted raw String publication parity and failures after the last
emitted row. A Count(*) control scans 100,001 generated rows under an 8 KiB
query allocation allowance. Count operand controls cover large generated inputs,
empty/filter counts, EOF before publication, repeated and
aliased expressions, mixed count/row arm promotion, final raw/conversion errors,
sticky failures, every cancellation checkpoint and cumulative quotas. Operand
controls also pin non-written scalar argument lookup, separate duplicate
occurrences, literal Text values, raw/Error nullness and the documented difference
between row-operand and final-projection error phases. Scoped strict query Clippy
passes.

At the initial shared-runtime extraction, seventy-six query unit, domain,
named-crosstab, external-named and static declaration tests passed, along with
minimal query checking. The current scoped service checks are engineering
evidence, not native Access execution qualification or complete read/query
acceptance.

The parent integration checkpoint additionally passes 22 native cursor, 39 CLI
progressive and thirteen linked CLI tests, with strict linting and formatting.
Two new native controls exceed a snapshot ceiling of one with 1,100 input rows
and prevent every group result on late page/count failures. Three new CLI tests
cover exact profile results, Text extrema and final-error batch/UNION prefixes;
linked grouped execution also shares the existing forwarded source. The combined
checkpoint is 459 distinct tests, thirty new. Original/native Access qualification
remains separate.
