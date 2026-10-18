# Native index inspection

Index-page prefixes now use borrowed zerocopy layouts with explicit little-endian,
unaligned fields. Signature/owner/free-space/order/reference validation remains in
the decoder. Bitmap traversal uses bounded word blocks; byte work is precharged
and cancellation checked at each block (at most eight bytes), with existing
per-entry checks retained. [Benchmarks](testing/NATIVE_BENCHMARKS.md) compare the
original bit loop, word iteration and a SIMD candidate without changing the file
format or claiming native compatibility.

`inspect_table_indexes` returns a completed IndexInspection with owning table schema
and native logical index definitions. CLI: `inspect COPY --table NAME --indexes`.
It uses the existing bounded, unencoded Jet 4 / ACE-code-2, one-page table-definition
profile. Index pages and the selected table's data rows are not traversed by inspection.

Logical index numbers, physical data numbers, root/usage references, raw type/flags,
column numbers and direction are retained separately. Multiple logical indexes
can refer to physical index data. Native definition order is preserved; it can
differ from Jackcess's logical-number order. Column numbers resolve against native
logical column numbers, including current metadata gaps. This metadata is not a
schema uniqueness/foreign-key validation result or proof of a valid index tree.

## Layout and limits

The table header's logical count at 47 differs from its physical count at 51.
Following column names are 52-byte physical and 28-byte logical blocks, then UTF-16
index names. A logical definition selects physical data explicitly. Physical key
slots contain column number/direction pairs (ten maximum), followed by usage/root
references and flags. All offsets/tags are named in the owning definition decoder.
Counts are capped at 256, names use existing strict bounded decoding, and caller
allocation/work/cancellation budgets cover the operation. Unknown logical type codes
remain visible. Out-of-range physical/root/column references, duplicate identities,
invalid names, truncated definitions and unqualified key flags/layouts fail without
partial inspection. Chained definitions remain unsupported.

References reviewed as format research:

- [MDB Tools index layout](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/index.c), LGPL-2.0-or-later.
- [Jackcess IndexData](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java),
  [IndexImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexImpl.java)
  and JetFormat from the same revision, Apache-2.0.

No upstream implementation was copied or linked. Native metadata stays in ACE;
backend-neutral query contracts and shared schema do not import physical indexes.

## Independent metadata evidence

The original [IndexOracle](testing/IndexOracle.java) uses the already reviewed
external Jackcess 5.0.4 JAR read-only. Its protected root-page accessor is invoked
reflectively only in this engineering helper; this does not add Java to production.
The registered candidate matched all 60 key components across its 22 indexed tables
for names, logical/physical identities, root, type, flags, column order and direction.
After grouping by table/logical number while preserving key order, tagged output
SHA-256 is `e02d4a62e8f42d172982c33887ff2d62abc6e9bd341534f1f911d710a1a153b4`.

The original [IndexFixture recipe](testing/IndexFixture.java) creates only a new
V2000 or V2007 file with 1,000 permuted rows, a primary Int32 index, nullable descending
Int32, composite, Text and ignore-NULL indexes, plus an empty indexed table. Generated
ACE metadata matched 30 components/11 tables including system tables (hash
`33b93da870951f53a7bf9bf678eb426fbf535ed7cb62b334a10308e667f24328`);
Jet 4 matched 18/7 (hash `db6e6ec6b404a6059f8d43196f501d6cb6d838d7783210202b07bf55a23d1d47`).
This is independent-reader/writer evidence, not native Microsoft Access comparison.
Generated databases/JARs are not tracked or required by CI.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/IndexFixture.java path/to/new.accdb V2007
java -cp path/to/jackcess-5.0.4.jar docs/testing/IndexOracle.java path/to/copy.accdb
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --table ROULETTE_TB --indexes
```

Static candidate goldens, malformed metadata/limit/cancellation tests, a 256-case
fixed-seed mutation property and CLI tests cover the production path. Original
fixture bytes remain unchanged. Index writes, native constraints, automatic query
access-path selection and shared-file coordination are not implemented here.

## Bounded indexed reads

`read_table_index` reads a complete scalar TableSnapshot through an explicitly named
native index. The initial qualified profile is one Int32 key, normal/primary logical
type, ascending or descending, including NULL and duplicate keys. Ignore-NULL indexes
are rejected because they cannot supply a complete table snapshot. Composite, Text,
other key carriers and unqualified flags/types fail with Unsupported. Unknown index
names have a distinct payload-free error. No sequential fallback is attempted.

The decoder traverses root/node/leaf pages, reconstructs shared entry prefixes from
the entry-boundary bitmap, and decodes signed Int32 keys plus big-endian row/child
references. Fixed widths, markers, offsets, flags and ceilings have named owners.
It verifies page signatures/owner, free space, boundaries, key widths, strict key/row
ordering, parent separator bounds, owned-page membership, leaf sibling continuity,
cycles and a 16-level depth ceiling. At most 16,384 entries/live rows and the existing
page/read/allocation/work limits apply. Leaf count must equal the declared table
count. Referenced root slots must be live and belong to owned data pages; each root is
validated and fetched in one page read. Relocation
uses the existing bounded reader. Every decoded row must validate against its schema
and agree with the indexed key. A failure returns no partial snapshot.

This detects many structural inconsistencies; it does not independently establish
index completeness/constraints against an exhaustive separate data scan or provide
shared-file isolation. Read only stable/offline sources. Separator references may
be retained upper bounds, so they are checked as bounds rather than fetched as rows.
The recorded level byte is not used as a compatibility guarantee; depth is bounded
by actual traversal. Writes, index repair and native constraint enforcement remain
unimplemented. Native index key order is not a SQL ORDER BY guarantee.

The TableCraft adapter's `with_index(table, index)` selects this path when that table
is loaded; its complete snapshot is cached normally. Other tables retain sequential
reads. Reconfiguration discards that cache without refunding consumed budget.
Eligibility/name checking is deferred until the selected table is loaded. CLI:

```sh
cargo run --locked -p tablecraft-cli -- query run \
  --database docs/examples/Access_Example_VBA.accdb \
  --index ROULETTE_TB ORDR_NBR \
  "SELECT TOP 5 ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR > 4 ORDER BY ORDR_NBR DESC"
