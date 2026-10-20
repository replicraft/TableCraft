# Native text ordering

The current implementation identifies stored database/column sort descriptors.
It does **not** yet implement native locale comparison, text-index weights or
native LIKE character behavior. An explicitly selected ordinal query collation
retains its existing meaning; metadata recognition never silently replaces it.

## Working metadata boundary

`tablecraft-ace::inspect_database_sort_order` reads the Jet 4 / ACE root-page
descriptor under caller budgets and cancellation. It needs no password or user
records, including for encoded databases: page zero uses the fixed header mask,
not the data-page cipher. Jet 3 and unqualified families/subversions fail.
The mask implementation is shared with native encryption rewrites; owned root
prefix buffers are zeroized and no keys/password fields enter output or tracing.

`NativeColumn::text_sort_order` returns the exact descriptor for Short/Long Text
columns only. Numeric precision/scale and complex-type metadata occupy overlapping
native bytes and must never be interpreted as text settings. Unknown/zero text
descriptors remain visible without preventing existing raw scalar decoding.

`NativeSortOrder` retains the little-endian 16-bit LCID, 8-bit sort variant and
8-bit weight-table version. The four-byte descriptor is at root offset 110 and
column descriptor offset 11 for the current 4,096-byte-page families. Known
General descriptors use LCID 1033, variant zero, version zero (General legacy)
or one (General 2010). `general_family()` recognizes those facts only. Zero
LCIDs, other locales/variants and unknown versions return no recognized General
family; no OS locale or release-dependent default is substituted.

CLI `inspect PATH` reports Text sort LCID/version/variant/family beside encryption
metadata. `inspect PATH --table NAME` adds a separate text-column descriptor
table after the existing scalar metadata table. Terminal tables and escaped TSV
use the existing presentation boundary. ACCDC contained databases use the same
APIs. Inspecting any of these fields activates no application/provider content.

## Qualification and next implementation

Original independent header bytes exercise Jet 4 and ACE codes 2/3/5/6, known and
unknown LCIDs/variants/versions, encoded headers, budgets and final cancellation.
Native schema tests keep column settings distinct from database defaults and
non-text fields. CLI goldens cover a known General legacy descriptor and the
registered example's text column. Existing row/query and encryption behavior
must remain unchanged when the shared header mask or field metadata changes.

`SortOrderOracle.java` and `compare_sort_orders.py` compare database and text-column
descriptors through read-only Jackcess with expression evaluation disabled. The
script checks unchanged source hashes. Jackcess's zero-LCID fallback is deliberately
not adopted: raw unknown descriptors stay unknown in TableCraft. This comparison
qualifies metadata extraction, not locale ordering or native Access execution.

Ten inputs agree on their database descriptor and one selected table's text-column
descriptor: the registered ACE 2007 example; `pwd-none.mdb`, `db-enc.mdb`,
`db2007-enc.accdb`, `db2013-enc.accdb`, `db2007-oldenc.accdb`, `db-nonstandard.accdb`;
and original generated `Plain.accdb`, `Binary-128-5.accdb`, `Standard-256-6.accdb`.
These exercise plain/encoded Jet 4 and all current ACE read families/codecs;
they do not constitute native authored-profile or exhaustive locale coverage.

Next, implement explicit General legacy/General 2010 text-key/comparison behavior
with reviewed weight-data provenance and independent key/ordering vectors. Do not
substitute Unicode normalization, lowercase conversion, an OS default locale or
generic ICU/UCA weights for measured Access behavior. Query comparison must keep
the backend-neutral supplied-collation contract; physical index interpretation
belongs in ACE. Preserve the original filter and fail unsupported index selections
without a sequential fallback. LIKE unit/range behavior requires its own evidence.
Other locales/variants remain explicit gaps until their behavior is implemented.

## References and evidence limits

Microsoft's [Database.CollatingOrder](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao)
documents database/field-specific sort orders and their relation to database
creation/compaction locale. [LCID structure](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/63d3d639-7fd2-4afb-abbe-0d5b5551eef8)
is locale context, not a specification for native Access index weights.

Exact physical-field interpretation was checked against the already reviewed
Jackcess 5.0.4 binary and its Apache-2.0 source JAR (SHA-256
`4de1bc2f1663b0946ce39057dd90eace355f4f74b8f818c0fe167f0c65815428`),
`ColumnImpl.readSortOrder`, `SortOrder.getVariant` and `JetFormat` offsets.
The older pinned mirror does not expose the intervening variant byte; preserve
all four bytes rather than silently dropping it. No upstream implementation,
weight data or database bytes are copied, and no new dependency is introduced.
Native Windows/Access comparison, locale-equivalent SELECT results and general
text-index reads/seeks remain unqualified.
