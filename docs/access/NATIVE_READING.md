# Read-only native format inspection

`inspect PATH --table NAME --check` additionally runs the explicit
[table consistency check](NATIVE_CHECKS.md), with coverage gaps reported per index.
Ordinary inspection remains metadata-only.

`encryption inspect PATH` reports inert [encryption envelope metadata](storage/NATIVE_ENCRYPTION.md)
without credentials, secrets or page decryption. Envelope recognition does not
qualify encrypted table reads; qualified Agile sources require verified credentials.
Basic `inspect PATH` reports page encryption status and type without credentials.
It also reports inert database [text-sort metadata](indexes/NATIVE_TEXT.md); table-schema
inspection includes separate text-column descriptors. Unknown orders are retained
without substituting a default or enabling native comparison/index semantics.
The engine's [stored collection declarations](indexes/NATIVE_TEXT.md#stored-collection-text-declarations)
now retain separate flat value/member text orders without parent/child scans;
completed collection snapshots expose the same exact metadata.

`crates/engines/tablecraft-ace` is a working standalone native library.
`inspect_header` takes raw Read + Seek; `inspect_catalog` takes NativeReadSource,
which adds explicit page interpretation without changing raw byte reads. Both
consume caller budgets and cancellation controls. Plain sources make no password
attempts; an explicit AgileSource authenticates before native reads. Inspection
performs no writes, file discovery, provider/linked-source activation or startup.
The library consumes core/schema/model and thiserror; no TableCraft host is needed.

