# Read-only queries over native scalar tables

Explicit [cached calculation queries](CACHED_CALCULATION_QUERIES.md) extend an
opted-in source with qualified stored fields. Default eligibility, read-only
ownership and recomputation/freshness boundaries remain unchanged.

The working `crates/backends/tablecraft-access` package adapts ACE snapshots to the
existing `tablecraft-query::Source` interface. `NativeSource` accepts a caller-supplied
NativeReadSource (raw Read + Seek with explicit page interpretation), catalog namespace, cumulative ResourceBudget and cancellation
callback. It inventories metadata inertly and discovers scalar schemas without
loading user records. Complete table snapshots are cached for repeated scans,
joins and subqueries; bounded selected snapshots are ephemeral.
Sequential full/projected loads now use checked 256-row cursor batches, then
publish one complete cached snapshot. This removes the materialized 4,096-address
list from those loads; a host with an explicitly sufficient native byte budget
can query tables spanning more pages. The default 16,384-row snapshot ceiling remains,
and budgets never refill between batches. Explicit indexed reads retain their
existing ownership/range checks. See [batched-query evidence](testing/BATCHED_QUERY_SNAPSHOTS.md).
No application host, native provider, GUI, network runtime or new registry dependency
is required. ACE and query do not depend on the adapter; query still performs no
native I/O. Backend-neutral connection/cursor/runtime services remain unimplemented.

