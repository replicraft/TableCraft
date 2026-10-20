# Native text ordering

The current implementation identifies stored database/column sort descriptors,
encodes General legacy/General 2010 text-index payloads and reads qualified
single-column Short Text indexes. It does **not** yet
implement native SQL locale comparison, text-index range seeks or native
LIKE character behavior. An explicitly selected ordinal query collation
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

The General text-key encoder below now uses reviewed weight-data provenance and
independent key vectors. Checked single-column Short Text reads now work; next add range seeks and
implement explicit native SQL comparison. Do not
substitute Unicode normalization, lowercase conversion, an OS default locale or
generic ICU/UCA weights for measured Access behavior. Query comparison must keep
the backend-neutral supplied-collation contract; physical index interpretation
belongs in ACE. Preserve the original filter and fail unsupported index selections
without a sequential fallback. LIKE unit/range behavior requires its own evidence.
Other locales/variants remain explicit gaps until their behavior is implemented.

The private index walker now accepts keys without `Copy` or `Clone`: parent
separators are borrowed during recursion and selected leaf keys move into the
result buffer. Constraint checks borrow the previous key before replacing it.
This prepares traversal for variable-length text payloads without cloning them
or reserving a maximum-width buffer per entry. Existing mutation profiles retain
their explicit `Copy` requirement; text-index writes remain blocked.

## Checked Short Text index reads

`read_table_index` and `read_table_index_projection` accept single Short Text
indexes with explicitly recognized General legacy/General 2010 descriptors.
The CLI and Access adapter use the same reader with an explicit `--index TABLE INDEX`.
For the registered example:

```sh
cargo run --locked -p tablecraft-cli -- query run \
  'SELECT TAG_GRP_ID, TAG_GRP_NME FROM TAG_GRP_TB' \
  --database docs/examples/Access_Example_VBA.accdb \
  --index TAG_GRP_TB TAG_GRP_NME
```

Ascending/descending NULL markers, zero-length values and encoded payloads stay
distinct. Native page order, parent bounds, ownership, sibling chains, live row
references and complete row counts are checked. Every returned row is re-encoded
and must match its physical key, even if the text key is omitted from output.
No selected-index failure falls back to sequential reads.

Entry framing includes the value marker and caps the complete key at 510 bytes.
Longer encodings keep their first 508 bytes and replace the discarded suffix with
a two-byte polynomial digest. This is separate from the encoder's 255-UTF-16-unit
input prefix. Full-width keys need not end in a text terminator. The original
bitwise digest implementation has 24 independently recorded boundary/overflow
vectors from `KeyDigestOracle.java`; it does not use a copied reference lookup
table. Row re-encoding includes this framing before comparison.

The shared work context charges owned key buffers, byte/character scans and
digest processing to the caller, with cancellation checks before publication.
Full scans may be cached as complete snapshots; no text predicate pushdown or
range snapshot is introduced. The original SQL filter/sort and explicit ordinal
collation policy remain in force. Physical truncated keys never supply full-string
SQL equality or ordering.

Read-only checks validate eligible text indexes and exact short-key uniqueness.
A unique index containing a full-width key remains explicitly unqualified for
constraint checking: equal digests alone do not prove equal values. Unknown
LCIDs/variants/versions, text compound keys, Memo indexes and ignore-NULL indexes
remain unsupported. Ordinary native text-index writes remain blocked.

`TextIndexFixture.java` and `compare_text_indexes.py` exercise both General
families on original Jet 4 and ACE 2007/2016/2019 databases, including multi-page
ascending/descending indexes, NULL/empty values, case/trailing-space equivalence,
accents, ignored units, kana, supplementary characters and long/digested keys.
The recipe compares exact tagged rows, full/projected CLI queries, retained
filters/sorts and completed smaller-table checks, plus explicit refusals. The
General 2010 1,000-row cumulative check deliberately exceeds the fixed CLI step
budget and must publish no partial report. Sources remain unchanged. These are
independent-writer/reader comparisons, not native Microsoft Access qualification.
Registered-example regressions also verify text cursor order and reject a valid
row reference paired with another row's text key, including projected reads.

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
all four bytes rather than silently dropping it. The metadata implementation
introduced no upstream implementation/data. The subsequent General key encoder
adopts licensed adaptations as described below; no new registry dependency is introduced.
Native Windows/Access comparison, locale-equivalent SELECT results and general
text-index reads/seeks remain unqualified.