`tablecraft inspect <path>` opens that explicit file read-only and buffers a small
header summary before stdout. `--catalog` adds shared descriptors and native
origins: object ID, category, native ID, parent ID, signed type, raw flags and name.
Names/headers use the shared escaped formatter and 1 MiB final output ceiling.
CLI inspection uses compact bordered sections on terminals, with CSV-encoded escaped-display
TSV when redirected or explicitly selected; `--format table` forces borders.
System tables are hidden by default; `--catalog --show-system` includes them.
Other object kinds remain visible. Both policies belong to the host; the reader remains unchanged. Errors exit 2 without successful partial output or file paths in diagnostics.
A catalog error publishes no header either. Output I/O/broken-pipe behavior is retained.
CLI reads now hold a [shared OS lock](writes/NATIVE_TRANSACTIONS.md#cli-read-coordination)
while using a raw native source and refuse cooperating exclusive writers. Use a
stable/offline copy: native Access shared-file locking and consistent concurrent
snapshot acquisition remain unimplemented. Host Read/Seek callbacks own I/O timeouts.

`inspect PATH --query NAME` additionally reads [inert saved-query fragments](queries/NATIVE_SAVED_QUERIES.md)
with signed native IDs and exact nullable fields. It does not reconstruct or run
SQL, follow remote-database metadata or imply query/index integrity.

## Format facts and reader policy

Native detection uses content, not filename extensions. Supported native table
layouts can be read under `.accdr`, `.accde` and `.mde` names through the same
inspection, browsing and query paths. Renamed fixture regressions do not qualify
Access-produced compiled applications. The Access backend now supplies a bounded
single-database CAB input for `.accdc`; signatures remain unverified. Qualified
ACCDT package/schema/data reads use the separate bounded
[template adapter](containers/ACCESS_TEMPLATE_DATA.md). See the
[distribution read contract](../features/ACCESS_DISTRIBUTION.md#required-read-scope).

Page/row geometry, packed references, type tags, metadata offsets, signatures and
Unicode widths have named definitions within their owning format/header/catalog
modules. Primitive widths derive from Rust integer/float types. The reader's
structural ceilings live separately in its private limits module: identical numbers
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

The 2 GiB read-source envelope permits bounded page reads without retaining the
whole file. It is separate from the 64 MiB complete-image cap used by NativeImage
and OfflineReadTransaction. Per-operation page/map/row/payload/output budgets
still apply. Header acceptance is not whole-file, format-maximum or live-file
consistency qualification. Microsoft's [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
list a 2 GB database limit; TableCraft's explicit envelope does not establish
support for every valid database within it. [Large-source evidence](../testing/LARGE_FILE_READS.md)
records high-offset synthetic reads and the remaining map/snapshot constraints.

Read size is 66 bytes, charged before reading; source length is bounded to 2 GiB,
page-aligned and at least three pages. Header recognition alone does not decode
catalogs or records. The zero-key encoding constant is format metadata, not a
password key or an implementation of cryptography. Header inspection can report
encoded pages; catalog decoding requires an explicit qualified decoder for them.

## Read-profile selection

`Header::read_profile()` selects `ReadProfile::Jet4`, `Ace2007`, `Ace2010`,
`Ace2016` or `Ace2019` for plain sources: Jet 4 requires subversion zero, while
qualified ACE families accept zero or one. Encryption removal preserves subversion
one. An explicit qualified EncryptedSource/AgileSource attaches decoded read
context for encoded inputs while retaining the encoding flag. Raw inspection alone
does not grant that context. Other families, subversions and encryption schemes
fail explicitly. Field and index checks
remain operation-specific; selection never authorizes writes or active content.

Selection lives under `format/profiles/`; shared layout/row/text algorithms remain
in their feature modules. Resource ceilings now live in `limits.rs`, separately
from compatibility selection. Additional profiles require actual read evidence;
no placeholder per-release modules or codec implementations are introduced.

A profile is a validated capability selection, not a replacement decoder for each
Access release. Header signatures (`Standard Jet DB`/`Standard ACE DB`), format
code, subversion and encoding status select the candidate family. The qualified
families reuse 4,096-byte pages, table/catalog traversal, row-slot handling,
variable offsets, text/long-value decoding and ordinary scalar representations.
Feature modules apply further layout/type/index checks: for example Large Number
is limited to the ACE 16 families and Extended DateTime to the 2019 family.
Unsupported layouts fail rather than falling back to the nearest version.

After native decoding, every profile supplies the same TableCraft schema/values
to browsing and query evaluation. SQL execution is therefore shared as well.
Neither extensions nor magic bytes establish complete compatibility. Jet 3's
recognized 2,048-byte header family and unsupported encoded inputs do not
become readable by assigning a profile. Qualified Agile sources now use explicit
password verification/page decoding; ACE 2010 ordinary table reads additionally
have independent encrypted-fixture evidence. New support needs actual decoder work
where structures differ and independently checked fixtures for each operation.
The registered example reports `Ace2007` (code 2), not ACE 16; its existing bounded
read/query support and a fresh V2007 scalar comparison are regression evidence,
not proof of every ACE 12 feature or of its creating Access application's version.

## Compatibility targets

ACE 16 is the primary target, with 100% compatibility as the long-term goal.
New fixture collection, native comparison and implementation work prioritize it.
Immediate read/query work also prioritizes ACE 2007/ACE 12 and Jet 4; preserve
those regression profiles alongside ACE 16. Other generations remain secondary. [ROADMAP.md](../ROADMAP.md) records the scope
and existing deferrals. This priority does not expand the implemented read profiles.

Engine release numbers and file-format families are separate dimensions. An
`.mdb`/`.accdb` extension or recognized header is insufficient to qualify its
catalog, fields, indexes, encryption or application objects.

| Target | Current evidence and intended scope |
| --- | --- |
| Jet 4 | Bounded unencoded reads, including exact Decimal indexes/relationships; [current-profile qualification](profiles/NATIVE_JET4.md) uses independent comparisons, not native Access conformance |
| ACE 12 (Access 2007) | Bounded unencoded scalar/index reads plus qualified raw complex links, multivalues and stored attachment/history reads; [legacy complex evidence](../testing/FIXTURES.md#ace-2007-complex-key-and-multivalue-inputs) is independent, not native Access conformance |
| ACE 14 (Access 2010) | Bounded ordinary table reads; qualified Agile fixture inventory/rows/query checks are independent evidence; advanced field/index variants need separate qualification |
| ACE 15 (Access 2013) | Explicit compatibility target; test its actual stored layout and features rather than inventing a separate header family |
| ACE 16 (Access 2016 and later releases) | Primary 100% target; bounded unencoded 2016/2019-family reads now work with independent comparisons; writes and broader features remain unqualified |
| Jet 3 (Access 95/97) | Header recognized; legacy catalog, text/code-page, row and index reading planned |
| Jet 1.0/1.1 (Access 1.0/1.1) | Optional read-only feasibility study; detection and extraction unqualified, no implemented support |
| Jet 2 (Access 2.0) | Optional read-only feasibility study; recognition and data extraction unimplemented; writes are outside the initial study |

The reviewed [Jackcess format definitions](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java)
identify Access 2010, 2016 and 2019 layout codes separately. Their comments report
Access 2013 using the Access 2010 layout, with no observed file carrying the
nominal code 4. This is independent implementation research, not a Microsoft
binary-format guarantee. Do not enable code 4 or infer the creating engine from a
header without fixture evidence. Access 2016/2019/2021 sharing a version number
also does not establish identical file layouts.

Jet 2 feasibility starts with licensed/provenanced fixtures, enough layout research
to distinguish it reliably and a trustworthy comparison environment. Attempt
bounded catalog and scalar extraction only after those prerequisites; record gaps
and leave it deferred if it requires substantial reverse engineering. The
[Jackcess support matrix](https://jackcess.sourceforge.io/faq.html) starts at
Access 97, so that reference does not supply a Jet 2 comparison baseline.
[Microsoft's legacy import guidance](https://support.microsoft.com/en-us/access/import-access-2-0-and-access-95-databases-into-current-versions)
identifies Access 2007 as the last version supporting Access 2.0 import/link/export;
modern Access cannot simply be assumed to open the original files for comparison.

Microsoft's [DAO CreateDatabase documentation](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/dbengine-createdatabase-method-dao)
lists `dbVersion10`, `dbVersion11` and `dbVersion20` creation options. A compatible
older runtime is a candidate for generating original, known-content fixtures and
comparing extraction. These retained constants do not establish that modern ACE
can create/open those formats, or that any such environment has been run here.

Jet 1/2 studies start with reliable detection, catalog/schema discovery and
sequential scalar extraction. Indexes, long values, legacy code pages and each
1.0/1.1/2.x variant need their own evidence; application objects and writes remain
outside the initial experiment. README status stays "experimental research target;
not implemented" until useful reads are demonstrated. Any later "limited support"
label must identify the tested subset and comparison gaps. Synthetic corruption
tests help bound failures but cannot replace known-content legacy fixtures.

These are targets, not expanded reader support. Read qualification precedes
preservation and mutation qualification for each profile. Advanced fields,
encoding/encryption and compiled application objects retain separate coverage.

## Catalog profile

Catalog decoding accepts the shared 4 KiB-page Jet 4 / ACE-code-2/3/5/6
layouts, with subversion 0 for Jet 4 and 0/1 for plain or explicitly decoded ACE
sources. [ACE 16 evidence](profiles/NATIVE_ACE16.md) records original
newer-family comparisons. Other recognized headers remain unsupported for catalogs.
Synthetic inputs exercise the shared layout; the registered real candidate has
ACE code 2. A separately generated Jackcess V2000 (Jet 4) mixed-scalar file now
matches an independent reader; see the [mixed-scalar profile](storage/NATIVE_TABLES.md#mixed-scalar-jet-4-and-ace-comparison).
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

Compressed Unicode mode switches apply at character boundaries. A zero high byte
inside a wide UTF-16 unit is retained as data, including wide ASCII. Odd wide
tails and unpaired surrogates still fail. The
[wide-segment grammar controls](../testing/COMPRESSED_TEXT_WIDE_SEGMENTS.md) compare
original streams with pinned independent decoders and explicitly distinguish
derived payload controls from native-writer or Access qualification.

The shared [model](../foundation/MODEL.md) retains unknown categories and unresolved parent IDs.
NativeObjectOrigin retains the original signed IDs/type, raw flags and root page/slot
in catalog order. It is inspection metadata, not a mutation handle. Flags are
reported without claiming a complete system/hidden-object interpretation. Native
type 4 (ODBC-linked table) remains distinct from signed -32764 (report).
ASCII-insensitive name lookup is an explicit current policy, not Access locale fidelity.

Readers charge shared byte/row/step budgets and check cancellation between units
of work. Catalog enumeration borrows its current data page for each live root
instead of reading that page again per row. Signature, owner, slot and row bounds
are still validated; relocation targets use ordinary bounded physical reads with
cycle/hop checks. There is no cache across pages or calls, and a stable/offline
source remains required. Byte/read limits count the physical reads actually made;
row allocations and row/work charges remain explicit.

Structural ceilings also apply: 4,096 page reads, 4,096 allocated catalog
pages, 16 relocation hops, 256 column/index metadata entries, 16,384 live objects
and 2,048 encoded name bytes (also bounded by the declared column length).
Table definitions now join at most 64 pages (261,640 logical bytes), with bounded
declared lengths, signatures, links/cycles, allocation and I/O. See
[chained definition evidence](storage/NATIVE_TABLES.md#bounded-chained-table-definitions).
Unsupported usage maps/column profiles, encoded pages,
newer ACE catalog layouts and names invalid under SchemaName are rejected.
The CLI supplies 16 MiB cumulative bytes, 16,384 rows and 1,000,000 steps.
IDs use a caller-issued namespace; the CLI's scope 1 is local to one invocation
and is not a persisted identifier, file hash or native revision.

## Usage-map ordering and ownership bounds

Inline bitmap bits and indirect slots emit strictly ascending logical page
numbers. Each indirect slot covers a disjoint range of 32,736 pages. The decoder
checks the last emitted number for monotonicity/uniqueness in constant time,
replacing a search through every earlier page. This makes bookkeeping linear in
bitmap bytes plus selected pages and establishes the sorted contract used by
native index ownership binary searches.

Overflow/out-of-file pages, malformed pointer widths, signatures, unknown map
types and caller byte/work limits remain checked. Materialized consumers retain
the 4,096-address ceiling and charge emitted references to allocation allowance.
Bounded row paging now validates the whole usage metadata first, then enumerates
addresses through a [bounded cursor](../testing/ALLOCATION_MAP_PAGING.md), retaining
only its usage row and one bitmap page. It does not build the address list.
Physical reads remain capped separately; bitmap work/cancellation is precharged
at most eight bytes at a time. Tests cover inline/indirect agreement, byte/word/tail
boundaries, empty maps, limits, overflow, cancellation and a 256-case independent
bit model with strict order assertions.

The original [UsageMapFixture recipe](../testing/UsageMapFixture.java) creates 2,000
rows with 12 Binary fields of 240 bytes each, one row per owned data page. Its
indirect maps emit 2,000 pages in each generated Jet 4/ACE file. The standalone
[table_data example](../../crates/engines/tablecraft-ace/examples/table_data.rs) and
[comparison runner](../testing/compare_usage_maps.py) match all 4,000 tagged rows,
including every Binary byte, against Jackcess. Per-format output SHA-256 is
`c4572f8f86f2a600f4fc0b6ef5c55dc63e370552374ac567427c06785db6cb1e`.

These engineering runs explicitly use a 256 MiB native work/allocation budget;
the CLI's default 16 MiB allowance and all structural ceilings are unchanged.
The example uses clap Parser and emits engineering tags, not the CLI protocol.
The [benchmark report](../testing/NATIVE_BENCHMARKS.md#usage-map-traversal) retains
timing variance and does not establish a general whole-reader speedup.

Local table-definition inspection now uses the same bounded metadata reader; see
[NATIVE_TABLES.md](storage/NATIVE_TABLES.md) for implemented fields and comparisons.

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

A disposable byte-identical copy of the [registered MIT example](../testing/FIXTURES.md)
reports Ace2007, subversion 0, 4,096-byte pages, 827 pages and a zero encoding key.
It yields 146 live metadata records: 36 tables, 59 queries, 13 forms, 8 reports,
3 macros, 12 modules, 9 containers, 1 relationship and 5 unknown categories.
These include system objects; they are not counts of user-visible runnable objects.
Unknown native codes 2, -32757 and -32758 remain visible.

On 18 October 2026, a read-only Jackcess 5.0.4 scan of MSysObjects selecting only
Id, ParentId, Type, Flags and Name matched all 146 tuples exactly after sorting
and normalizing Flags to unsigned 32-bit. Temurin OpenJDK 21.0.8 ran the original
[comparison helper](../testing/CatalogOracle.java) on a disposable copy. The reviewed
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

Header and page reads use [tablecraft-io](../foundation/IO.md) for bounded read-at-offset operations. Every read attempt,
including `Interrupted` retries and short reads, consumes a step and checks
cancellation; completion checks cancellation again before returning bytes.
Absolute seeks also check cancellation and charge one work step before the
callback. Native page geometry/structure validation stays in ACE; I/O failures
map to the existing native error contract. Requested bytes are charged once by
ACE before I/O and failed work is not refunded.
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


`inspect PATH --table NAME --attachments COLUMN` reports checked stored metadata
and encoded byte lengths. `--version-history MEMO` reports stored versions for a
visible append-only Memo. These complete-column views use shared NativeBrowser
APIs, retaining exact DATE bits and scoped parent memberships without extraction,
latest-value selection or active-content execution. See the
[CLI contract](../cli/CLI_BROWSING.md#stored-attachments-and-memo-history).

## Explicit larger stored-collection pages

Buffered `rows --collections`/`--multivalues` can now supply native byte/row/work
controls for complete parent-key and child-table validation. Visible page ranges
remain independent: even an empty page validates required whole collections.
The browser retains cumulative controls, output remains buffered under its own
ceiling, and attachment data stays encoded. See [request, scope and independent
evidence](LARGER_NATIVE_COLLECTIONS.md#buffered-larger-cli-pages). Inspection and
ordinary scalar browsing defaults remain unchanged.
