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
scalar metadata. Original source order and eager predicate/projection
kernels are retained. Scalar conversion, arithmetic, text and inspection services
use the same explicit execution policies as buffered queries.

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

Live retained memory is bounded to current-row work, while allocation charges
remain cumulative after buffers are dropped. Query byte/row/work limits never
refill. Raising limits is explicit and does not change provider or output budgets.
Transport framing, batching, output quotas and stdout behavior belong to hosts;
this library does not write a header or open files/providers.

## Verification and remaining work

Twenty-three progressive service tests and nine independent UNION promotion/
boundary tests pass. They cover buffered parity, exact parameters, field policy
routing, strict publication, eager failures, unsupported shapes before scans,
sticky sink/query failures and checked EOF. UNION controls retain written arm
order, per-arm TOP/EOF, default/declared result descriptors, conversion failures,
late schema mutation, unused comparison policies and shared cumulative limits.
Each existing query quota has a one-row successful control followed by cumulative
exhaustion; every observed cancellation checkpoint is exercised. TOP controls
also cover omitted raw String publication parity and failures after the last
emitted row. Scoped strict query Clippy passes.

At the initial shared-runtime extraction, seventy-six query unit, domain,
named-crosstab, external-named and static declaration tests passed, along with
minimal query checking. The current scoped service checks are engineering
evidence, not native Access execution qualification or complete read/query
acceptance.