## General text-index payload encoding

`encode_text_index_key` accepts decoded Rust text; `encode_utf16_text_index_key`
also accepts raw native UTF-16 units, including separated surrogate units, without
replacement-character coercion. Both require a known General descriptor, caller
budgets and cancellation, and return a non-Debug `NativeTextKey`. Its bytes exclude
the column value marker and row/child pointer. Unknown LCIDs/variants/versions fail;
there is no locale fallback or implicit query-policy activation.

The physical format keeps the first 255 UTF-16 units and then discards trailing
ASCII spaces. A prefix boundary may separate a surrogate pair. These are index
key rules, **not a general SQL string equality/ordering implementation**. Do not
use this API to silently truncate arbitrary query strings or Memo values.

Both General families use their actual primary weights, character/entry counts,
secondary modifiers, unprintable offsets, expansion and kana/repeat rules. General
2010 also applies surrogate-unit weights, short-weight padding and the native
primary/tertiary overflow-tail behavior. Descending encoding inverts the proper
payload and adds its extra terminator. Fixed-size and length-derived temporary
capacities are charged before allocation, appends remain bounded, and final
cancellation prevents publication. No cache, network service or OS locale is used.

The [bundled data/adaptation record](../crates/engines/tablecraft-ace/data/general/README.md)
contains exact provenance, normalized artifact hashes, Apache-2.0 license/notices
and the deterministic engineering generator. Two adapted Rust encoder files and
the data retain Apache-2.0 obligations; other original project code remains MIT.
ACE's SPDX expression records both. Existing zerocopy supplies endian-explicit
offset reads and alignment-one borrowed weight records. Normal builds require no
Java, Python or build script and allocate no runtime weight table.

`GeneralKeysOracle.java` independently supplies 280 multi-character vectors
(560 ascending/descending keys) and 512 block digests covering every one of the
65,536 UTF-16 units in both families/directions (262,144 keys). The corpus includes
case, accents/combining marks, expansions, controls, Arabic modifier/overflow
sequences, Lao short weights, kana repeats/prolonged sounds, unpaired and non-BMP
surrogates, empty strings, trailing spaces and overlong input. A further 512
generated raw-unit sequences exercise bounded encoding. Resource/cancellation,
unknown-profile refusal and the UTF-8 wrapper also have focused tests.

This verifies an independent encoder implementation against the same reviewed
weight data. It does not independently validate those weights against native
Access, qualify text-index B-tree traversal or establish SQL/LIKE equivalence.

### Encoding measurements

On the local Apple Silicon macOS/Rust 1.99 release build, the `text_keys` benchmark
measured **about 598 nanoseconds per owned General legacy key** and **702
nanoseconds per owned General 2010 key**, over eight fixed mixed-script strings.
Allocation, budget accounting, encoding and key destruction are inside the timer;
format selection and corpus setup are outside it. Neither case measures I/O,
index traversal, SQL comparison or native Access performance.

Hyperfine 2.0.0 used two warmups and eight accepted runs per case, with 10,000
iterations of the eight-string corpus (80,000 keys per run). Complete process
times were 50.2 ± 1.6 ms and 59.8 ± 10.0 ms respectively. All accepted samples,
including slower General samples, are retained; these are different native
formats with different output/work, not optimization contenders or a speedup.
The benchmark reports raw per-run nanoseconds and a stable checksum; its raw
Hyperfine JSON and process transcript were retained for the engineering run.

Build with `cargo bench -p tablecraft-ace --bench text_keys --no-run --locked`;
run the printed executable with `0 --iterations 10000` or `1 --iterations 10000`.
