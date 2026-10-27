# Explicit column long-value storage checks

`check_table_long_values(source, scope, table, budget, cancelled)` performs a
bounded read-only storage check in the default engine build. It returns
`TableLongValueCheck`: the native schema, complete live-root count and explicit
per-column coverage. Ordinary Memo/OLE reads retain their existing behavior;
this separate operation grants no write, constraint, macro or native Access
qualification.

## Qualified profile and visible limitations

The initial checked profile is ordinary unencoded ACE 16, subversion zero,
variable Memo/OLE fields with native flag 2, extended flags 0 and zero declared
length. These metadata facts identify the observed read-check profile, not write
permission. The [60 public UPDATE observations](../../testing/LONG_VALUE_UPDATE_OBSERVATIONS.md)
provide original declaration, descriptor, allocation and orphan evidence.

The checker reads the actual trailing column-map list after validated native
index names. Each qualified ten-byte little-endian record contains a logical
column number and packed owned/candidate map references; `FFFF` terminates the
list. A borrowed zerocopy view owns this exact record shape, with independent
unaligned golden tests. Existing table-definition continuation parsing and index
metadata cursor logic remain authoritative.

Inline column maps receive complete checks. Indirect column maps remain
`IndirectMaps`, despite structural support for their common bitmap grammar;
ordinary table-owned indirect coverage does not qualify column-LVAL allocation.
Absent pairs, unknown suffixes, declarations without a matching visible field,
unsupported field flags and unsupported formats remain visible as separate
statuses. Historical-declaration status identifies an unmatched native column
number; it does not prove that a particular former field existed.

Unsupported columns are not decoded or fabricated as NULL. Kind/allocation
counts are populated only for `Checked` columns. Known duplicate active
column declarations, malformed qualified references, broken live chains and
resource/cancellation failures return errors without publishing a prefix report.

## Checks and counts

For each checked column, the service verifies map-container provenance, global
allocation, owned LVAL page signatures, native row directories/counts and exact
free-word geometry. Candidate hints must be a subset of ownership. They are not
a complete free-space inventory or a fixed-byte threshold.
Reserved descriptor/page bytes remain opaque; this check invents no checksum or
authority to rewrite their unknown semantics.

A complete ordinary logical-root visit validates row framing and forwarding and
matches the stored live count. Current NULL, inline, single-row and chained-kind
definitions use the existing long-value decoder with optional paid trace
capture. External references must lie in the column-owned set. Memo also passes
its existing bounded text decoder; OLE bytes remain uninterpreted. Normal reads
without tracing retain their prior decoder behavior and limits.

Logical-root enumeration skips deleted/hidden slots from their raw flag before
interpreting a former body, including valid empty deleted/relocated slots. Live
malformed ranges still fail. This does not change explicit physical target or
LVAL chunk validation, and opaque backing-field scalar coverage remains explicit.

Owned storage is not equated with current reachability. Orphan rows are framed
and counted while their bodies remain opaque: the first four bytes of an orphan
are not guessed to be a chain link. `owned_rows()`, `distinct_chunks()` and
`orphan_rows()` distinguish these facts. `shared_references()` and
`shared_owned_pages()` surface sharing without declaring it corrupt or exclusive.
The result does not authorize retirement or in-place edits.

All map/page/row/trace/sort work uses cumulative budgets and cancellation. Common
represented-map readers now serve this default check as well as preview writers;
preview-only future-cell allocation helpers and existing writer admissions retain
their boundaries. No normal save or journal path is enabled.

## Working consumer and evidence

The default-build read-only example buffers its report until the operation
completes and preserves every explicit status:

```sh
cargo run --locked -p tablecraft-ace --example check_long_values -- DATABASE TABLE
```

Its `key=value` diagnostic format is separate from CLI row/TSV rendering. It
prints counts and native column numbers, not stored values. It accepts an
explicit cumulative byte allowance without changing engine defaults.

The [scoped verification record](../../testing/results/long-value-storage-native-2026-10-26.json)
records final source/log identities and distinguishes repeated tests. An
initially frozen default example checked all **60 saved public outputs** on its
first attempt: all **120 column reports** match independently captured descriptor
kinds, map memberships, physical slot counts, orphans and sharing counts.
The final frozen consumer repeats the same 60 checks with the same 120 exact
reports; the record preserves both binary/source identities and counts these as
repeated controls. Every input and original remains unchanged. This is engine storage-check evidence,
not another writer run or Microsoft Access qualification.
