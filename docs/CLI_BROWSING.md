# Native table browsing

The `tables` and `rows` CLI commands provide read-only discovery and paged scalar
contents without requiring SQL. Both consume the reusable
`tablecraft-access::NativeBrowser`; the CLI owns only argument handling, explicit
file opening and display formatting. Backend table
inventory filters shared object descriptors; the ACE engine owns physical catalog,
slot and value decoding. No SQL strings, expression evaluation, external providers
or active database content are involved. ACE 16 `.accdb` and Jet 4 `.mdb` are the
immediate read/query priorities. Other accepted reader profiles retain their existing limits.

```sh
tablecraft tables example.accdb
tablecraft rows example.accdb "Customers" --limit 25
tablecraft rows example.mdb "Customers" --limit 25 --offset 25
```

When running from source, replace `tablecraft` with
`cargo run --locked -p tablecraft-cli --`.

`tables` prints a `Name` header and every table-category catalog entry in catalog
order, including hidden/system and linked-table metadata. It does not open those
tables, classify their complete readability or contact linked sources. Use
`inspect PATH --catalog` for native origin/type/flag details and other objects;
`inspect PATH --table NAME` provides the selected table's column metadata.

`rows` prints column names followed by live scalar rows in physical traversal
order. The default displayed limit is 100; `--limit` accepts 1–16,384 and
`--offset` accepts 0–4,294,967,295. Offsets count live rows, not page slots or
bookmarks. Empty tables and offsets beyond the end produce column headers only.
Names are passed directly to the reader, so brackets and quote characters do not
become SQL syntax. Normal reader name-resolution rules apply.

The display uses the existing escaped tab-separated formatter: quoted text,
`NULL`, Boolean values, exact Decimal/Currency formatting, hexadecimal binary,
GUIDs and qualified date values. Embedded tabs/newlines/control characters are
escaped. Decimal decoding and scale preservation do not require decimal query
arithmetic. Use `query run --database PATH 'SELECT ... ORDER BY ...'` for explicit
sorting, filtering, joins or aggregates.

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

Every column must have a qualified scalar layout. Linked tables, encrypted files
and unsupported calculated/complex/attachment layouts fail explicitly rather than
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
