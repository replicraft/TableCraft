# Read-only native format inspection

`crates/engines/tablecraft-ace` is a working standalone native library.
`inspect_header` and `inspect_catalog` take a caller-supplied Read + Seek source,
ResourceBudget and cancellation callback. They perform no writes, file discovery,
provider activation, password attempts, linked-source access or application startup.
The library consumes core/schema/model and thiserror; no TableCraft host is needed.

`tablecraft inspect <path>` opens that explicit file read-only and buffers a small
header summary before stdout. `--catalog` adds shared descriptors and native
origins: object ID, category, native ID, parent ID, signed type, raw flags and name.
Names/headers use the shared escaped table formatter and 1 MiB logical output
ceiling. Errors exit 2 without successful partial output or file paths in diagnostics.
A catalog error publishes no header either. Output I/O/broken-pipe behavior is retained.
Use a stable/offline copy: native shared-file locking and consistent concurrent
snapshot acquisition are unimplemented. Host Read/Seek callbacks own I/O timeouts.

## Format facts and reader policy

Page/row geometry, packed references, type tags, metadata offsets, signatures and
Unicode widths have named definitions within their owning format/header/catalog
modules. Primitive widths derive from Rust integer/float types. The reader's
structural ceilings live separately in its private profile: identical numbers
such as page bytes and maximum page reads represent different contracts.
`Header::PREFIX_BYTES` exposes the exact prefix/read-byte requirement to hosts.
Synthetic/golden test bytes remain independent of production definitions.
This naming pass changes no supported format, byte layout or limit value.

## Header profile

The reader recognizes Jet 3/4 and ACE header codes 2, 3, 5 and 6. It reports format
family, raw compatibility subversion, inferred page size, whole-file page count
and whether the obfuscated page-encoding key is nonzero. This identifies header
fields, not the creating Access version/build, successful decryption or a valid
application. Unknown codes/signature combinations are rejected explicitly.

Read size is 66 bytes, charged before reading; file length is bounded to 64 MiB,
page-aligned and at least three pages. Header recognition alone does not decode
catalogs or records. The zero-key encoding constant is format metadata, not a
password key or an implementation of cryptography. Header inspection can report
encoded pages; catalog decoding rejects them.

## Catalog profile

