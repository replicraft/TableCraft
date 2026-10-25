# Progressive scalar query execution

`PreparedQuery::execute_progressive` is an explicit borrowed-row query service.
It visits one strictly published scalar output row at a time, returning checked
scanned/emitted counts only after successful source EOF. Ordinary execution
remains buffered. Working consumers are `MemorySource` and the explicit cursor-backed
[`NativeSource`](NATIVE_PROGRESSIVE_QUERIES.md). The [explicit CLI host](STREAMING_QUERIES.md) supplies TSV batching and
publication; the API itself never writes stdout.

## Admitted query shape

The service accepts one direct root scalar table, optional alias, declared
parameters, row-local WHERE and scalar projections. Wildcards use the ordinary
qualified scalar metadata. Original source order and eager predicate/projection
kernels are retained. Scalar conversion, arithmetic, text and inspection services
use the same explicit execution policies as buffered queries.

Admission rejects ordering, TOP, DISTINCT/active DISTINCTROW, grouping/HAVING,
UNION/crosstabs, joins/products, named/derived sources, subqueries, domains,
collections, external namespaces, Eval and volatile clock/random calls. Rejection
occurs before scanning or visiting output, including false filters and empty
sources. This is a progressive execution profile, not a change to parser or
ordinary query availability. Single-table DISTINCTROW remains inactive and
preserves duplicate rows, as in buffered execution. Metadata-only `output_names()` supplies ordinary
read-plan names; top-level crosstabs return None. Output type/policy metadata
retains its existing conservative contracts.

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

Thirteen independent conformance tests cover buffered parity, exact parameters,
field policy routing, strict publication, eager failures, metadata changes,
unsupported shapes before scans, sticky sink/query failures and checked EOF.
Each query quota has a one-row successful control followed by cumulative
exhaustion; every observed cancellation checkpoint is exercised.

Seventy-six existing query unit, domain, named-crosstab, external-named and static
declaration tests pass after extracting shared Runtime initialization. Scoped
strict Clippy and minimal query checking pass. This is engineering evidence,
not native Access execution qualification or complete read/query acceptance.