```

`--index` takes exactly TABLE and INDEX and requires --database. It is an explicit
snapshot access selection without silent fallback. Binding discovers scalar schema
without user data reads. [Safe Int32 filters](QUERY_EXECUTION.md#safe-source-constraints)
can select ranges through the required index; other predicates load it completely.
WHERE, ORDER BY, grouping and TOP still run in the reusable query engine. TOP
pushdown, index nested-loop joins and automatic index selection remain absent.
The native API supports the bounded selections below. Trace diagnostics expose the selected
operation/counts without recording table/index names or keys.

## Native Int32 range selections

`read_table_index_range` accepts `Int32IndexSelection::All`, `Null`, or `Range`
with standard-library `Bound<i32>` endpoints. Bounds use ascending numeric order
for both index directions; numeric ranges exclude NULL, and reversed/empty ranges
return no rows. These are storage intervals, without SQL coercion or Access
BETWEEN normalization. `All` retains the complete reader's validation and row count.
Partial snapshots retain the full declared count in schema metadata.

Interior separators prune disjoint children before page reads. Inclusive coarse
key bounds keep duplicate keys spanning separators; row references still enforce
strict physical ordering. Only selected live roots are decoded. Visited pages and
returned rows retain ownership, ordering, separator, sibling, key agreement,
budget, cancellation and I/O failure checks. The skipped tree and unselected rows
are not validated. A successful seek therefore does not establish whole-index
integrity. Sources must still be stable/offline, and unsupported selected index
profiles still fail without fallback.

Tests exercise open/closed/unbounded/empty/extreme ranges, nullable descending
keys, duplicate separators, pruning, corruption in visited versus skipped pages,
fixed-seed generated intervals and every read/seek failure callback. A six-row
selection skips the disjoint leaf and fetches exactly six live root records.

The original [range example](../crates/engines/tablecraft-ace/examples/index_range.rs)
and [comparison runner](testing/compare_index_ranges.py) reproduce 28 selections
across both generated file formats and both tested indexes. Exact rows/order
match independently filtered Jackcess cursor output, including NULL and empty
results. This is independent-reader evidence, not Microsoft Access qualification:

```sh
cargo build --release --locked -p tablecraft-ace --example index_range
python3 docs/testing/compare_index_ranges.py \
  --example target/release/examples/index_range \
  --jar path/to/jackcess-5.0.4.jar \
  --ace path/to/fresh-indexes.accdb --jet path/to/fresh-indexes.mdb
```

[Measurements](testing/NATIVE_BENCHMARKS.md#native-index-seeks) compare seeks with
complete indexed snapshots followed by equivalent filtering. The adapter connects
these selections to safe query filters after schema-only discovery; no complete
snapshot is cached from a selected read.

## Indexed-row evidence

The original [IndexRowsOracle](testing/IndexRowsOracle.java) iterates a named Jackcess
index cursor read-only and compares selected integer/NULL output. Both generated
Jet 4 and ACE files matched all 1,000 rows in each tested index, including order:

| Index | Compared fields | SHA-256 of newline-terminated rows |
| --- | --- | --- |
| PrimaryKey | ID, Category | `12f4a36e8244d40e212dbcd9d5b75c09076bb390835727f1378491c082bb4503` |
| CategoryDesc | ID, Category | `0749222c67a1b1ff9ece3e49e4355b0eb44c32ca847a5d605fe79561c7a18624` |

These trees contain interior/leaf pages and compressed duplicate keys. Generated
empty-table reads return zero rows; composite/Text/ignore-NULL selections reject.
The registered candidate matched all 38 rows in each index, comparing ORDR_NBR and
COLOR_ID: ORDR_NBR hash `d8924f903a21db26b2df0bffeb0789f70990fba99fbebf975543d1d567fa772c`,
COLOR_ID hash `c26da8191acf5faf10067334ace00f53719aa89931f1b77e723204281b4d2e32`.

Automated tests include independent compressed/descending/NULL byte goldens,
actual indexed/sequential row agreement, relocated live roots/deleted targets,
a synthetic two-level tree, separator/link
errors, cycles/depth, malformed keys/references, cancellation/budgets and a 256-case
fixed-seed index-page mutation property. Read/seek fault injection fails every callback
in a successful indexed read and verifies no snapshot. Cancellation after traversal
also prevents publication. Adapter tests verify cached indexed access and no fallback;
CLI tests preserve filtering/order/TOP semantics, failure status and buffered stdout.
This is independent-reader evidence, not native Access/provider qualification.