Catalog decoding accepts the unencoded, 4 KiB-page Jet 4 / ACE-code-2 layout with
subversion 0. Other recognized header formats remain unsupported for catalog work.
Synthetic inputs exercise the shared layout; the registered real candidate has
ACE code 2. A separately generated Jackcess V2000 (Jet 4) mixed-scalar file now
matches an independent reader; see the [mixed-scalar profile](NATIVE_TABLES.md#mixed-scalar-jet-4-and-ace-comparison).
No Microsoft Access-authored Jet 4 candidate or native Jet provider has been run.

The reader bootstraps MSysObjects at table-definition page 2, obtains required
column types/positions from its definition, follows its owned-page usage map
(inline or indirect bitmap), and reads live catalog rows. Deleted roots are skipped;
live relocation pointers are followed with cycle detection. Dynamic fixed-field
positions, variable offset tables, null bits and strict UTF-16/compressed-Unicode
names are checked. Only Id, ParentId, Name, Type and Flags are decoded. Other
fields, object binaries, saved SQL, table contents and active content are not
interpreted. The live row total must agree with the table-definition count;
malformed/unsupported layouts fail without a partial inventory.

The shared [model](MODEL.md) retains unknown categories and unresolved parent IDs.
NativeObjectOrigin retains the original signed IDs/type, raw flags and root page/slot
in catalog order. It is inspection metadata, not a mutation handle. Flags are
reported without claiming a complete system/hidden-object interpretation. Native
type 4 (ODBC-linked table) remains distinct from signed -32764 (report).
ASCII-insensitive name lookup is an explicit current policy, not Access locale fidelity.

Readers charge shared byte/row/step budgets and check cancellation between units
of work. Structural ceilings also apply: 4,096 page reads, 4,096 allocated catalog
pages, 16 relocation hops, 256 column/index metadata entries, 16,384 live objects
and 2,048 encoded name bytes (also bounded by the declared column length).
Chained table definitions, unsupported usage maps/column profiles, encoded pages,
newer ACE catalog layouts and names invalid under SchemaName are rejected.
The CLI supplies 16 MiB cumulative bytes, 16,384 rows and 1,000,000 steps.
IDs use a caller-issued namespace; the CLI's scope 1 is local to one invocation
and is not a persisted identifier, file hash or native revision.

Local table-definition inspection now uses the same bounded metadata reader; see
[NATIVE_TABLES.md](NATIVE_TABLES.md) for implemented fields and comparisons.

## Research provenance

Primary implementation references reviewed on 18 October 2026:

- [MDB Tools format research](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/HACKING.md): native page/signature/catalog research.
- [MDB Tools header reader](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/file.c): header offsets, obfuscation and encoding key.
- [Catalog bootstrap](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/catalog.c), [table metadata](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/table.c), [row/usage decoding](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/data.c) and [Unicode compression](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/iconv.c): format facts used for the original bounded reader.
- [Jackcess JetFormat](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java) and [DatabaseImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/DatabaseImpl.java): independent layout constants and catalog types.
- [Microsoft-hosted community catalog query](https://learn.microsoft.com/en-us/answers/questions/5229782/how-do-i-list-my-tables-forms-queries-reports-and): signed application-object type mappings. This is community research, not a normative Microsoft binary-format specification.

MDB Tools library sources carry LGPL-2.0-or-later notices; Jackcess is Apache-2.0.
These were inspected as technical references in a temporary research directory.
No upstream source implementation, fixture, library or license text was copied
into production code or linked into the executable. Rust inspection code and
synthetic test inputs are original. Reverse-engineered reference agreement is not
Microsoft Access comparison evidence or a complete published file specification.

## Observed candidate and independent comparison

A disposable byte-identical copy of the [registered MIT example](FIXTURES.md)
reports Ace2007, subversion 0, 4,096-byte pages, 827 pages and a zero encoding key.
It yields 146 live metadata records: 36 tables, 59 queries, 13 forms, 8 reports,
3 macros, 12 modules, 9 containers, 1 relationship and 5 unknown categories.
These include system objects; they are not counts of user-visible runnable objects.
Unknown native codes 2, -32757 and -32758 remain visible.

On 18 October 2026, a read-only Jackcess 5.0.4 scan of MSysObjects selecting only
Id, ParentId, Type, Flags and Name matched all 146 tuples exactly after sorting
and normalizing Flags to unsigned 32-bit. Temurin OpenJDK 21.0.8 ran the original
[comparison helper](testing/CatalogOracle.java) on a disposable copy. The reviewed
Apache-2.0 [Maven artifact](https://repo.maven.apache.org/maven2/com/healthmarketscience/jackcess/jackcess/5.0.4/)
was 1,407,244 bytes with SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`;
its downloaded bytes also matched the publisher's SHA-1 sidecar. The JAR stayed
outside the repository and is neither a runtime nor CI dependency.

To repeat the metadata scan, supply that reviewed JAR and a disposable copy:

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/CatalogOracle.java path/to/copy.accdb
cargo run --locked -p tablecraft-cli -- inspect path/to/copy.accdb --catalog
```

The helper emits native fields as TSV; compare with CLI columns 3–7 after its
five header lines and one table-header line (normalize hexadecimal flags).
The original fixture SHA-256 is unchanged. No macros, VBA, object activation or
saved data queries were executed. This qualifies metadata inspection of that
candidate against an independent reader. It does not establish Microsoft Access
behavior, editing/preservation, application execution, database query execution,
shared-file safety or compatibility with other native generations.

## I/O failure and retry boundaries

Header and page reads use a shared exact-read loop. Every callback attempt,
including `Interrupted` retries and short reads, consumes a step and checks
cancellation; completion checks cancellation again before returning bytes.
Requested bytes are charged once before reading and failed work is not refunded.
An individual blocking Read/Seek callback still needs host-owned timeouts.
EOF maps to Truncated, other I/O errors retain only ErrorKind. No source error
message, path or payload is included in the engine diagnostic.

Fault-injection tests fail each Read/Seek callback in otherwise successful scalar
and Long Text snapshots, including EOF, and verify immediate failure and unchanged
input. Short reads complete normally. Repeated interruptions stop on cancellation
or exhausted work; cancellation during the final read also rejects completion.
Adapter tests verify a failed load publishes no rows or cache entry, an explicit
retry may later cache a complete snapshot, and repeated failures exhaust the same
original budget. Cached successful tables require no further source I/O.
