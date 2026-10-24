# Native table browsing

The `tables` and `rows` CLI commands provide read-only discovery and paged scalar
contents without requiring SQL. Both consume the reusable
`tablecraft-access::NativeBrowser`; the CLI owns only argument handling, explicit
file opening and display formatting. Backend table
inventory filters shared object descriptors; the ACE engine owns physical catalog,
slot and value decoding. No SQL strings, expression evaluation, external providers
or active database content are involved. ACE 16 and ACE 2007/ACE 12 `.accdb`, plus Jet 4 `.mdb`, are the
immediate read/query priorities. Other accepted reader profiles retain their existing limits.

Qualified Agile-encrypted ACCDB sources accept global `--password` / `-p` before
or after the command. For example, `tablecraft rows example.accdb Customers -p
'your-password'`. Unlocking, discovery and row reads share the native allowance;
wrong/missing credentials publish no partial output or credential diagnostics.
Raw header/encryption inspection needs no password. Other encryption schemes and
encrypted UPDATE copies remain unsupported; see [encryption scope](NATIVE_ENCRYPTION.md).

```sh
tablecraft tables example.accdb
tablecraft tables example.accdb --details
tablecraft rows example.accdb "Customers" --limit 25
tablecraft rows example.mdb "Customers" --limit 25 --offset 25
```

When running from source, replace `tablecraft` with
`cargo run --locked -p tablecraft-cli --`.

`tables` prints `Name` and `Kind` headers and user table-category catalog entries in catalog
order, including hidden user and linked-table metadata. It does not open those
tables, classify their complete readability or contact linked sources. Use
`inspect PATH --catalog` for native origin/type/flag details and other objects;
`inspect PATH --table NAME` provides the selected table's column metadata.

`Kind` is `System` when either native system bit (`0x80000000` or `0x02`)
is set, otherwise `User`. Hidden status alone (`0x01`) is not a system marker;
names are not used for classification. `User` does not prove who created a table.
The engine interprets these flags and the shared inventory exposes `is_system()`.
The flag interpretation follows the reviewed Jackcess `DatabaseImpl`
`SYSTEM_OBJECT_FLAGS`/`isSystemObject` reference recorded in [fixtures](FIXTURES.md).

System tables are hidden by default. `tables --show-system` includes system-marked
tables; hidden user tables and user tables whose names happen to begin with MSys
remain visible in either mode. Filtering uses the shared native flag classifier,
with no additional table/property/provider reads. `inspect --catalog --show-system`
applies the same table policy; other object kinds always remain visible, even
when system-marked. The inspect flag requires `--catalog`.

`tables --details` adds declared `Storage` and explicit row-inspection status.
Local, linked non-ODBC and linked ODBC classifications come from the engine's
captured catalog type; non-ODBC links are not assumed to target another Access
file. No connection string/provider target is read, opened or displayed. Local
entries say `Not inspected`; classification is not a row-readability claim.
Default inventory output includes both Name and Kind.

`NativeBrowser::describe` supplies a reusable `TableDescription` for one local
table. Its nonempty-schema scalar support summary and unsupported-column iterator
use `NativeColumn::scalar_type`, the same engine classifier used by scalar reads.
The CLI's existing `inspect --table` output now adds `Scalar columns` and
`Unsupported columns` summary lines before the original column metadata table.
Unsupported columns retain their names/native codes/widths/flags. Linked schemas,
malformed metadata, limits and cancellation still return typed errors; failures
are not converted into an unsupported-column summary.

Description does not read user record pages or qualify row contents, historical
row layouts, long values, constraints, collation or whole-table resource needs.
A supported-carrier description can precede a failed row read on corrupt data.
The shared `NativeBrowser` keeps inspection within the caller's cumulative budget
and checks cancellation again before publishing the description.

An explicit [streaming mode](STREAMING_ROWS.md) now continues through one scalar
cursor and publishes TSV batches until checked EOF. It supports ordinary scalar
column subsets and cumulative controls beyond snapshot/buffered-output caps.
Late failures may leave prior output; the following default page contract remains
buffered.

