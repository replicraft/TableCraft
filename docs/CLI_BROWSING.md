# Native table browsing

The `tables` and `rows` CLI commands provide read-only discovery and paged scalar
contents without requiring SQL. Both consume the reusable
`tablecraft-access::NativeBrowser`; the CLI owns only argument handling, explicit
file opening and display formatting. Backend table
inventory filters shared object descriptors; the ACE engine owns physical catalog,
slot and value decoding. No SQL strings, expression evaluation, external providers
or active database content are involved. ACE 16 and ACE 2007/ACE 12 `.accdb`, plus Jet 4 `.mdb`, are the
immediate read/query priorities. Other accepted reader profiles retain their existing limits.

```sh
tablecraft tables example.accdb
tablecraft tables example.accdb --details
tablecraft rows example.accdb "Customers" --limit 25
tablecraft rows example.mdb "Customers" --limit 25 --offset 25
```

When running from source, replace `tablecraft` with
`cargo run --locked -p tablecraft-cli --`.

`tables` prints `Name` and `Kind` headers and every table-category catalog entry in catalog
order, including hidden/system and linked-table metadata. It does not open those
tables, classify their complete readability or contact linked sources. Use
`inspect PATH --catalog` for native origin/type/flag details and other objects;
`inspect PATH --table NAME` provides the selected table's column metadata.

`Kind` is `System` when either native system bit (`0x80000000` or `0x02`)
is set, otherwise `User`. Hidden status alone (`0x01`) is not a system marker;
names are not used for classification. `User` does not prove who created a table.
The engine interprets these flags and the shared inventory exposes `is_system()`.
The flag interpretation follows the reviewed Jackcess `DatabaseImpl`
`SYSTEM_OBJECT_FLAGS`/`isSystemObject` reference recorded in [fixtures](FIXTURES.md).

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

`rows` prints column names followed by live scalar rows in physical traversal
order. The default displayed limit is 100; `--limit` accepts 1–16,384 and
`--offset` accepts 0–4,294,967,295. Offsets count live rows, not page slots or
bookmarks. Empty tables and offsets beyond the end produce column headers only.
Names are passed directly to the reader, so brackets and quote characters do not
become SQL syntax. Normal reader name-resolution rules apply.

The display uses comfy-table Unicode borders when stdout is a terminal and
escaped TSV when redirected. Global `--format table` and `--format tsv` override
this selection for `tables`, `rows` and `query run`/`query eval`. Other inspect,
SQL and edit reports retain their existing formats. Tables wrap at a fixed
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

## Explicit stored calculations

`rows COPY.accdb TABLE --stored-calculations` permits the qualified ACE 16-family
stored Boolean, Int32, Double, Currency, legacy DateTime and Short Text calculation
wrappers alongside ordinary scalar columns. Both display formats append
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

Larger total tables are accepted within existing file/page/allocation-map limits;
the old 16,384-row complete-snapshot ceiling is not applied to total rows here.
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
only its stated stored-result profile. Linked tables, encrypted files and
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
