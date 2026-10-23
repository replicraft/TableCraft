# Explicit larger native collections

Complete native parent/child collection reads can exceed the default 16,384-row
whole-table ceiling under explicit `TableSnapshotLimits`. Ordinary entry points
retain that default and a batch size of 256. A positive explicit ceiling can be
up to 1,000,000 rows, with a batch size up to 16,384. These are consumer controls,
not native format maxima or proof that every table of that size works.

## Engine and adapter behavior

ACE exposes `read_complex_column_links_with_limits`,
`read_multivalue_column_with_limits`, `read_attachment_column_with_limits` and
`read_version_history_column_with_limits`. Each uses one supplied ceiling for
both the complete parent-key view and the complete hidden child table. Metadata
and owned snapshots remain native. Borrowed parent schemas use the existing
incremental root traversal with feature-qualified key carriers; public ordinary
scalar projection rules are unchanged. Child tables use the existing source-bound
cursor materializer.

Usage metadata, framing, live counts, scoped key/identity uniqueness, orphan
references and owning typed schemas must all validate before publication.
Internal batches reset their physical-read quotas, while native byte/row/work
budgets and cancellation remain cumulative. Failed rows/allocations are not
refunded. This does not stream SQL or return a partial collection. A native source
must remain stable/offline throughout the operation.

The 100-value-per-parent multivalue policy remains unchanged. Attachment payload
and codec limits remain separate. History retains stored order and exact DATE
bits; this supplies no history SQL representation, sorting or latest-value rule.
Larger limits do not qualify calculated values or permit writes.

`NativeSource::new_with_snapshot_limits` now applies those limits to explicitly
selected multivalue and attachment caches as well as scalar snapshots. Binding
still reads metadata only. Caches publish only after complete link/type checks;
a failed child read can be retried explicitly under the remaining original
budget, and only a complete retry becomes reusable. Selected native indexes on
collection-reading parents remain unsupported, without sequential fallback.

## CLI scope

`query run --native-rows` now also bounds complete parent/child collection tables
when `--multivalues` or `--collections` is explicitly enabled. It does not enable
collections by itself. Cumulative row charges include metadata and repeated
native views, so a budget equal to a child count may still fail. Query execution
and output keep their independent [resource pools](LARGER_NATIVE_QUERIES.md).

```sh
tablecraft query run 'SELECT Count(Numbers.Value) FROM ManyParents' \
  --database example.accdb --multivalues --native-bytes 268435456 \
  --native-rows 1000000 --native-steps 100000000 \
  --execution-bytes 268435456 --execution-rows 1000000 --execution-steps 100000000
```

Ordinary `rows --collections`, inspection and legacy engine entry points retain
their default complete-collection limits. Scalar `rows --stream` does not become
a collection stream. The larger history reader is currently an explicit library
API; history query access remains separate unfinished work.

## Evidence and limits

The original [large collection recipe](testing/LargeCollectionFixture.java) creates
20,000 parents with 48,000 children, another 400 parents with 40,000 children,
and a separate 101-value refusal control. The [independent runner](testing/compare_large_collections.py)
checks every stored collection with Python models and read-only Jackcess getters.
Twenty-two CLI controls pass across ACE 16 and ACE 2007, covering aggregates,
empty-parent placeholders, NULLs, duplicates, explicit collection opt-ins and
separate default/lower-cap refusals. Larger allowances still reject 101 values;
scalar-only queries do not load that collection. Inputs remain unchanged.

Five new native tests cover both table caps, batch boundaries, EOF counts,
100/101 values, typed attachment/history limits and every observed successful
link-path cancellation checkpoint. An adapter fault test confirms that failed
child reads publish no cache, explicit retry completes and later scans avoid I/O.
See [registered inputs](FIXTURES.md#larger-multivalue-collection-inputs).

These checks establish the stated multivalue scale and bounded typed APIs.
Larger attachment/history corpus qualification, million-row collections, native
Access SQL and coherent capture during live Access edits remain unqualified.