`rows` prints column names followed by live scalar rows in physical traversal
order. The default displayed limit is 100; `--limit` accepts 1–16,384 and
`--offset` accepts 0–4,294,967,295. Offsets count live rows, not page slots or
bookmarks. Empty tables and offsets beyond the end produce column headers only.
Names are passed directly to the reader, so brackets and quote characters do not
become SQL syntax. Normal reader name-resolution rules apply.

The display uses comfy-table Unicode borders when stdout is a terminal and
TSV encoded by `csv::WriterBuilder` when redirected. Global `--format table` and `--format tsv` override
this selection for `tables`, `rows`, `inspect` and `query run`/`query eval`.
SQL reconstruction/normalization and edit reports retain their existing formats. Tables wrap at a fixed
120-column target (wide schemas can exceed it); values are not truncated.
Both formats retain the existing scalar policy: quoted text,
`NULL`, Boolean values, exact Decimal/Currency formatting, hexadecimal binary,
GUIDs and qualified date values. Border expansion is checked against the same
1 MiB output ceiling before stdout publication. comfy-table owns infallible
internal allocations; a conservative 32 MiB rendered-layout estimate rejects
excessive padding/wrapping before calling it (use TSV for rejected large results).
This is a layout estimate, not a total heap quota, and can reject results whose
actual rendering would be smaller. Allocation failure inside that dependency
cannot be translated into a typed error. Embedded tabs/newlines/control characters are
escaped. Decimal decoding and scale preservation do not require decimal query
arithmetic. Use `query run --database PATH 'SELECT ... ORDER BY ...'` for explicit
sorting, filtering, joins or aggregates.

TSV uses a tab delimiter, LF record endings and standard CSV quote doubling.
Fields carry the escaped display representations above, rather than raw database
values: a TSV reader returns `NULL` for stored NULL, `"NULL"` for text containing
NULL, and `""` for empty text. Text's display quotes are themselves CSV-quoted in
the transport, so its bytes differ from the older hand-written output. Tabs,
newlines and controls inside values remain visible escapes. Parse with a TSV/CSV
reader rather than splitting the raw bytes on tabs. For example:

```python
import csv
import subprocess

text = subprocess.check_output(
    ["tablecraft", "--format", "tsv", "rows", "example.accdb", "Customers"],
    text=True,
)
rows = list(csv.reader(text.splitlines(), delimiter="\t"))
```

Inspection retains summary pairs and detail tables with different field counts;
read records flexibly. The default final 1 MiB output ceiling includes CSV quote expansion,
and formatting completes before stdout receives any bytes.

## Inspection presentation and object names

`inspect` uses compact comfy-table borders on terminals, including header-only
inspection. Summary properties have explicit Property/Value headings; columns,
indexes, properties, fragments, relationships and check reports have separate
tabular sections. Section boundaries are supplied by the report producers rather
than inferred from table names, text contents or changes in column counts.
`--format table` forces this presentation and `--format tsv` CSV-encodes the
escaped display cells, including summary lines and existing headers. The final 1 MiB
ceiling applies to all rendered sections together, before stdout publication.

```sh
tablecraft inspect example.accdb
tablecraft inspect example.accdb --table tblContacts
tablecraft inspect example.accdb --catalog --show-system
tablecraft tables example.accdb --show-system
```

