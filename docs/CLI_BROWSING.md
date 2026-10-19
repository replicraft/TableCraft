# Native table browsing

The `tables` and `rows` CLI commands provide read-only discovery and paged scalar
contents without requiring SQL. Both use the existing native reader directly;
no SQL strings, expression evaluation, external providers or active database
content are involved. ACE 16 `.accdb` and Jet 4 `.mdb` are the immediate read/query
priorities. Other accepted reader profiles retain their existing limits.

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
`rows` decodes the complete bounded scalar table before displaying a page.
Pagination limits output, not native I/O, allocation or decode work. Physical
order can change between file copies; paging is not a persistent cursor or SQL
ordering guarantee.

The current CLI native budget is 16 MiB cumulative read/allocation bytes,
16,384 cumulative catalog/table rows and 1,000,000 steps, in addition to engine
structural ceilings. The common output buffer is limited to 1 MiB. A large valid
table can exceed these limits even when requesting one displayed row.

Every column must have a qualified scalar layout. Linked tables, encrypted files
and unsupported calculated/complex/attachment layouts fail explicitly rather than
silently hiding columns or replacing values with NULL. Their specialized inert
inspection APIs remain separate. All decoding/formatting must finish before
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
