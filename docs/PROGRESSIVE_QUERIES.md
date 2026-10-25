# Progressive scalar query execution

`PreparedQuery::execute_progressive` is an explicit borrowed-row query service.
It visits one strictly published scalar output row at a time, returning checked
scanned/emitted counts only after successful source EOF. Ordinary execution
remains buffered. Working consumers are `MemorySource` and the explicit cursor-backed
[`NativeSource`](NATIVE_PROGRESSIVE_QUERIES.md). The
[explicit CLI host](STREAMING_QUERIES.md) supplies TSV batching and publication;
the API itself never writes stdout.

## Admitted query shape

The service accepts a direct root scalar table or a flat UNION ALL of those
SELECT arms, with optional aliases, declared parameters, row-local WHERE, scalar
projections and positive unordered TOP counts. Wildcards use the ordinary qualified
scalar metadata. Global COUNT(*), COUNT(expr), SUM(expr), AVG(expr), MIN(expr)
and MAX(expr) projections
over the same direct table are also supported as described below. Original source order and eager
predicate/projection kernels are retained. Scalar conversion, arithmetic, text and inspection services
use the same explicit execution policies as buffered queries. Prior explicitly
named row-local projections can feed later projections through the ordinary
[projection alias](PROJECTION_ALIASES.md) references. Each definition executes once
at its written position, and consumers preserve its retained carrier/declaration
and text policy. Alias references do not bypass the complete arm admission check
or introduce new WHERE/aggregate/subquery scopes.

Admission rejects ordering, TOP PERCENT, DISTINCT/active DISTINCTROW, grouping/HAVING,
duplicate-eliminating UNION/crosstabs, joins/products, named/derived sources,
subqueries, domains, collections, external namespaces, Eval and volatile
clock/random calls. Rejection
occurs before scanning or visiting output, including false filters and empty
sources. This is a progressive execution profile, not a change to parser or
ordinary query availability. Single-table DISTINCTROW remains inactive and
preserves duplicate rows, as in buffered execution. Metadata-only `output_names()`
supplies ordinary read-plan names; top-level crosstabs return None. Output type/policy metadata
retains its existing conservative contracts.

## Global COUNT

A global aggregate SELECT whose only aggregate nodes are `Count(*)` or
`Count(expr)` can scan a direct root table progressively. Optional scalar WHERE
predicates use the normal eager evaluator. The query retains one checked Int64
counter per written aggregate occurrence, not a complete input snapshot. Each
native row still passes the source's selected-read and EOF validation. A pure
`Count(*)` needs no scalar columns; other counts request only fields needed by
WHERE and their operands.

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
SUM/AVG and MIN/MAX can share this execution service as described below. Other aggregates,
GROUP BY/HAVING and wider sources remain outside this progressive slice; ordinary
buffered aggregate availability is unchanged. Microsoft's
[Access SQL Count contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
establishes field/calculation operands and the asterisk's inclusion of records
with NULL fields. Our Int64 carrier, publication/error timing and resource ceilings
remain explicit engineering contracts, not native Access execution qualification.

## Global SUM and AVG

Global `Sum(expr)` and `Avg(expr)` use the existing buffered numeric aggregate
kernels over the same direct-table progressive inputs. They can share a SELECT
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

## Progressive UNION ALL

Flat UNION ALL arms execute sequentially in their written order, with no complete
arm/result snapshot or implicit buffered fallback. Every arm must satisfy the
same direct-table expression profile; source-free arms, nested UNION plans and
named/derived sources remain outside this slice. The complete shape and every
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

Live retained memory consists of current-row work, the admitted per-aggregate
numeric states and any resolver-dependent String extrema candidates. Allocation
charges remain cumulative after buffers
are dropped. Query byte/row/work limits never
refill. Raising limits is explicit and does not change provider or output budgets.
Transport framing, batching, output quotas and stdout behavior belong to hosts;
this library does not write a header or open files/providers.

## Verification and remaining work

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