Native names stay exact: in the registered ACE 2007 example the actual name is
`tblContacts`, and its decoded property groups contain no alternate Caption.
We do not strip `tbl`, split CamelCase or use Description as an inferred name.
Access field/control captions are separate presentation metadata; custom table
properties or form labels do not establish a universal alternate table name.
Microsoft documents [TableDef.Name](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/tabledef-name-property-dao)
and lists [Access-defined DAO properties](https://learn.microsoft.com/en-us/office/vba/access/concepts/data-access-objects/set-properties-of-data-access-objects-in-visual-basic)
with Caption under Field. No automatic table alias mapping is implemented.

## Explicit stored calculations

`rows COPY.accdb TABLE --stored-calculations` permits the qualified ACE 16-family
stored Boolean, Int32, Double, Currency, legacy DateTime, [Decimal](CACHED_DECIMAL_CALCULATIONS.md)
and Short Text/[Long Text](CACHED_MEMO_CALCULATIONS.md) calculation
storage alongside ordinary scalar columns. Both display formats append
` [cached]` to calculated column headers. These are stored results, potentially
stale or inconsistent with other columns; no formula, VBA or active content runs.
For example a cache of 42 is displayed unchanged even when its formula and current
input would yield 2. This opt-in is browsing only: ordinary `rows` and query Source
eligibility remain unchanged. Calculated ACE 2007 columns remain unqualified.

The reusable API is `NativeBrowser::stored_page`, returning the distinct engine
`StoredTablePage` from `read_stored_table_page`. It retains original schema and
inert declarations through `features()`, qualified carriers, physical rows,
request/end information and `is_stored_calculation(column)` (None out of range).
Every column must qualify before record traversal. Calculations require the same
matching typed ResultType, nonempty Expression and exact layout gates as the
existing single-column stored inspector, even for empty pages or huge offsets.
Unknown/complex/unsupported wrappers fail; they never become NULL or hidden fields.
The physical page walker, limits and cancellation checks are shared with scalar
pages. Selected values are checked; skipped/later payloads remain unchecked.

[Cached-page comparison](testing/compare_cached_pages.py) reuses four original
inert V2016/V2019 fixtures with seven result encodings (including both Text
encodings). Thirty-eight independent controls cover pages, EOF/empty results,
ordinary query/read refusals, pretty output and corrupt-copy atomic failures.
DATE display is checked from independently retained serial bits using an explicit
nanosecond calendar model, avoiding the Java calendar carrier's millisecond
rounding. Fixtures include deliberately stale results; formulas stay disabled.
See [provenance](FIXTURES.md#cached-calculation-page-regression).

## Explicit multivalue cells

`rows COPY.accdb TABLE --multivalues` displays qualified ACE 2007/ACE 16 multivalue
collections beside ordinary scalar cells. Collection headers append ` [multivalue]`.
An assigned empty collection displays `[]`, an absent parent key displays `NULL`,
a stored NULL element displays `[NULL]`, and a text element remains quoted
(e.g. `["NULL"]`). Duplicates and flat-table order are preserved. These are stored
bound values; lookup RowSource/display text is never executed or substituted.
This does not implement Access SQL `.Value` expansion or scalar collection coercion.

`--multivalues --stored-calculations` also permits qualified cached scalar results
in the same page, with the existing `[cached]` header and no freshness guarantee.
Either opt-in alone still rejects unsupported columns; no columns are omitted.
Attachments/history and unqualified complex templates fail explicitly in this
profile. They retain separate stored-content APIs and explicit inspection views. Current complex
key/collection qualification includes ACE 2007 and both ACE 16 families.
Large Number collection carriers remain ACE 16-only. Typed attachment/history
read APIs separately qualify ACE 2007/ACE 16, but they are not rendered by this
multivalue page profile. See [legacy stored-content evidence](FIXTURES.md#ace-2007-stored-attachment-and-history-inputs).

The reusable `NativeBrowser::multivalue_page` returns `MultivalueTablePage` from
`read_multivalue_table_page`. `schema()` retains native types; `cell(row, column)`
returns an explicit `MultivaluePageValue::Scalar` or `Collection`, or None for an
out-of-range coordinate. Collections expose their scoped key and checked stored
values; no core Value variant is repurposed for a collection. `collection_column`
retains the original typed collection/child metadata. Inert `features()` metadata
is present when cached-calculation opt-in is selected; otherwise properties are
not read. The request, row count and EOF metadata remain explicit.

This operation validates every column and materializes complete parent-key and
flat-child snapshots for each collection, even for empty/out-of-range requests.
Existing positive/unique key/identity checks, orphan rejection, 100 values per
parent and 16,384-row complete-snapshot ceilings remain. Multiple collections
share cumulative budgets and can exceed the CLI limits. Stored-column metadata
must match the page's original native layout, and selected keys must match their
validated full parent snapshots. This checks observed consistency on the required
stable/offline source, without providing concurrent-read coherence.
Ordinary scalar and cached payloads outside the selected page remain unchecked.
Repeated pages currently repeat complete collection validation; this is not lazy
child fetching or a whole-table scalar integrity proof.

The [independent runner](testing/compare_multivalue_pages.py) checks 46 controls
across existing eight-carrier originals and fresh mixed scalar/collection/cache
fixtures for both ACE 16 families. It covers pages, NULL/empty/duplicate values,
stale results, complete-snapshot refusals and atomic corrupt-copy rejection.
NaN display is `NaN` and does not encode payload bits; borrowed core values retain
those original bits. [Fixture provenance](FIXTURES.md#mixed-multivalue-page-inputs)
records immutable sources and qualification scope. The optional `--matrix2007`
adds eleven ACE 2007 controls while retaining the original 46-control matrix;
[ACE 2007 evidence](FIXTURES.md#ace-2007-complex-key-and-multivalue-inputs) also
checks raw links and exact collection APIs.

## Current limits

These commands require a stable/offline file; shared-file locking/coherent
acquisition remains outside this CLI path. The source is opened read-only.
`rows` reads a bounded page rather than materializing a complete table. It walks
allocated pages/live root slots up to the requested range, validates visited page
ownership/slot geometry and decodes only the selected records, including their
required relocations and long values. Deleted roots do not consume offsets.
Skipped payloads and later pages are not decoded or qualified; a successful page
is not a whole-table integrity check. At an enumerated end, the live-root count
must match the declared count. A full page may stop before establishing EOF.

Larger total tables are accepted within existing file/physical-page/resource limits.
Paging validates all allocation metadata, then enumerates addresses incrementally;
it does not construct the 4,096-address list used by full snapshots. The old
16,384-row complete-snapshot ceiling is not applied to total rows here.
The page size remains capped at 16,384. Repeated offset requests rescan earlier
root slots, so deep pagination still costs page I/O/work. Physical order can change
between file copies; paging is not a persistent cursor or SQL ordering guarantee.
The complete-snapshot query path retains its existing limits.

The current CLI native budget is 16 MiB cumulative read/allocation bytes,
16,384 cumulative metadata/decoded rows and 1,000,000 steps, in addition to engine
structural ceilings. The common output buffer is limited to 1 MiB. A large valid
table or deep offset can exceed these limits even when requesting one displayed row.
Skipped live roots consume steps, not the decoded-row allowance.

Every column must have a qualified layout for the selected operation. Ordinary
scalar browsing rejects calculations; the explicit cached option above enables
only its stated stored-result profile; the multivalue option above enables
explicit collection cells under its separate snapshot limits. Linked tables, encrypted files and
unsupported calculated/complex/attachment layouts fail explicitly rather than
silently hiding columns or replacing values with NULL. Their specialized inert
inspection APIs remain separate. All requested decoding/formatting must finish before
stdout is published; failures exit 2 with no partial table or header.

## Evidence

Four CLI integration tests cover authored Jet 4 bytes, default/page/end-of-file
behavior, case-insensitive names, maximum offsets, unusual table names, unknown
and unsupported columns, malformed files, invalid arguments, registered-sample
output agreement and unchanged source bytes.

[The independent comparison](testing/compare_browsing.py) reuses the original
[ScalarFixture recipe](testing/ScalarFixture.java) and reviewed Jackcess artifact.
Fresh original V2000, V2016 and V2019 files each verify complete table inventory,
five page controls and a sorted SELECT against independent catalog/schema/row
oracles. Scalars include Unicode text, long values, binary/GUID data, NULLs,
Boolean, signed numeric extrema, Currency, 28-digit Decimal and legacy dates.
[Fixture provenance](FIXTURES.md#native-cli-browsing-inputs) records hashes and
reproduction. This is independent reader evidence; native Microsoft Access,
Windows and concurrent-file behavior remain unqualified.


Seven engine tests additionally cover unvisited-page I/O counts, lower allocation/
row charges, an 18,000-row table under a small decoded-row budget, deleted and
relocated roots, empty/end ranges, malformed selected rows/visited pages, partial
validation boundaries, count mismatches, unsupported columns, final cancellation
and resource exhaustion. A fixed-seed 128-case property compares pages with an
independent sequence model. Two backend tests exercise inventory filtering and
paging without the CLI, cumulative budgets and cancellation.

[The larger-table comparison](testing/compare_browsing_pages.py) uses the original
[BrowsingFixture recipe](testing/BrowsingFixture.java): each of three original
families has 20,050 rows and ten page controls, including offsets beyond 16,384,
Unicode/escaped text, 28-digit Decimal extrema, NULLs and long text. Original bytes
must remain unchanged. This comparison does not expand complete query snapshots,
qualify unvisited payloads or establish native Access conformance.


Three additional backend tests cover local/non-ODBC/ODBC storage declarations,
linked-definition refusal, metadata-only I/O despite corrupt data pages,
unsupported column reporting, typed failures and final cancellation. A CLI test
covers detail/summary output and unsupported-column preservation. Independent
[metadata comparisons](testing/compare_browsing_metadata.py) use fresh original
Jet 4 and both ACE 16-family databases with local rows and an unresolved link.
Inventories, column metadata and explicit link refusals agree with independently
read native declarations; originals remain unchanged.


## Stored attachments and Memo history

```sh
tablecraft inspect example.accdb --table Customers --attachments Files
tablecraft inspect example.accdb --table Customers --version-history Notes
```

These explicit views use `NativeBrowser::attachment_column` and
`NativeBrowser::version_history_column`, returning the engine's checked
`AttachmentColumn` and `VersionHistoryColumn`. The Access backend re-exports
these types and their borrowed `StoredAttachment`/`StoredVersion` records for
other hosts. The methods share the browser's cumulative controls and perform no
path opening, lookup/formula execution or implicit payload decoding.

The qualified profiles are ACE 2007 and both ACE 16 families, with the engine's
strict conceptual/child-field and scoped-reference checks. History takes the
visible append-only Memo name, resolving its hidden complex field internally.
Both views report every parent ordinal/key/count, including empty memberships,
then child details in stored flat-snapshot order. NULL keys and assigned keys with
zero children remain distinguishable. Ordinals/child identities are snapshot-local.

Attachment details show nullable URL/name/type, encoded byte length, timestamp
and raw flags. A present empty encoded value prints 0; NULL prints NULL. Encoded
length is not decoded file length or wrapper-validity evidence. Files, URLs and
metadata strings are never opened or used as paths. Invalid payload wrappers can
still have readable metadata; decoding remains an explicit separate library API.

History shows each stored value and timestamp without selecting a current/latest
value or sorting newest first. NULL and empty text remain distinct. Both reports
show the exact 16-digit hexadecimal legacy DATE bits beside the normal rounded
calendar display, retaining signed zero, aliases and sub-nanosecond distinctions.
The standard escaped-display/CSV TSV policy and compact bordered sections apply.

These operations validate complete parent/child snapshots under the existing
whole-table, cumulative resource and 1 MiB output limits; they are not paged.
Other inspection modes conflict with these flags and `--table` is required.
Failure publishes no stdout. Use a stable/offline source; no concurrent-file
snapshot or native Access qualification is implied. Ordinary `rows`/query source
profiles remain separate and still reject attachment/history scalar coercion.

The original attachment/history comparison runners now also check CLI membership,
nullable metadata, stored order, exact DATE bits, encoded lengths, and buffered
refusal against their independent Jackcess results, using standard Python CSV
parsing. No additional third-party fixture bytes are introduced.


## Mixed scalar and stored-collection pages

```sh
tablecraft rows example.accdb Customers --collections --limit 25
tablecraft rows example.accdb Customers --collections --stored-calculations
```

`--collections` permits all qualified stored multivalue, attachment and hidden
Memo-history columns beside ordinary scalars. It conflicts with `--multivalues`;
the existing narrower mode keeps its prior behavior. `--stored-calculations`
remains separately required for qualified cached results. Unsupported columns
still fail the complete request; none are omitted or coerced to NULL.

The engine's `read_stored_collection_table_page` and shared
`NativeBrowser::collection_page` return `StoredCollectionTablePage` with native
schema, optional inert feature declarations, checked request/EOF information and
explicit scalar/collection cells. `collection_column` returns a typed
`StoredCollectionColumn`; a collection cell exposes its scoped key and checked
`items()` iterator of `StoredCollectionItem::Value`, `Attachment` or `Version`.
No collection enters core `Value` or acquires SQL flattening semantics.

The CLI labels headers `[multivalue]`, `[attachments]` and `[history: MEMO]`.
The original visible Memo/current value and hidden native history field both
remain present, with the latter explicitly labelled by its visible Memo binding.
Multivalue lists retain existing scalar formatting. Attachment list entries show
scoped identity, nullable URL/name/type, encoded byte length, exact timestamp bits
and flags. Version entries show scoped identity, nullable text and exact timestamp
bits. Metadata strings remain escaped text; no files, URLs or wrappers are opened,
extracted or decoded. History is not sorted newest-first and no latest value is
synthesized. `NULL` key, assigned empty `[]`, NULL elements and nullable metadata
remain distinct. CSV transport quoting applies to these display cells too.

The shared page decoder validates complete parent-key and child snapshots for
every collection before returning any page, even empty/out-of-range requests.
Existing whole-snapshot limits, reference checks, cumulative budget/cancellation
and stable/offline-source requirements remain. Selected raw keys must match their
checked parent snapshots and owner layouts. Unselected ordinary scalar payloads
remain unchecked. Repeated requests repeat complete collection validation; this
is not lazy collection loading or concurrent-file coherence. Final output retains
the 1 MiB cap and complete buffering.

The original [mixed-page oracle](testing/CollectionPageOracle.java) uses inert
Jackcess getters and raw stored history order on nine existing ACE 2007/16-family
inputs. [Comparison](testing/compare_collection_pages.py) checks 72 page, offset,
empty and bordered cases with standard CSV parsing and unchanged input bytes.
The existing multivalue/cache matrix additionally checks the general mode,
retaining all old controls; attachment/history corrupt-copy runners verify that
empty requests cannot bypass strict type/reference checks. These remain
independent evidence, not native Microsoft Access qualification.


## Inventory counts and all-table pages

```sh
tablecraft tables example.accdb --count
tablecraft tables example.mdb --count --details --show-system
tablecraft rows example.accdb --all
tablecraft rows example.mdb --all --limit 250 --offset 100
```

`tables --count` adds `Row Count` immediately after `Kind`, preserving the ordinary
inventory when absent. It reads local table-definition metadata through the shared
NativeBrowser description API; it does not decode user records or validate the
stored count against live roots. Linked and unknown storage show `N/A`; external
providers remain unopened. `--details` can be combined with counts and retains its
storage/row-inspection columns. Malformed/unqualified selected metadata returns
an error with no success output. System visibility still uses `--show-system`.

`rows PATH --all` replaces the positional table argument. It visits user table
entries in catalog order and labels each section with `Table` and the escaped
native name, followed by that table's header/rows. Empty tables retain their column
headers. Linked/unknown entries get a Status section saying the source was not
opened. System tables remain excluded. Default page size is 100 per local table;
`--limit` and `--offset` apply independently to each table, not to the combined
result. This displays pages from all tables, not an unbounded full-database dump.
Existing `--stored-calculations`, `--multivalues` and `--collections` profiles and
conflicts also apply in this mode. Unsupported local fields fail explicitly;
no local table is silently skipped. Table names are passed as data, never SQL.

One shared NativeBrowser owns the file and cumulative native budget across all
sections. All rendering finishes before stdout, with a cumulative 1 MiB output
ceiling. A later local read/format failure publishes no earlier sections. Terminal
sections use separate comfy-table summary/detail borders. TSV sections use the
same `csv` tab-delimited encoding, a `Table` identity record, per-table column
header/data records and an empty separator record; column widths can differ
between sections. These remain escaped display values rather than raw exports.

CLI integration tests cover stored/zero/unavailable counts, flag conflicts,
per-table offsets, empty sections, linked status, late failure atomicity, both
presentation modes and original-byte preservation. On the registered ACE 2007
example, counts and two-row pages from all eleven user tables completed locally.
Native Access comparison and concurrent-file qualification remain unrun.

## Larger native read sources

Native inspect/tables/rows/query reads accept qualified sources up to 2 GiB without
implicitly capturing a whole image. Synthetic CLI cases cover all four current
read profiles in 128 MiB sparse files, including a table data page at the end,
row offsets and bounded SELECT results. Metadata/read/allocation/row/output budgets
remain unchanged. Native complete-image capture, offline owned transactions and
editing retain their separate 64 MiB image cap; oversized update copies fail before
output creation. Stable/offline ownership is still the caller's responsibility.

Paging now uses a bounded address cursor after full map validation; its first
rows can come from tables owning more than 4,096 pages. Physical page-read and
byte/work ceilings still bound deep offsets. Sequential query snapshots now load
through bounded cursor batches while retaining complete materialization and row
limits; indexed reads retain materialized maps. Cumulative budgets can reject larger tables inside an
accepted file. A larger file-size envelope is not unrestricted large-table,
streaming, native Access or live-file compatibility. See
[large-source evidence and reproduction](testing/LARGE_FILE_READS.md).

Only native `query run` has the explicit `--native-bytes BYTES` override; it
requires `--database` and replaces the cumulative native byte allowance within
1..=268435456. It retains the 16 MiB default and all separate row/work/query/output
limits. It does not apply to `rows`, `tables`, `inspect` or writes. See
[query controls](NATIVE_QUERIES.md#explicit-supported-boundary) for scope/examples.

## Bounded paging across large owned maps

Independent Jet 4/ACE 2007/2016/2019 fixtures each hold 6,000 ordinary rows on
6,000 owned pages. `rows` returns small windows at tested offsets through 1,000,
matching independent getters; `tables --count` reports 6,000. Map metadata remains
fully validated before selected data, including later invalid bitmap references.
No complete address vector is retained for this paging path. Repeated offsets
still rescan earlier roots. Sequential queries now build bounded complete snapshots
through cursor batches; indexed reads and writes retain materialized ownership
checks. Library hosts can use [source-bound scalar
cursors](testing/RESUMABLE_TABLE_READS.md); CLI offset requests and streaming queries
remain separate. [Evidence and benchmarks](testing/ALLOCATION_MAP_PAGING.md) describe
resource ceilings, original fixtures and qualification limits.

Explicit `rows --stream` now uses this cursor path; [progressive row output](STREAMING_ROWS.md)
retains its own publication and cumulative-control contract. SQL execution remains
bounded and buffered.


### Explicit larger query output

Ordinary browsing retains the limits above. `query run --output-bytes` separately
allows up to 16 MiB of escaped display and final TSV/table output, including CSV
quote expansion. `--text-bytes` controls individual query Text values up to 1 MiB.
Defaults remain unchanged; bordered layout limits may still require TSV. See
[larger payload controls and evidence](LARGE_PAYLOADS.md).
