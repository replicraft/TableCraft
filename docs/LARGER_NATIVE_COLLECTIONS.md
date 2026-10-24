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

Buffered `rows --collections` and `--multivalues` now accept the explicit native
controls described below. Their defaults, inspection and legacy engine entry
points retain the default complete-collection limits. Scalar `rows --stream`
does not become a collection stream. SQL history access remains unfinished.

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
The additional attachment/history corpus below qualifies its explicit larger
read profile. Million-row collections, broader native Access SQL and coherent
capture during live Access edits remain unqualified.

## Reusable attachment decoding sessions

`AttachmentDecoder::new(budget, cancelled)` borrows one cumulative pool and
cancellation policy without allocating a codec. `decode` handles one independent
wrapper; `StoredAttachment::decode_with` also retains NULL payloads as None.
The first compressed file allocates/charges the existing 128 KiB logical codec
workspace. Later compressed files reset and reuse it. Uncompressed/NULL-only
sessions do not allocate a codec. Consumers meter additional owned copies and
formatting through `budget_mut()` against the original pool; they must not replace
that pool. Each file still charges its body/type/copy allocations and decode work.

The selected flate2 1.1.10/miniz_oxide 0.9.1 Rust backend's
[reset operation](https://docs.rs/flate2/1.1.10/flate2/struct.Decompress.html#method.reset)
starts a fresh stream using existing state. Its locked implementation was checked
locally for counter/dictionary reset without reallocation. Recheck this assumption
when changing backend/features or versions. No upstream implementation is copied.
The logical workspace allowance remains conservative, not an allocator quota.

Checksums, exact consumed/produced lengths, output sentinel, header/type/file caps
and bounded cancellation blocks remain unchanged. A failed decode publishes no
file and never retries; the caller can explicitly decode a later independent
stream, which resets the workspace even after corruption/cancellation. Sessions
retain no inputs or decoded files. The existing free `decode_attachment` and
`StoredAttachment::decode` APIs use a fresh session per call. Attachment query
caches now use one session per complete decoded column, and still publish only
after all members succeed. This introduces no new provider or write capability.

Explicit [decoded-file allowances](LARGER_DECODED_ATTACHMENTS.md) now support a
validated per-file ceiling up to 16 MiB, retaining the 1 MiB default and separate
encoded/header limits. File-policy changes clear decoded caches without budget
refunds, while complete stored snapshots remain reusable. Metadata-only member
inventories rebuild when decoding is first enabled; unrequested payloads stay inert.

Four native controls cover 512 independent compressed files under a 192 KiB
pool, lazy workspace allocation, resets after checksum/truncation/trailing/plain
streams and every observed reused-stream cancellation checkpoint with explicit
subsequent recovery. The same finite pool rejects repeated fresh workspaces;
limits are not raised or refunded to admit the batch.

## Larger attachments and stored histories

Original [LargeStoredObjectsFixture.java](testing/LargeStoredObjectsFixture.java)
creates 20,000 attachments and 20,000 versions, with 201 parents per owning table
including an empty parent. Version values include NULLs, duplicate Unicode text,
tied/nonmonotonic timestamps and NULL dates. Attachments have varied flags and
three exact data bytes. ACE 16 and ACE 2007 inputs persist under ignored `.cache/`.

The [runner](testing/compare_large_stored_objects.py) passes 33 controls against
independent Python models, read-only Jackcess getters and exact tagged readers.
Every encoded metadata field, decoded byte and stored history identity/text/DATE
bit agrees. CLI controls cover larger metadata/decoded queries, default refusals,
scalar pruning and the existing separate hidden-membership/displayed-empty rules.
Latest visible Memo values remain NULL; histories do not replace them. Ordinary
history SQL and native FileData SQL representation remain unqualified.

The engineering `attachment_data --rows` and `table_data --collection-rows`
options select explicit cumulative native row and parent/child retention limits.
The latter requires a history/multivalue mode before opening a file. Defaults
remain 16,384; the explicit maximum is 1,000,000. These tagged tools are separate
from the public CLI output protocol and do not widen ordinary `rows` inspection.
[Hashes and reproduction](FIXTURES.md#larger-attachment-and-stored-history-inputs)
record unchanged originals and the reviewed external artifact.

## Buffered larger CLI pages

`CollectionPageRequest` combines a validated `TablePageRequest`,
`TableSnapshotLimits` and an explicit cached-result opt-in. ACE's
`read_stored_collection_table_page_with_request` and
`read_multivalue_table_page_with_request` validate complete parent-key and hidden
child tables before returning the requested visible range, including when the
range is beyond EOF. The old entry points still supply default limits. Browser
methods with the same request suffix retain the browser's original cumulative
controls, with no refill between pages or tables. No formula is evaluated.

Buffered `rows --collections` and `rows --multivalues` now accept `--native-bytes`,
`--native-rows` and `--native-steps`, using the same ranges/defaults as scalar
streaming. Those flags require one of the explicit collection/stream modes before
I/O; they are not silently ignored by ordinary scalar browsing. The row allowance
also supplies the complete parent/child snapshot ceiling. Metadata, repeated
views and failed work consume that same pool. A visible `--limit 1` does not skip
validation of the rest of a collection.

```sh
tablecraft rows example.accdb Histories --collections --limit 1 \
  --native-bytes 268435456 --native-rows 1000000 --native-steps 100000000
```

Attachments remain encoded/inert metadata in this command. Stored versions keep
exact identities, Unicode/NULL text, raw DATE bits and stored order; they do not
replace the visible Memo or establish SQL history semantics. Cached scalar values
still require `--stored-calculations` and retain their stale-result labels.
Templates and unsupported selected types remain refused.

The 1 MiB output ceiling remains independent. Large collection pages can exceed
it even with sufficient native resources; lower `--limit` to display fewer parent
rows. All normal results remain buffered. `--all` applies one range per table and
uses one cumulative budget; a later invalid collection or formatting/output-cap
failure publishes no earlier section. This adds no collection stream or automatic
retry. Shared OS locks retain their separate offline-source guarantee.

The original [large-page runner](testing/compare_large_collection_pages.py) passes
88 controls over the four unchanged registered multivalue/stored-object inputs.
Read-only Java getters supply every heading/cell, including generated history
headings. Controls cover both multivalue modes, offsets, final/empty pages,
default/lower caps, later 101-value failure in `--all`, successful mixed-table
sections and output refusal. Four new native tests cover range/cap separation,
orphans outside the displayed range, typed attachment/history pages and every
observed successful cancellation checkpoint. A browser test proves budgets stay
cumulative across pages; a CLI test checks rendered equivalence and pre-I/O range
validation. These do not qualify live-file coherence or native Access SQL.
