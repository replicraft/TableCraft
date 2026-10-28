# Required indexes with stored collection queries

An explicitly required native parent index now composes with qualified stored
attachment, Memo-history and multivalue members:

```sh
tablecraft query run --database copy.accdb --collections --index Items IdOrder \
  'SELECT Id, Files.FileName FROM Items'
```

The index and fields must meet their existing native read profiles. This adds
row alignment between those readers; it does not qualify another physical key
codec, execute lookup queries or decode attachment data. Query filtering remains
in TableCraft, and this collection path reads the complete required index rather
than pushing a predicate into a partial parent selection.

## Identity and completeness

`read_table_index_with_references` returns an `IndexedTableSnapshot`: the checked
scalar projection and one `NativeRowReference` per row, in actual index order.
References identify original logical DATA roots before forwarding. Complete
collection parent links retain those same logical references in sequential order,
including parents with absent keys or assigned empty collections.

The adapter revalidates the captured owner definition for every loaded collection,
including native column metadata, flags and offsets. It checks strict sequential
reference order and agreement across complete parent snapshots, then proves a
bijection from all indexed roots to those parents before calling any visitor.
Each comparison, allocation and lookup spends the existing cumulative budget;
cancellation remains checked throughout. Visible field values and row ordinals
are never used to infer identity. Duplicate scalar values therefore retain their
different memberships, and a forwarded root keeps its original membership.

References are scoped to the captured table definition and unchanged caller-owned
source handle. They are not durable IDs, cross-file identity, snapshot isolation
or mutation capabilities. `TableSchema::same_definition` compares captured
metadata, not row contents, property maps or an entire-file revision.

## Retained boundaries

- The query path requires a complete `All` index traversal. An omitting/unsupported
  index does not become a complete parent source; failures never fall back to a
  sequential scalar scan.
- Complete parent-key and child certification still precedes result publication.
  TOP, filters and an earlier warm scalar cache cannot hide a selected-index or
  child failure. This path does not cache a partial indexed relation.
- Metadata-only attachment queries keep their separate cache and omit FileData
  values/pages. Selecting stored FileData explicitly loads encoded Binary;
  decoding remains a separate opt-in. A failed payload read cannot poison the
  completed metadata cache.
- Selected cached-calculation scalars with a required index remain unsupported.
  They need a separately qualified stored-cache indexed reader. Ordinary scalar
  columns on the same table can still be selected beside stored collection members.
- Automatic constrained scalar scans and their table-order requirements remain
  unchanged. These references do not make collection execution progressive or
  provide live Access shared-file coherence.

## Evidence

Four engine tests cover exact references, zero-width projections, NULL/range
selections, index aliases, forwarding, scope/column-definition drift, resource
limits and cancellation. Eight independent adapter tests use literal four-parent
fixtures with deliberately different index/sequential orders and duplicate visible
IDs. They check both directions and ACE 16 layout families, absent/empty/NULL
memberships, forwarded roots, warm caches, aggregate queries, missing/duplicate/
hidden/foreign roots, late orphan failures and metadata-only payload isolation.
Explicit decoded attachment data and stored Memo-history members also retain
their alignment when complete caches are reused without reading child pages again.

The fixtures are repository-original synthetic data. These headless checks do
not establish Microsoft Access execution or native write compatibility.