The CLI opens only an explicit `--database PATH`, read-only. Qualified Agile
inputs accept global `--password`/`-p`; metadata/key derivation and native reads
consume the same allowance. [The encryption contract](NATIVE_ENCRYPTION.md)
separates original raw bytes from decoded native pages. Other schemes fail
explicitly; native UPDATE copies are not enabled by a password. Native CLI
reads use [shared OS locks](NATIVE_TRANSACTIONS.md#cli-read-coordination) to exclude
cooperating exclusive writers. Use a stable/offline copy; native Access shared-file
coordination and concurrent snapshot acquisition
are unimplemented. Explicit bounded [offline read transactions](NATIVE_TRANSACTIONS.md)
remain a separate API. Without that option, `query run` retains its empty catalog and
inline-source behavior. `query parse` and `query eval` remain independent of files.

```sh
cargo run --locked -p tablecraft-cli -- query run --database path/to/copy.accdb "SELECT TOP 5 ORDR_NBR, NBR_VALUE, COLOR_ID FROM ROULETTE_TB ORDER BY ORDR_NBR"
```

On a byte-identical copy of the registered example this prints:

```text
ORDR_NBR  NBR_VALUE  COLOR_ID
1         "2"        0
2         "0"        65280
3         "28"       0
4         "9"        255
5         "26"       0
```

Actual output uses bordered terminal presentation or CSV-encoded escaped-display TSV. Filtering, sorting,
TOP, parameters, joins, aggregates, subqueries and explicit text policies use the
existing [bounded retrieval profile](QUERY_EXECUTION.md). `--collation ordinal`
retains Unicode order; `general-legacy`/`general-2010` select the provisional
[full-string General model](NATIVE_TEXT.md#general-query-policy) under separate
cumulative limits. Native-only `--collation database` instead selects the stored
database default and per-field General policies under one shared quota. Unknown
required field policies and expressions combining distinct identities fail.
Opted-in `.Value`, attachment and stored-history Text members retain their stored flat-field
policies; decoded FileData remains Binary. [Stored-policy scope](NATIVE_TEXT.md#stored-database-general-policy) records the
conservative boundary. Family overrides infer no field/database locale. Native
SQL qualification remains pending. Direct scalar Short Text equality can use
[comparer-qualified index candidates](TEXT_EQUALITY_SEEKS.md); collection member
predicates do not use that hint profile. A table's stored row order
is not a SQL ordering guarantee; use ORDER BY for deterministic selection.

The source also supplies [opaque text-policy identities](QUERY_EXECUTION.md#source-text-policy-identity)
from exact scalar and opted-in stored collection field descriptors. Plans preserve/check those identities without
activating comparison or decoding native flags. Native full/projected snapshot
schema checks retain the same distinction. The explicit native resolver now
uses the metadata for per-operation routing; native mixed-expression precedence
remains unqualified.

## Explicit supported boundary

[Native scalar tables](NATIVE_TABLES.md) defines the accepted unencoded profiles
and column carriers; [ACE 16 coverage](NATIVE_ACE16.md) has its own evidence.
Native schemas now expose ordinary scalar columns plus explicit unavailable
field names. A scalar query can read ID/Memo fields from a table that also has
collections, calculations or other unqualified fields. References to unavailable
fields, including matching `*`/`alias.*`, fail before scanning; no field is silently
omitted from a wildcard or replaced with NULL. This applies to empty tables too.
Derived queries with explicitly selected scalar outputs have ordinary scalar
schemas and may be expanded normally. COUNT(*)/constant projections can count
checked native rows even when the source has no available scalar columns. Linked tables,
saved-query sources, external providers and active database content are never
opened/executed as sources. This retrieval source performs no action/DDL or native
writes; qualified offline UPDATE copies use the separate [editing path](QUERY_EDITING.md).

[Saved-query metadata inspection](NATIVE_SAVED_QUERIES.md) is now available through
`inspect --query NAME`. It preserves stored fragments without SQL reconstruction
or execution. The separate Access adapter now reconstructs its qualified local
SELECT profile into owned syntax through `query saved --database PATH NAME`;
neither path makes saved-query names eligible
FROM sources.

Execution normally materializes all live rows before evaluating clauses. An
explicit eligible index permits [safe integer/Currency/GUID filter pushdown](QUERY_EXECUTION.md#safe-source-constraints);
opt-in [automatic selection](AUTOMATIC_INDEXES.md) now chooses matching qualified
single-key indexes. TOP pushdown and cost-based optimization remain unimplemented.
Explicit Binary index ranges are available to library hosts through
`read_table_binary_index_range` and projected Binary selections. The query adapter
still applies Binary predicates after complete snapshots; it does not automatically
push them into the new reader. [Range evidence](testing/BINARY_INDEX_RANGES.md)
records borrowed bounds, pruning and partial-validation limits.
Single Short/Long Text indexes now supply complete checked snapshots and projections
for General legacy/General 2010, including entry-level overflow digests. Library hosts can explicitly select physical Text start/end intervals; the query
adapter does not push text predicates into them or replace the supplied SQL collation.
See [text eligibility and evidence](NATIVE_TEXT.md#checked-short-text-index-reads).

The CLI supplies 16 MiB cumulative native bytes,
16,384 cumulative catalog/table rows and 1,000,000 native steps by default across the adapter;
each reader also enforces structural ceilings. Repeated catalog/schema reads for
new tables consume that shared budget. Sequential cursor loads enforce the physical
read quota per batch and charge aggregate row-vector storage as well as batch
allocations; they still consume the same cumulative native byte/row/work controls.
The CLI's 16 MiB native allowance can therefore still refuse large valid tables.
`query run --database PATH --native-bytes BYTES` explicitly replaces only that
cumulative native read/allocation allowance (1 through 268,435,456 bytes). Omitting
it retains 16,777,216 bytes. For example:

```sh
tablecraft query run "SELECT TOP 5 ID FROM LargeRows ORDER BY ID DESC" --database copy.accdb --native-bytes 67108864
```

The option applies to explicit native SQL and `query run --saved`, including
catalog/schema/saved-source reads and indexed paths. It requires `--database`
and is unavailable to SQLite, source-free retrieval, parsing, evaluation,
`query saved`, inspection, ordinary buffered browsing and writes. Explicit
`rows --stream` has its own similarly named read controls. Clap validates the inclusive byte
range before file I/O; values are decimal bytes, not unit-suffixed strings.
This allowance is cumulative work/storage accounting, not a file-size limit or
an up-front allocation. It does not change row/step/structural ceilings, query
execution resources, output bounds, shared-file guarantees or write qualification.

Retrieval separately uses its documented
16 MiB / 100,000-row defaults and a default 1,000,000-step allowance. Explicit
`--execution-bytes`, `--execution-rows` and `--execution-steps` can raise these
separate shared pools to 256 MiB / 1,000,000 rows / 100,000,000 steps. Output is buffered under the shared
1 MiB logical ceiling. Failures during load, bind, execute or formatting exit 2
with no partial result/header and diagnostics that omit paths/names/values.
I/O/broken-pipe behavior remains unchanged.

Cached tables do not repeat native reads. Cancellation applies to cached scans
as well as decoding; visitor rejection stops immediately. Host I/O timeouts and
stable-source ownership remain the caller's responsibility. This adapter is
synchronous and does not expose concurrent mutation or connection pooling.

## Evidence and limitations

Focused adapter tests execute a real query over a candidate copy, count reads to
verify caching, reject unsupported/missing sources, check empty relations and
cancellation/visitor rejection. CLI tests cover the documented TOP result,
filter/parameter/order behavior, Count(*), self-join, empty table, explicit text
collation, action/type/name/open failures, no partial stdout and unchanged bytes.

The underlying 1,488-row Jackcess comparison is recorded in NATIVE_TABLES.md.
It verifies independent reading, not Microsoft Access query/expression behavior.
No Microsoft Access oracle, application activation, native edit/save/reopen,
shared-file operation or Windows/Linux execution has been qualified by these checks.

Legacy Date/Time projection, temporal comparison/order, [SQL literals](SQL_DATE_LITERALS.md)
and the bounded ISO parameter/output profile are implemented; see
[native date contracts](NATIVE_TABLES.md#legacy-datetime).

Bounded Long Text snapshots now include inline and overflow/chained native values;
see [Long Text limits](NATIVE_TABLES.md#bounded-long-text). Markup remains inert data.

GUID projection/equality/literals/parameters and binary hexadecimal output are
implemented; [GUID limits](NATIVE_TABLES.md#guid-and-binary-output) distinguish
equality from unqualified native index ordering.

Qualified exact native Decimal fields support projection/comparison/ordering under
the existing scalar rules; [Decimal limits](NATIVE_TABLES.md#exact-native-decimal)
record the accepted storage/metadata profile and independent comparison evidence.

Bounded native OLE values now project as opaque hexadecimal Binary. The
[OLE profile](NATIVE_TABLES.md#opaque-long-binary--ole) records limits, inertness
and comparison evidence. MSysObjects is readable through the same scalar source.

Existing dense appended-column histories can now be read without altering the
file; [history limits](NATIVE_TABLES.md#dense-appended-column-histories) distinguish
absent fields from evaluating current defaults. Qualified deleted-column histories
also retain native identities; known fixed-prefix values can now be read when no
surviving variable field existed at that row count. Deleted tail bytes remain
unqualified and do not authorize writes.

## Larger sequential snapshots

[Explicit scalar snapshot and execution controls](LARGER_NATIVE_QUERIES.md) now
permit larger sequential query inputs without changing defaults or publishing
partial caches/results. `--native-rows` and `--native-steps` require `--database`;
all native budget flags explicitly conflict with SQLite. Separate `--execution-steps`
controls shared evaluator/comparison work. Existing query byte/row/output caps,
index structural limits and collection/template profiles still apply.

## Explicit native index access

Use `--index TABLE INDEX` with --database, or NativeSource::with_index in library
hosts, to load that table through a qualified one-to-ten-component native
index (Boolean/Byte/Int16/Int32/Int64/Currency/Single/Double/legacy DateTime/GUID, including mixed tuples) under the
owning format profile. Other
tables retain their ordinary access path. The engine validates tree/row references
and key agreement. The Access adapter caches complete snapshots and never falls
back on an ineligible/corrupt required index. Eligibility is checked when the
selected table is loaded.
See [NATIVE_INDEXES.md](NATIVE_INDEXES.md#bounded-indexed-reads) for the exact profile,
limits and evidence. Wholly safe Int32/Int64 comparisons and NULL tests can use
the matching typed range API on a single matching-width key. Exact GUID equality
and IS NULL use the single-GUID point reader, including scalar projections of mixed
schemas. GUID literals and exact typed parameters retain all 128 canonical bits;
GUID ordering and non-equality predicates receive no point hint. Qualified
[Currency intervals](CURRENCY_RANGES.md) now use exact typed bounds and retain
original filtering. [Byte/Int16 intervals](NARROW_RANGES.md) also use exact matching-width bounds.
Boolean and composite indexes retain complete reads. Floating indexes likewise use complete reads; query's existing
non-finite source rejection applies even to ID-only projections. Native indexed
snapshots retain exact IEEE bits independently of that query boundary. The original WHERE evaluates over
returned rows. Unrelated/unsafe predicates use the complete required index.
Legacy DateTime also uses complete reads: exact native serial order does not
replace the evaluator's civil-time equality/sort. Both calendar aliases remain
visible to the original filter, with no unsafe temporal seek pushdown.
ACE 16-family single and composite Decimal indexes also supply complete exact
snapshots; integer seeks over those profiles remain rejected. Each declared scale
is preserved, and original ID/NULL filters run over the required indexed source.
Bounded selections never enter the
complete-table cache; a later unconstrained query loads the entire table.
Within each native operation, consecutive references share one current data-page
buffer. The native buffer ends with that operation; it cannot turn a partial
snapshot into the adapter's complete-table cache. Required-index traversal,
residual filtering, fresh full loads and row budgeting retain separate checks.
Seeks validate only visited pages and selected rows, not skipped tree integrity.

The original [CLI comparison runner](testing/compare_query_filters.py) matched
32 exact outputs against independently filtered/sorted Jackcess cursor rows:
ascending and nullable descending indexes in generated Jet 4 and ACE files,
including open/closed/empty intervals, NULL/non-NULL and rebound parameters.
This does not qualify Microsoft Access query semantics. Reproduce after creating
fresh inputs using the existing index recipe:

```sh
cargo build --release --locked -p tablecraft-cli
python3 docs/testing/compare_query_filters.py \
  --cli target/release/tablecraft --jar path/to/jackcess-5.0.4.jar \
  --ace path/to/fresh-indexes.accdb --jet path/to/fresh-indexes.mdb
```

[ACE 16-family comparisons](NATIVE_ACE16.md) now exercise these bounded reads on
original V2016/V2019 fixtures. Native Access comparison and ACE writes remain unqualified.


## Scalar views of mixed native schemas

```sh
tablecraft query run "SELECT ID, Notes FROM Collections ORDER BY ID" --database example.accdb
tablecraft query run "SELECT Count(*) FROM Collections" --database example.accdb
```

`Source::columns` exposes the complete ordinary scalar view; the optional
`Source::unavailable_columns` contract retains all other declared names. NativeSource
captures both atomically without reading user records. Unavailable declarations
remain part of binding/ambiguity, qualified wildcard refusal and execution schema
checks, but occupy no runtime row cells. Joins and outer-join padding use only
available row width. Native names and profiles retain their existing qualification.

The engine's explicit `read_table_projection` reads named ordinary carriers in
native logical order. Its empty selection returns zero-width records for checked
live-row counts. NativeSource projects its ordinary scalar view when the native
schema contains unavailable fields. It validates row framing, allocation ownership
and declared live counts while skipping unselected value decoding/long-value
reads. This does not validate collection children, unavailable payloads or rules.
Retrieval plans now prune unused ordinary fields as described below. Requested
records remain materialized under cumulative source controls and output buffering.

Explicit native indexed reads now support these mixed scalar views through
qualified projected index snapshots, with no sequential fallback. The adapter
maps constraint positions from dense scalar columns to native key declarations;
eligible single Int32/Int64 seeks retain exact range behavior. Scalar-only
indexed reads retain their existing behavior. No collection `.Value` SQL expansion, calculated-field evaluation,
latest-history selection or relaxed write qualification is introduced.

The original explicit-getter oracle and mixed ACE 2007/2016/2019 inputs pass 45
query/refusal controls: scalar projections after physical collection columns,
filters, counts, both outer joins, correlated reuse, derived wildcards, unavailable
references/wildcards and explicit-index refusal. Byte hashes remain unchanged.
These compare inert getters and simple independent result models; native Access
SQL/locale conformance remains unrun.


## Indexed mixed scalar views

`--index MixedIndexed IdAsc` remains a mandatory native index choice, including
when the table contains unavailable fields. Complete projected reads inherit the
existing qualified scalar/composite index profiles and key-order checks. Safe
single-Int32/Int64 filters can prune ranges using dense scalar-view positions;
other predicates use the complete selected index and retain the executor filter.
No unsupported or missing selected index falls back to a sequential scan.

`read_table_index_projection` accepts explicit output names and a typed
`IndexProjectionSelection`. It decodes the union of requested ordinary fields
and required key fields, reconstructs each key using native positions, checks it
against the index entry, and removes unrequested key cells before returning rows.
Key validation never fabricates NULL placeholders. Empty output selections retain
checked row counts, not skipped key validation. Full traversal checks complete
counts; bounded reads only qualify traversed pages/returned rows. Unselected
non-key payloads and skipped subtrees remain unchecked, and unavailable fields
remain unsupported for SQL references/wildcards. Writes are unchanged.

Fresh original 1,000-row ACE 2007/2016/2019 inputs pass 198 independent cursor,
filter/count/order/range/refusal controls. Cases place native keys after collection
fields and include ascending/descending/composite scans, NULLs, duplicate keys,
Int32 endpoints and exact Int64 values beyond 2^53. General Short Text indexes also participate in the projected read path;
missing and ignore-NULL indexes publish no success output. Synthetic checks additionally verify omitted
key reconstruction across existing GUID/Decimal/tuple profiles and disjoint
corrupt-leaf pruning. [Fixture records](FIXTURES.md#mixed-schema-index-projection-inputs)
retain independent evidence separately from native Microsoft Access qualification.


## Query projection pruning

NativeSource implements the query's projected-scan contract for ordinary scalar
fields in both scalar-only and mixed tables. Only fields needed by output, filters,
sorts, joins, grouping and correlations are decoded. Unrequested ordinary payloads
and long values may remain unread, so a query can succeed even if an unused scalar
payload is invalid. Complete row framing, ownership and declared count checks
remain; indexed reads additionally decode and verify every required key field.
This is a selected-read guarantee, not a whole-table payload audit. Explicit index
choices retain their qualification and never fall back to sequential reads.

Complete snapshots are cached by requested field set; a complete cached superset
can satisfy a narrower query without I/O. Zero-width COUNT snapshots cannot satisfy
later value requests. Bounded index range results remain ephemeral, and the original
WHERE still runs. Cache storage and selection maps consume cumulative source budgets;
changing the selected index clears caches without refunding consumed allowances.
Metadata declarations are checked independently of selected payloads. Ordinary
`scan` remains a complete scalar-view read. Query rows remain bounded and buffered,
with no streaming or TOP pushdown. Derived output expressions retain eager evaluation.

Synthetic regressions cover unused malformed dates, complete/superset and correlated
cache reuse, independent projection shapes, range isolation and indexed selected
reads. Independent getter/cursor comparisons retain the mixed-schema corpus and
add narrow outputs with non-output filter/sort fields. These checks do not establish
native Microsoft Access SQL/locale compatibility.


## Explicit saved SELECT input

`tablecraft query run --saved NAME --database PATH` executes an explicitly selected
saved SELECT/UNION/crosstab through the same bounded binder/evaluator as SQL text. Use
`--param`, `--collation`, `--like-mode`, `--index` and `--format` normally. SQL text
cannot be combined with `--saved`, and SQLite cannot supply this native metadata.
The shared source reads native fragments and reconstructs owned syntax under the
same cumulative source controls used for subsequent reads. Reconstruction alone
(`query saved`) remains inert. Saved-query sources are not recursively expanded;
owner-access, UDFs and unsupported query profiles still fail. See
[execution and evidence](NATIVE_SAVED_QUERIES.md#explicit-saved-select-execution).


Named local saved SELECT/UNION queries now resolve during explicit SQL/saved retrieval through
NativeSource's bounded metadata cache and the shared binder. This supersedes the
earlier nested-source restriction above. Aliases, per-definition parameter scopes,
compatible supplied values, cycle errors and planning limits follow
[the named-source contract](QUERY_EXECUTION.md#named-retrieval-sources). No inspection
or startup execution, remote source opening, owner authority or native Access
qualification is implied.

## Larger source-file envelope

Qualified native sources up to 2 GiB can now supply bounded scalar queries without
implicit complete-image capture. The source still materializes bounded requested
snapshots and caches; table map/page/row and cumulative source/execution limits
remain unchanged. This does not add streaming or TOP pushdown. Complete-image
preservation/edit APIs retain a separate 64 MiB cap. See
[large-file evidence](testing/LARGE_FILE_READS.md) for high-offset page checks,
independent padded-input comparisons and limits on the qualification claim.


Explicit `--index TABLE INDEX` also accepts qualified text-bearing compound
indexes with two through ten mixed scalar components. Projection still decodes
all key fields and reconstructs the whole physical key before exposing rows.
Per-component direction/collation and whole-entry overflow digests stay inside
ACE. The adapter adds no SQL text pushdown or compound seek; original evaluator
filters, sorts and explicit ordinal policy remain unchanged. See
[the native tuple contract](NATIVE_TEXT.md#compound-text-index-reads).


The same explicit index path now accepts text-free mixed tuples containing Short
Binary fields. Complete/projection reads verify all key fields and whole-entry
overflow digests. Typed Binary ranges remain single-column; no compound seek or
Binary predicate pushdown is inferred. [Scope and evidence](testing/BINARY_INDEX_TUPLES.md)
retain SQL, mutation and native Access qualification gaps.

## Explicit multivalue .Value queries

`NativeSource::with_multivalues()` and CLI `query run --database PATH --multivalues`
connect qualified ACE 2007/2016/2019-family stored scalar collections to the
[backend-neutral expansion contract](MULTIVALUE_QUERIES.md). Ordinary queries
retain the previous metadata path unless opted in. Schema inspection resolves
complex mappings and qualifies element carriers without reading parent/child data
records. It never evaluates lookup queries, formulas or active content.

Selected collections load complete validated native parent links/child snapshots,
including ownership, reference and 100-value limits. Caches remain separate from
scalar/index projection snapshots. Unreferenced collections are not loaded, so
unselected invalid floating payloads do not block an Int32 member query. Native
constant-time child/value lookup borrows stored carriers without payload clones.
Every parent callback pairs a sequential scalar projection with checked collection
views; parent counts must agree. Scalar parent count queries retain original
cardinality before/after expanded reads. The query layer owns duplication, NULL
padding, filtering and grouping rather than the adapter.

An explicitly selected index on an expanded parent table is Unsupported; other
tables can retain their chosen indexes. Expansion does not silently change order,
use partial scalar snapshots as complete data, or manufacture scalar parent keys.
Stable/offline inputs and cumulative native/query/output budgets still apply.

Three adapter tests cover all qualified ACE families, no-data binding, separate
complete caches, parent counts, cancellation, malformed child data and explicit
index refusal. Two CLI tests cover opt-in, actual buffered results, invalid scope,
criteria-only rejection and unchanged input bytes. A native test separately
qualifies metadata inspection and direct value lookup. The
[reference recipe](testing/compare_multivalue_queries.py) passes 75 controls over
three original typed multivalue inputs using actual parent IDs and stored values
from read-only Jackcess APIs. It checks six finite scalar families, exact Decimal,
GUID/Text, duplicates/empty/NULL cases, grouping/counts, products, untouched scalar
counts and buffered refusals, while preserving source hashes.

These are native storage/independent row-model comparisons. Microsoft Access SQL
has not run. Scalar-parent and mixed displayed/hidden criteria now use the
documented explicit membership profile. Explicit [stored-history member views](STORED_HISTORY_QUERIES.md)
now reuse checked native records; collection display values, native history SQL,
native empty/product/NULL semantics, locale/Variant return promotion and
Windows Access qualification remain explicit gaps. This feature does not change
native write restrictions or make VBA/lookup execution available.

## Attachment metadata member queries

```sh
tablecraft query run 'SELECT ID, Files.FileName AS Name, Files.FileType AS Kind FROM Collections' --database copy.accdb --collections
tablecraft query run 'SELECT ID, Count(Files.FileName) AS N FROM Collections GROUP BY ID ORDER BY ID' --database copy.accdb --collections
```

`--collections` requires `--database` and conflicts with `--multivalues`. It enables
qualified `.Value` lists plus named stored attachment metadata on ACE 2007 and
ACE 16-family (2016/2019) layouts. Default and `--multivalues` behavior remain
unchanged. Library hosts opt in with `NativeSource::with_collections()`.
The same explicit mode also exposes [stored Memo history members](STORED_HISTORY_QUERIES.md)
under actual resolved hidden field names, without replacing the visible current Memo.

Supported members are `FileURL` (Text), `FileName` (Text), `FileType` (Text),
`FileTimeStamp` (legacy DateTime) and `FileFlags` (Int32), all nullable. Stored
labels, empty strings, NULL, timestamp bits and flags are retained; no MIME/path
inference, URL resolution or active content runs. Metadata members of one field
remain aligned to the same stored child. Distinct collections still form bounded
products; empty child sets retain the documented NULL-extension policy.

Binding uses `inspect_attachment_declarations` without reading parent/child records,
retaining each flat Text member's exact policy identity.
Execution lazily caches a complete metadata projection per referenced field,
separately from scalar projections, `.Value` collections and full attachment
snapshots. All parent keys, child references/identities, selected metadata and live
counts are checked before publication. Metadata-only queries omit FileData values
and never visit their external long-value pages. The omitted payload is unavailable,
not a fabricated NULL, and its integrity remains unqualified. This permits metadata
queries over valid files beyond the existing encoded/decoded payload limits.
`FileData` is available as exact stored Binary under the same explicit
`--collections` option; see [stored attachment data](STORED_ATTACHMENT_DATA.md).
It retains the encoded envelope without implicit decoding. Bare attachment
fields and matching wildcards remain unsupported. Ordinary scalar queries and
counts do not load attachment records. Selected indexes on a collection-reading
parent are rejected, with no sequential fallback. All native/query/output budgets
and unchanged/offline-source requirements continue to apply.

The [independent comparison](testing/compare_attachment_queries.py) uses the
existing original `AttachmentFixture` corpus and read-only Jackcess 5.0.4 metadata.
It checks all five individual members, paired/full/reordered projection alignment,
empty parents, aggregates, displayed-child filtering, scalar-parent membership,
stored payload bytes and wildcard refusal and unchanged hashes across all three
qualified families. These are stored-record/explicit-model comparisons, not native
Access SQL, empty-set/negative-criteria semantics or Windows qualification.
The script also accepts `--collation database` to compose stored child policies
with the shared native resolver. The [24 October replay](testing/results/stored-attachment-queries-2026-10-24.json)
passes 198 model controls per policy (396 total) against fresh original
V2007/V2016/V2019 files, including aligned/mixed/joined membership,
correlated reads, crosstabs, decoded payloads and rejection checks. Synthetic
legacy/2010/unknown metadata controls separately distinguish field routing from
using one database-wide policy.

Microsoft's [attachment overview](https://support.microsoft.com/en-us/access/attach-files-and-graphics-to-the-records-in-your-database)
guides the multi-file/inert-content boundary; exact observed storage comes from
[the existing fixture evidence](FIXTURES.md). This feature adds no dependencies,
lookup/VBA execution, new file writes or extraction destination policy.

### Explicit decoded attachment values

```sh
tablecraft query run 'SELECT ID, Files.FileName AS Name, Files.FileData AS Data FROM Collections' --database copy.accdb --collections --decoded-attachments
```

`--decoded-attachments` requires `--collections`; library hosts explicitly call
`NativeSource::with_decoded_attachments()`, which also enables collection metadata.
This supplies a **TableCraft decoded Binary view**, not a claim about native
Access SQL's binary representation. Ordinary `--collections` returns the exact
stored encoded envelope; merely opening, binding or requesting metadata never
decodes it.
Microsoft describes FileData as reserved internal binary attachment storage in
[the DAO attachment guide](https://learn.microsoft.com/en-us/office/vba/access/concepts/data-access-objects/work-with-attachments-in-dao);
raw native SQL/DAO Value versus decoded-file semantics require Windows comparisons.

Under this explicit decoder option, FileData references decode complete checked
attachment collections before
visiting any parent rows. The existing bounded decoder verifies native wrappers,
lengths and zlib/checksums, retains original file bytes and returns NULL for stored
NULL. A valid zero-byte file becomes empty Binary (`0x` in CLI output), separately
from NULL or an empty malformed wrapper. Metadata and data remain aligned to the
same child; empty parents retain the existing NULL extension. Stored field/wrapper
type labels remain separate, and no content is opened, extracted or executed.

When FileData is requested, every selected member of that collection uses the
same complete full attachment snapshot, including metadata selected before
FileData. Enabling the decoder flag alone does not load unrequested payloads.
An existing complete full snapshot can also satisfy later metadata requests.

The complete decoded cache is separate from the full encoded snapshot and is
published only after every child succeeds. A later corrupt payload fails even
under TOP or a filter that would omit it; errors publish no CLI rows. Failed
decoding does not invalidate an already cached metadata projection. Shared native budgets
charge buffers, decoding/copy work and cancellation checkpoints; query/output
budgets still apply separately. Native decoding retains a 2 MiB encoded default
with an explicit [32 MiB encoded allowance](LARGER_ENCODED_VALUES.md),
1 MiB default file data and a 4 KiB header. Explicit
[file allowances](LARGER_DECODED_ATTACHMENTS.md) can raise decoded file bytes to
16 MiB; they do not raise encoded storage or cumulative pools. Changing an
allowance clears decoded caches so a lower policy cannot reuse oversized values.
Separate query Text and CLI output limits remain; see [larger payloads](LARGE_PAYLOADS.md). Selected parent indexes remain
unsupported without fallback. Binary comparison, ordering and arbitrary Binary
function overloads are not introduced by the projection/Count/NULL-test profile.
Hidden parent criteria retain their existing independent membership/negation
policy, including the documented empty-set behavior; they are not ordinary
flattened child predicates. Unprojected siblings of a displayed/grouped
attachment member now filter/sort the same aligned child, including a hidden
FileData reference under the decoder opt-in. A wholly hidden independent
collection can instead use explicit parent membership beside displayed members. See
[collection identities and limits](MULTIVALUE_QUERIES.md#unprojected-members-of-displayed-collections).
Scalar-only joined parent queries can now combine hidden metadata/data membership
atoms without child multiplication under the explicit
[joined-parent policy](MULTIVALUE_QUERIES.md#parent-membership-over-joins).
Decoded data remains separately opt-in and fully checked before visiting parents;
joined TOP/Boolean criteria do not conceal a later corrupt payload.
[Crosstab WHERE membership](CROSSTABS.md#collection-membership-criteria) now uses
the same explicit source views, preserving dynamic pre-WHERE heading discovery
and fixed-IN filtering order. Native collection opt-in, decoder limits and
selected-index refusals remain in force.
[Subqueries in membership criteria](MULTIVALUE_QUERIES.md#subqueries-inside-membership-criteria)
also retain scoped collection items, complete snapshot caches and buffered late
failures. Decoded Binary remains opt-in; no Binary comparison overload or native
Access SQL qualification follows from these NULL-test controls.
[ON-only member references](MULTIVALUE_QUERIES.md#collection-members-in-join-conditions)
now retain child expansion without requiring output fields, including sibling
filters and independent WHERE membership on another collection/alias. Native
collection join admissibility/cardinality still needs actual Access comparisons.
[Multi-member WHERE criteria](MULTIVALUE_QUERIES.md#several-hidden-members-in-one-atom)
now align fields of one child and fold independent collection/alias products,
including explicit decoded-data/metadata NULL predicates. Complete cache and
decoder opt-in/error boundaries remain unchanged; native SQL qualification is
separate from these stored-record models.
[Mixed queries](MULTIVALUE_QUERIES.md#displayed-children-with-independent-parent-membership)
retain displayed child rows once per matched parent/pair, without multiplying
them by hidden matching items. The decoder opt-in and eager validation still apply.

The original independent attachment recipe's public `getFileData()` supplies
expected original bytes, including compressed maximum-size, empty, Unicode and
signature samples. The comparison also tests late checksum corruption on copies
and unchanged metadata/source bytes. This is independent logical-decoder evidence;
native SQL representation, collection semantics and Windows qualification remain
unrun. No engine dependency on query execution or new registry library is added.

Saved UNION metadata now has an explicit [read adapter](NATIVE_SAVED_UNIONS.md),
including labelled SQL fragments, flags, parameters, ordering and named dependencies.
Native Access execution and wider fragment/type behavior remain unqualified.

[Native saved crosstabs](NATIVE_SAVED_CROSSTABS.md) now reconstruct and explicitly
execute through the existing grouped/pivot executor. Their named/derived schemas,
TOP 0/owner execution and wider native compatibility remain unsupported. Positive
stored TOP counts/percentages now limit completed groups through the shared executor.

Explicit [legacy DATE numeric conversion](DATE_NUMERIC.md) now applies to stored
Date/Time columns after native carrier/schema validation. No source mutation,
implicit storage coercion, date rounding or engine-specific evaluation is added.

Native legacy DATE columns may now feed IIf/Switch conditions and Choose indices;
Text selector slots use the same explicit number profile as scalar execution.
[Selector scope](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
keeps source carriers, payload selection and native qualification separate.

## Logical numeric and Text operands

Native scalar fields now feed the shared [logical coercion profile](INTEGER_LOGIC.md):
float/fixed/legacy DATE operands use checked half-even Long or Int64-selected
LongLong targets; numeric Text operators need an explicit number format. Final
read predicates use numeric zero/nonzero or contextual CBool Text conversion.
This does not add SQL text-index pushdown, infer regional policy from LCIDs or
qualify native result subtypes. Original Jet 4/ACE 2007/ACE 16-family ID/DATE getter
controls remain unchanged by the 2,658-control independent comparison recipe.

## Explicit larger collection-table loads

With `--multivalues` or `--collections`, `--native-rows` now supplies the same
explicit snapshot ceiling to complete parent-key and child-table loads. Native
budgets stay cumulative and collection schemas/references still validate before
cache publication. Defaults and the 100-value-per-parent multivalue policy remain
unchanged. Metadata-only binding and selected-index refusal remain. See [larger
collection limits and evidence](LARGER_NATIVE_COLLECTIONS.md).

Explicit decoded attachment columns now reuse one budget-bound inflater session.
The workspace is charged once per decoded column; per-file body/type/copy work and
allocations remain cumulative, with complete cache publication unchanged. The
larger ACE 16/ACE 2007 corpus includes exact 20,000-file decoded reads/queries;
see [scope](LARGER_NATIVE_COLLECTIONS.md#larger-attachments-and-stored-histories).
This does not qualify native FileData SQL representation or native history SQL.
The separate [stored-history engineering view](STORED_HISTORY_QUERIES.md) now
supports bounded named-member reads without chronological/latest-value inference.


## Attachment presence criteria

Explicit `--collections` queries now support bare attachment `IS NULL` / `IS NOT
NULL` WHERE criteria through [typed collection emptiness](COLLECTION_PRESENCE.md).
The predicate tests membership length, not nullable filenames, and uses complete
metadata-only snapshots without reading FileData. Parent counts, displayed
children and qualified joins retain their existing cardinality contracts.
Function-wrapped/projection/correlated-container tests remain unsupported; native
SQL semantics and the wider advanced-field acceptance work remain unqualified.
