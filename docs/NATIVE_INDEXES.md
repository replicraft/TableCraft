# Native index inspection

[Table consistency checks](NATIVE_CHECKS.md) combine complete sequential reads
with every qualified index and report unverified profiles explicitly.

Index-page prefixes now use borrowed zerocopy layouts with explicit little-endian,
unaligned fields. Signature/owner/free-space/order/reference validation remains in
the decoder. Bitmap traversal uses bounded word blocks; byte work is precharged
and cancellation checked at each block (at most eight bytes), with existing
per-entry checks retained. [Benchmarks](testing/NATIVE_BENCHMARKS.md) compare the
original bit loop, word iteration and a SIMD candidate without changing the file
format or claiming native compatibility.

`inspect_table_indexes` returns a completed IndexInspection with owning table schema
and native logical index definitions. CLI: `inspect COPY --table NAME --indexes`.
It uses the existing bounded, unencoded Jet 4 / ACE-code-2/5/6 table-definition
profiles. Index pages and the selected table's data rows are not traversed by inspection.

Logical index numbers, physical data numbers, root/usage references, raw type/flags,
column numbers and direction are retained separately. Multiple logical indexes
can refer to physical index data. Native definition order is preserved; it can
differ from Jackcess's logical-number order. Column numbers resolve against native
logical column numbers, including current metadata gaps. This metadata is not a
schema uniqueness/foreign-key validation result or proof of a valid index tree.
Known ordinary/primary declarations now expose IndexConstraints (primary, unique,
required key and ignore-NULL); unknown profiles remain explicit. Qualified
[Exact numeric constraint validation](NATIVE_CHECKS.md#qualified-index-constraints) belongs
to check_table, not metadata inspection or normal indexed retrieval.

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
partial inspection. [Shared bounded definition decoding](NATIVE_TABLES.md#bounded-chained-table-definitions)
joins continuation payloads for column/index metadata crossing page boundaries.

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
fixture bytes remain unchanged. Inspection/read APIs never write. Qualified
[offline existing-tree maintenance](NATIVE_WRITES.md#existing-tree-int32-index-maintenance)
now exists separately; broader mutation, automatic query access-path selection
and shared-file coordination remain unimplemented.

## Reciprocal foreign-index links

`NativeIndex::foreign_reference()` retains type-2 logical references separately
from physical flags: local parent/dependent role, target definition page/logical
index number, and exact update/delete bytes. Ordinary metadata inspection does
not resolve these pointers. Unknown roles/action bits remain visible, while the
missing-index sentinel supplies no reference. Typed action accessors return no
interpretation for unknown/conflicting bytes.

`inspect_foreign_index_links` explicitly resolves every foreign index on one
stable/offline table. It checks unique local catalog identities for source and target
page, target logical number, reciprocal back-pointer, opposite roles, equal key
counts and matching action declarations. Shared physical data numbers do not
replace logical identities; self-references and multiple primary aliases remain
distinct. It reads bounded metadata only, with charged fallible allocations,
work/cancellation checks and no recursive graph walk or partial report. Unknown
roles/actions or absent references return Unsupported; broken pointers return
Malformed. It also reads MSysRelationships metadata and requires one enforced
declaration per link, matching endpoint names, aligned key-column pairs, actions
and declared parent/one-to-one uniqueness. Declaration ordinal permutations are
accepted when they retain the same column pairs. Every enforced endpoint on the
selected table must be covered, including both ends of self-relations; non-enforced
declarations require no foreign links. Ambiguous/missing/inconsistent coverage is
Malformed and unknown rule/physical-flag semantics are Unsupported. No user rows
or key pages are traversed, key membership/actions are not executed, and ordinary
index constraints and write gates remain unchanged.

The [original read-only oracle](testing/ForeignIndexOracle.java) resolves both
ends through Jackcess's public implementation accessors. The inert
[fixture recipe](testing/ForeignIndexFixture.java) creates fresh V2016/V2019
restrict, cascade-update/delete, SET NULL, composite, self-reference and shared
primary-alias cases, plus one-to-one and non-enforced controls in v2.
[The comparison runner](testing/compare_foreign_indexes.py)
accepts `--jar`, `--cli`, `--example`, `--ace2016`, `--ace2019` and `--sample`.
It matches fourteen directed links per generated v2 file and two in the registered
sample, rejecting thirty-one corrupted copies per generated family with empty
output. Reordered composite declaration ordinals also match both readers.
[Hashes and provenance](FIXTURES.md#relationship-catalog-agreement-inputs)
remain separate from native Access qualification.

Eight CI tests cover literal self/alias metadata and actions on both ACE families,
broken/unknown pointers and roles, early/late cancellation, resource limits and
registered-sample links. Distinct catalog IDs aliasing the target page reject as
ambiguous. Corrupt user/index data pages do not affect link-only inspection,
proving that metadata success is not physical-tree qualification.
Catalog disagreement, missing/ambiguous edges, unsupported rules, uniqueness and
composite pair/ordinal checks are covered separately from physical traversal.
The engineering example uses clap Parser and buffers output until inspection
succeeds:

```sh
cargo run --locked -p tablecraft-ace --example foreign_index_links -- copy.accdb Parents
```

## Bounded indexed reads

`read_table_index` reads a complete scalar TableSnapshot through an explicitly named
native index. The qualified profile has one to ten Boolean/Byte/Int16/Int32/Currency/Single/Double/legacy DateTime/GUID key
components, also permitting Int64 components on the 2016/2019 profiles, with
normal/primary logical type, or a foreign logical index after the complete
reciprocal/catalog checks above. Each component retains its own direction; NULL and
duplicate tuples are included. Native Boolean atoms cannot be NULL. GUIDs retain the explicit segment profile below,
including GUID-bearing tuples. Ignore-NULL indexes are rejected because they
cannot supply a complete table snapshot. Text, other key carriers and
unqualified flags/types fail with Unsupported. Unknown index
names have a distinct payload-free error. No sequential fallback is attempted.
Jet 4 and ACE 12/16-family Decimal keys additionally use the exact precision/scale
profiles below, including Decimal-bearing tuples. Legacy descending sign groups
are qualified separately; other unqualified index carriers still fail.

### Foreign physical traversal and checks

Selecting a foreign index qualifies its declarations on the current stable source
before choosing the existing key codec or traversing any tree. There is no fallback
to a sequential read. Metadata-only inspection and fresh explicit link inspection
still return raw foreign declarations without a physical-read qualification marker.
Qualification is internal to that operation, not a cached or transferable write
capability. Typed numeric ranges retain their carrier checks and partial-tree scope.

`check_table` validates foreign metadata before checking eligible physical trees
and exact unique/required declarations. Unsupported metadata leaves foreign indexes
explicitly unverified; malformed references/catalog agreement, resource errors and
cancellation fail without a report. No successful prefix is published. Each
qualified foreign index then receives the same complete signature/owner/order/
separator/sibling/root/key/row checks as an ordinary index. Physical child-key
duplicates and NULLs remain allowed when its declaration permits them; parent
aliases retain primary-backed uniqueness and requiredness. This does not establish
referential row membership or execute cascade actions. Ordinary Jet 4 plans still
block foreign-index tables. ACE 16 [engineering previews](testing/ACE16_WRITE_PREVIEWS.md#checked-foreign-index-updates)
can now maintain eligible single-column integral keys after these physical and
metadata checks plus complete before/after relationship audits. A successful read
alone remains insufficient for mutation.

Six additional CI tests cover leaf/node/descending/nullable alias reads, exact
Int32 ranges, malformed versus unsupported coverage, physical corruption and
duplicate-key constraints, shared budgets/cancellation and the separate preview/
journalable write guard. Eight metadata tests remain separate. The independent
[read runner](testing/compare_foreign_index_reads.py) takes `--jar`, `--cli`, `--rows`,
`--ace2016`, `--ace2019`, `--sample`, `--mask2016` and `--mask2019`. The current v3
recipe includes duplicate child keys plus 1,000-parent/1,200-child node trees.
Comparisons match 38 foreign-index reads/5,408 returned rows across both ACE
families, the registered sample and existing nullable Large Number fixtures;
eight corrupted copies reject through both read and check paths. No Java input
or generated database is required in CI. [Hashes and limits](FIXTURES.md#foreign-physical-read-inputs)
remain independent evidence, not native Access or write qualification.

The decoder traverses root/node/leaf pages, reconstructs shared entry prefixes from
the entry-boundary bitmap, and decodes exact unsigned Byte/signed integer or scaled-Currency keys plus big-endian row/child
references. Fixed widths, markers, offsets, flags and ceilings have named owners.
Boolean keys use their separately qualified two-value encoding below.
Single/Double keys preserve IEEE bits under their private physical ordering below.
Legacy DateTime shares that physical transform while validating its calendar
envelope for every key, including internal separators.
It verifies page signatures/owner, free space, boundaries, key widths, strict key/row
ordering, parent separator bounds, owned-page membership, leaf sibling continuity,
cycles and a 16-level depth ceiling. At most 16,384 entries/live rows and the existing
page/read/allocation/work limits apply. Leaf count must equal the declared table
count. Referenced root slots must be live and belong to owned data pages. Each
root is validated against the current data page; consecutive references to the
same page reuse that one buffer. A different page replaces it, and returning to
an earlier page fetches it again. Relocation
uses the existing bounded reader. Every decoded row must validate against its schema
and agree with the indexed key. A failure returns no partial snapshot.

This buffer exists only during the current native read, including typed range
reads. It preserves per-root ownership/slot/deletion checks, row/key agreement,
work/row charging and cancellation. Relocation targets still take physical reads,
even on the same page; their owner/slot/cycle checks remain unchanged. Empty
selections fetch no user data page. Every new native operation starts fresh.
There is no shared-file isolation or cross-operation page cache; stable/offline
source requirements remain in force. Four focused tests trace hits, misses,
fresh operations, short reads, relocation targets, late cached corruption and
every cancellation point. Adapter tests separately track data and required-index
reads so fewer physical reads cannot hide fallback or a cached partial table.

Internal nodes may either use a separate final-child pointer or give every child
an explicit upper-bound entry with a zero tail pointer. The latter requires at
least one entry. Empty childless internal nodes, nonzero leaf tails, repeated
children/cycles and inconsistent separators remain malformed. Complete reads,
checks and typed single-key seeks share this traversal. A synthetic two-leaf
regression independently covers both the zero-tail node and its malformed variants;
the narrower key writer still rejects this topology.

This detects many structural inconsistencies; it does not independently establish
index completeness/constraints against an exhaustive separate data scan or provide
shared-file isolation. Read only stable/offline sources. Separator references may
be retained upper bounds, so they are checked as bounds rather than fetched as rows.
The recorded level byte is not used as a compatibility guarantee; depth is bounded
by actual traversal. These reads do not repair indexes or authorize writes;
the separate native edit profile owns mutation constraints. Native index key
order is not a SQL ORDER BY guarantee.

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

## Complete single-Decimal indexes

Jet 4 and ACE 12/16-family single-column Decimal indexes now share complete tree/row reads and
explicit unique/primary/required checks. Exact signed i128 coefficients preserve
all 28 digits, with the declared scale retained in returned Decimal values.
Every key, including internal bounds, validates the field precision. Row agreement
requires a Decimal carrier with exactly the declared scale and bounded coefficient;
Int64, Currency and floating carriers are never reinterpreted or coerced. NULL and
zero remain distinct. Typed integer seeks reject Decimal even for All/Null.

The corrected ACE fixed-point encoding has an ordinary direction/NULL marker,
a sign byte and 16 big-endian magnitude bytes. Ascending nonnegative values use
`ff` plus the magnitude; negative values use `00` plus complemented magnitude.
Descending complements that entire sign/magnitude body. Other sign bytes,
out-of-precision magnitudes and noncanonical negative-zero keys are malformed.
A separately derived 26-byte entry buffer holds the 18-byte key plus row/child
references. Existing numeric/GUID buffers and compact key models stay unchanged;
actual i128 entry allocation sizes and bounded work remain charged.

The reviewed Jackcess 5.0.4 `FixedPointColumnDescriptor` and big-endian
`ColumnImpl.writeNumericValue` establish the reference transform. The original
[recipe](testing/DecimalIndexFixture.java) asserts its V2016/V2019 formats use the
corrected encoding, then writes coefficients directly through BigInteger/BigDecimal
with no floating conversion. Jet 4 additionally retains the sign byte for descending keys while
complementing only the magnitude. Its descending negative group precedes the
nonnegative group. A private disjoint integer band models that physical order for
both decoded keys and row agreement; returned Decimal coefficients and SQL ordering
are unchanged. The band is derived from the core Decimal bound and fits i128.
ACE 12 uses corrected encoding, established by fresh V2007 fixtures rather than
header retagging. See [Jet 4 qualification](NATIVE_JET4.md).

[Independent comparisons](testing/compare_decimal_indexes.py) match 6,116 ordered
rows/profile, actual ascending/descending node trees, precisions 4/28, scales
0/2/4/28, beyond-i64/2^53 neighbors, signed extrema, unique/primary/NULL checks,
required-index residual queries and corrupt-root buffering. [Input/canonical hashes](FIXTURES.md#original-ace-16-decimal-index-inputs)
remain recorded and originals unchanged. The earlier empty Decimal composite
now qualifies through the separate complete tuple profile below.

Seven new CI tests include exact carrier/scale rejection, a 256-case i128
ordering model, independent sign/word-order goldens, maximum compressed prefixes, every
sign byte, precision overflow and a malformed separator with a valid child.
Metadata/profile/typed seek/resource/cancellation failures and the ACE format
write gate remain explicit. No dependency was added. Decimal composites now have
their separate qualification below. Temporal and Decimal seeks, Decimal key
mutation, other unqualified index variants, actual
Microsoft Access and application save/reopen remain unqualified. All ACE writes
stay blocked; read evidence does not authorize native saves.

## Complete Decimal-bearing composite indexes

Jet 4 and ACE 12/16-family complete reads/checks now admit two-to-ten Decimal components mixed
with qualified numeric/Boolean/legacy DateTime/GUID carriers. Each Decimal position
retains its own precision and scale; exact i128 coefficients never enter an i64
or floating representation. A distinct private atom model is selected only when
the index contains Decimal. Existing compact numeric and GUID-only key models
remain separate. The shared codec rejects out-of-precision magnitudes, bad signs
and noncanonical negative-zero keys at any component or tree level.

Decimal's 18-byte component fits the existing 19-byte GUID maximum, so the wider
profile retains the derived 198-byte entry-buffer ceiling, including mixed GUID
keys and row/child references. Actual wider atom/entry allocation sizes are
charged. Per-component NULL, direction, metadata mapping and work/cancellation
retain the common complete walker. Unique tuples exempt only entirely NULL keys;
partial-NULL duplicates fail and primary/required keys reject any NULL. Decimal
or GUID zero is a value. Integer seeks reject every composite even for All/Null.

Compact numeric/GUID column metadata now also stays separate from Decimal's
wider descriptors. Statically selected column sets share the tuple policy without
adding Decimal dispatch to every existing component. Raw native type detection
chooses the column set before profile construction; the chosen builder still
validates format, declared precision/scale, flags and column mapping. This changes
neither the public read/check API nor native eligibility or constraint semantics.

The original [recipe](testing/DecimalCompositeIndexFixture.java) writes fresh
V2016/V2019 inputs through reviewed Jackcess 5.0.4, with corrected numeric encoding
asserted and expression evaluation disabled. It combines exact Decimal scales
4/28, GUID, Currency, Int64, Byte, Int16, Single, Boolean and legacy DateTime in
reordered ten-component keys, plus ten Decimal fields at ten scales. Unique,
primary, all/partial NULL and empty/unqualified controls remain explicit.

[Independent comparisons](testing/compare_decimal_composite_indexes.py) match
3,512 ordered rows/profile with exact coefficients/scales, GUID/IEEE/date bits,
required-index residual filters and complete checks. Ten-Decimal trees contain
nine nodes/92 leaves across three indexes, maximum expanded entries 188/184 bytes
and maximum observed prefix 162. The 183-byte prefix is a separate synthetic
golden. [Fixture records](FIXTURES.md#original-ace-16-decimal-tuple-index-inputs)
retain immutable original/canonical hashes. Corrupt selected roots and unqualified
Decimal/Text tuples fail without output or sequential fallback. The earlier
6,116-row single-Decimal runner additionally checks its empty Decimal/Int32 tuple
without changing the original input or canonical hash.

Six new CI tests include a 256-case typed nullable model, every other qualified
numeric carrier, per-field scales, component permutations, exact constraints,
ten Decimal fields/compressed prefixes, strict component/key agreement, legacy/corrected direction distinctions
and empty Text rejection, typed seek/resource/cancellation failures and the ACE
write gate. No dependency or SQL coercion/order policy was added. Exact Decimal
foreign-key membership has [separate qualification](NATIVE_CONSTRAINTS.md#exact-decimal-relationship-membership).
Decimal composite seeks/mutation, other numeric variants, actual Access and application
save/reopen remain unqualified; all ACE writes stay blocked.

## Complete legacy DateTime indexes

Legacy eight-byte DATE now qualifies for complete single and mixed index reads/
checks. Its exact serial bits share the Double physical transform, while a focused
DATE policy validates the finite year-100-through-9999 envelope on **every** key.
That includes internal separators with no directly fetched row. Invalid dates,
infinities and NaNs are malformed, rather than loose accepted parent bounds.
Returned carriers retain signed zero, day-zero aliases, negative fractions and
sub-millisecond bits without a calendar round trip or precision quantization.
The 17/98/198-byte entry buffers and existing resource ceilings remain unchanged.

Physical order follows the encoded serial, separately from civil time. For
example `-0.25` and `+0.25` remain different physical keys but denote the same
epoch-day time; `-1.75` sorts before `-1.25` physically while its civil time is
later. TableCraft's residual equality and ORDER BY retain their existing temporal
semantics. No DateTime seek/pushdown or foreign-key qualification is enabled.
Typed integer seeks still reject this distinct carrier even for All/Null.

The reviewed Jackcess 5.0.4 `FloatingPointColumnDescriptor`, big-endian DATE writer
and bit-preserving Date carrier establish the independent transform. The original
[recipe](testing/DateIndexFixture.java) writes fresh inert V2016/V2019 trees through
that artifact, without evaluating expressions. It asserts physical uniqueness
distinguishes both signed zeros and calendar aliases, exact duplicates fail,
ordinary unique NULLs repeat and primary NULLs fail. This is artifact evidence,
not Microsoft Access qualification of aliased date writes or constraints.

[Independent comparisons](testing/compare_date_indexes.py) match 4,528 ordered
rows/profile with exact tagged serial bits, sequential controls, complete checks,
single/mixed/ten-DateTime node trees and corrupt-root buffering. Required-index
CLI tests check residual NULL/ID filters, quarter-day alias equality and civil
sorting independently of native key order. The corrected read-only oracle
serializes retained DATE carriers into memory, avoiding the lossy `toDateDouble`
calendar path. [Input/canonical hashes](FIXTURES.md#original-ace-16-date-index-inputs)
remain recorded and originals unchanged.

Six CI tests include a 256-case valid arbitrary-bit model, independent negative
serial wire goldens, NULL/alias/primary constraints, maximum compressed prefixes,
mixed Boolean/Currency/GUID keys and an explicit-tail node whose valid separator
is replaced by an invalid DATE while its live child remains valid. Integer seeks,
resource errors, cancellation and unchanged bytes outside an unrelated eligible
Jet 4 Int32 edit retain explicit checks. Production DateTime mutations, normal ACE saves,
writes, actual Access and application save/reopen remain unqualified. Extended
DateTime indexes have their own unimplemented encoding and are not enabled here.

## Complete Single and Double indexes

Single/Double now qualify for complete single and two-to-ten-component mixed
index reads/checks. The codec transforms exact IEEE bits into private ordered
atoms, without a floating operation, narrowing or canonicalizing any NaN. The
original Single/Double carrier, signed zero, subnormal, infinity, NaN sign and
payload remain unchanged in returned rows. NULL remains distinct from every bit
pattern. The private physical order matches IEEE total bit order, not SQL numeric
equality; positive and negative zero are distinct physical keys.

Ascending positive-sign values flip their sign bit; negative-sign values complement
all bits. Descending complements the ascending bytes. The ordinary direction/NULL
marker precedes the four/eight value bytes. Existing buffer ceilings, full tree/
row agreement and resource charging remain unchanged. A fixed carrier per index
position prevents Float/Int32 or Double/Int64/Currency reinterpretation. Typed
integer seeks reject both floating carriers, even for All/Null. No floating range
API, foreign-key qualification, general coercion or new dependency is introduced.

The reviewed Jackcess 5.0.4 JAR and pinned
[FloatingPointColumnDescriptor/ColumnImpl reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
establish the physical transform. The original [recipe](testing/FloatingIndexFixture.java)
includes both signed zeros, finite endpoints, subnormals, infinities, positive/
negative quiet NaNs and distinct payloads. It asserts exact duplicate rejection,
distinct zero/NaN-key acceptance and repeated unique NULLs through that artifact.
This is independent-reader engineering evidence, not native Access qualification
of non-finite writes or SQL constraints.

[The runner](testing/compare_floating_indexes.py) compares tagged hexadecimal
IEEE bytes in complete native index order, with sequential controls. Engineering
`table_data --index INDEX` and `TableDataOracle PATH --index TABLE INDEX` select
the required index without fallback; the normal CLI protocol is unchanged.
Finite-only tables also exercise required-index CLI queries and residual NULL/ID
filters. Tables containing non-finite values retain query's explicit rejection,
even when only an ID is projected. Corrupt finite roots must fail with no output.
[Fixture hashes](FIXTURES.md#original-ace-16-floating-index-inputs) record immutable
inputs and canonical comparisons.

Six CI tests cover an independent 256-case all-bit-pattern total-order model,
signaling/quiet NaN goldens, exact constraints, compressed row references,
ten-Double/93-byte prefixes, mixed Boolean/Currency/GUID tuples, invalid markers/
key mismatches, typed seek rejection and resource failures. Unrelated eligible
Jet 4 Int32 edits preserve every byte outside the field; floating targets and all
ACE writes remain blocked. Actual Access, SQL non-finite behavior, floating seeks/
mutation and legacy DateTime indexes have separate qualification requirements;
the latter now use the explicit DATE profile above.

## Complete Boolean indexes

Native Yes/No fields now qualify for complete single-key and two-to-ten-component
mixed index reads/checks. Their row mask represents the value, so both true and
false are present values; neither can make a tuple entirely NULL. The private
physical order places true before false ascending and reverses it descending.
This introduces no Boolean-to-integer SQL coercion or general Value ordering.

Each Boolean key has the ordinary non-NULL direction marker followed by `00`
for ascending true or `ff` for ascending false. Descending complements that value
byte. Other bytes and NULL markers are malformed. A focused codec maps only these
two exact values into private ordered atoms; row comparison requires a Boolean
carrier. Existing single/numeric-tuple/GUID-tuple buffer ceilings and allocation
charges remain unchanged. Typed Int32/Int64 seeks reject Boolean indexes even
for All/Null. Ignore-NULL index declarations retain the complete-reader rejection.

The reviewed Jackcess 5.0.4 JAR's `IndexCodes`, `BooleanColumnDescriptor` and
`ColumnDescriptor.writeValue` establish the reference encoding. Source reference:
[IndexData.java](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
and [IndexCodes.java](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexCodes.java).
The original [fixture recipe](testing/BooleanIndexFixture.java) separately asserts
that supplied NULL becomes stored false and that unique/primary Boolean indexes
reject a subsequent NULL input as a duplicate false value. This observation is
specific to native stored Yes/No fields, not linked backends or expression NULLs.

[Independent comparisons](testing/compare_boolean_indexes.py) match 4,509 ordered
rows/profile, actual single/mixed/ten-Boolean node trees, Boolean equality and
IS NULL residual filters, unique/primary/partial-NULL checks and corrupt-root
buffering. [Input and canonical hashes](FIXTURES.md#original-ace-16-boolean-index-inputs)
remain recorded and original bytes unchanged. Six CI tests include a 256-case
two-value model, compressed row-reference prefixes, every possible value byte,
NULL/key mismatches, typed seeks, mixed carriers, constraints, budgets and
cancellation. Unrelated eligible Jet 4 Int32 edits retain every byte outside the
field; the Jet 4 Int32 editor still rejects Boolean targets and normal ACE saves stay blocked. Boolean seeks/production mutation,
foreign-key qualification and actual Microsoft Access evidence remain separate.

## Complete single-GUID indexes

Single GUID keys now use the shared complete tree/row reader and explicit
unique/primary/required checks. A dedicated private profile retains all 128
canonical bits and distinguishes NULL from the all-zero GUID. Native field storage
still uses its existing little-endian integer groups; the physical key uses
canonical big-endian group order. Core Guid gains no Ord implementation and SQL
GUID comparison/ORDER BY semantics remain separate.

The known key has a direction marker followed by two eight-byte segments. The
intermediate length tag is 09 even descending. The final tag is 08 ascending and
F7 descending; only data bytes and the final tag are complemented. Strict segment
tags, exact widths, reconstructed prefixes and row agreement apply alongside all
existing owner/order/separator/sibling/count/live-root/resource gates. A separate
27-byte maximum entry buffer bounds GUID keys plus row/child references; numeric
single/composite buffers retain their earlier sizes. NULL uses the normal single
marker. Typed integer seeks still reject GUID indexes; GUID-containing tuples now
have the [separate compound profile](#complete-guid-bearing-composite-indexes).

The reviewed Jackcess 5.0.4 GuidColumnDescriptor, general binary entry writer and
ColumnImpl GUID writer establish the independent encoding and byte-order boundary.
The [pinned IndexData source](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
and [ColumnImpl source](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java)
are research references; no upstream implementation was copied. The actual reviewed
artifact and original fixtures provide comparison evidence, not Microsoft Access
qualification.

The original [GUID index recipe](testing/GuidIndexFixture.java) creates fresh inert
V2016/V2019 files with 1,000-row ascending/descending trees, unique/primary keys,
group/endian boundary patterns, repeated NULLs, zero/high GUIDs and empty indexes.
Generation asserts duplicate and primary-NULL rejection.
[Independent comparisons](testing/compare_guid_indexes.py) match 2,034 ordered rows
per family, complete checks, required-index NULL filters and buffered rejection of
corrupt selected roots. The original empty GUID composite now also qualifies.
Oracle text is rendered in
the CLI's canonical lowercase spelling; this does not add a SQL case policy or
change the earlier default-cursor case observation. Original hashes remain
unchanged and are recorded in [provenance](FIXTURES.md#original-ace-16-guid-index-inputs).

Five CI tests include a fixed-seed 256-case arbitrary-byte ordering model, both
directions, prefixes crossing both segments/row references, NULL/zero distinctions,
exact uniqueness/required rules, malformed tag/key/width/profile rejection and
shared budgets/cancellation. The Int32 editor still rejects GUID targets and all
ACE writes; a synthetic unrelated Jet 4 Int32 edit preserves the entire GUID
index and every byte outside its exact field. Optional [GUID engineering write previews](testing/ACE16_WRITE_PREVIEWS.md#guid-index-write-previews) now exist separately. GUID seeks/production mutation,
general binary/text/float/date/Decimal indexes and native Access remain
unqualified.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/GuidIndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/GuidIndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_guid_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

## Complete GUID-bearing composite indexes

Complete indexed reads/checks now support two-to-ten numeric/GUID components in
any native key order, with independent directions and exact NULL shapes. Each
position has a fixed declared carrier. Private mixed atoms retain canonical GUID
bytes and exact signed numeric/scaled-Currency values separately; cross-type
ordering is never used. Core Guid gains no Ord implementation or new SQL semantics.
Integer range APIs still require their single matching-width column.

Composite models, column codecs and profiles now have focused modules. Static
profiles retain the compact i64 numeric tuple and existing 17/27/98-byte single-
numeric/single-GUID/numeric-tuple entry buffers. Only GUID-bearing tuples use the
wider atom representation and a separately derived 198-byte entry buffer. The
single and compound GUID paths share segment validation. Actual allocation sizes,
component work and existing read/row/depth/cancellation ceilings remain charged.
Unique tuples exempt only entirely NULL keys; primary/required indexes reject a
NULL in any component. Numeric and GUID zero values remain real keys.

The original [GUID tuple recipe](testing/GuidCompositeIndexFixture.java) generates
fresh inert V2016/V2019 inputs with reordered mixed GUID/Currency/Int64/Int16 keys,
ten-GUID keys, both directions, duplicates, NULL shapes, unique/primary tuples and
empty/explicitly unqualified profiles. Generation asserts exact and partial-NULL
duplicate rejection and primary rejection for NULL in every key position.
[Independent comparison](testing/compare_guid_composite_indexes.py) matches 4,512
ordered rows/profile, required-index NULL filters and complete checks. The ten-GUID
trees have nine nodes and 96 leaves across three indexes, maximum expanded node/
leaf entries of 198/194 bytes and observed prefix 19. Depth-two roots list their
final child explicitly with a zero separate tail; the shared traversal fix now
handles that valid layout, while its key mutation remains blocked.

Six CI tests include an independent 256-case typed tuple model, all five numeric
carriers mixed with GUIDs, native key permutations, ten-GUID keys and a 193-byte
prefix crossing the row reference. That long prefix is synthetic, not observed
in the independent files. Zero/NULL and partial-NULL uniqueness, malformed tags/
keys, integer-seek/resource rejection and edit boundaries remain explicit.
An unrelated qualified Jet 4 Int32 edit preserves every byte outside its exact
field and the entire GUID composite index. Any affected composite key and all ACE
writes remain blocked. Input hashes stay unchanged; provenance and canonical row
hashes are in [FIXTURES.md](FIXTURES.md#original-ace-16-guid-tuple-index-inputs).

The [benchmark runner](testing/benchmark_scans.py) compares frozen prior/current
numeric and single-GUID workloads plus complete sequential/compound operations.
Raw samples are recorded under [native benchmarks](testing/NATIVE_BENCHMARKS.md).
Optional [GUID tuple write previews](testing/ACE16_WRITE_PREVIEWS.md#guid-index-write-previews) now exist separately. General binary/Text/float/date/Decimal key encodings, GUID composite seeks/production mutation,
native Access comparisons and application save/reopen remain unqualified.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/GuidCompositeIndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/GuidCompositeIndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_guid_composite_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

## Complete numeric composite indexes

`read_table_index` and `check_table` now support two-to-ten-component numeric
tuples, mixing Byte/Int16/Int32/Int64/Currency under each field's existing profile.
Key columns map by native logical identity, independently of table display order.
Each component retains its direction and NULL marker; tuple comparison is
lexicographic in those directions, followed by the unsigned physical row reference.
Every returned row must reproduce all decoded components in the exact declared
carriers. These private physical policies do not introduce SQL coercion or ordering.

Page prefix/bitmap/header validation and bounded tree traversal have one shared
implementation for single and compound keys. The single-key path keeps its
17-byte maximum entry buffer; the compound path has a separately derived 98-byte
maximum (ten marker/eight-byte components plus row/child references). Compact
tuples retain a separate presence mask. Actual entry/vector sizes are charged
before allocation, component work is bounded/charged, and depth/read/row ceilings
and cancellation still apply. Typed Int32/Int64 seeks reject composite indexes
even for All/Null; complete required-index scans retain residual query filtering.

Unique indexes allow repeated entirely NULL tuples and reject equal partial-NULL
or complete tuples. Primary/required indexes reject NULL in any component. Shared
primary backing semantics remain. The reviewed Jackcess 5.0.4 IndexData descriptor
and insertion checks confirm per-component encoding and these independent tuple
rules; no upstream implementation was copied. Native Access remains unrun.

The original [composite recipe](testing/CompositeIndexFixture.java) creates fresh
inert V2016/V2019 inputs with five mixed-width and ten full-width components,
ascending/descending/mixed directions, reordered key columns, partial/all NULLs,
duplicates, unique/primary pairs and empty/explicitly unqualified profiles.
Generation asserts duplicate/partial-NULL unique and any-NULL primary rejection.
[Independent comparison](testing/compare_composite_indexes.py) matches 4,513
ordered rows/profile and required-index residual queries, with actual node trees
and complete checks. Ten-column trees have three node/45 leaf pages across their
three indexes, observed maximum expanded node/leaf entries of 98/94 bytes, and
maximum prefix two. The [fixture record](FIXTURES.md#original-ace-16-compound-numeric-index-inputs)
retains file and canonical row hashes. Corrupt selected roots and Text/ignore-NULL
profiles fail without partial stdout or sequential fallback; original files remain
unchanged.

The [12-file ACE16 comparison](testing/compare_ace16.py) additionally compares all
1,000 original two-Int32 Composite rows/profile, with row hash
`9e0cba9b3eb34112b294d09a525ea4df89ec431d7cf8d34ff08c9bad13c10f93`.
Seven synthetic tests include a fixed-seed 256-case nullable tuple model,
all five carriers, key permutations, compression through a 93-byte prefix,
unique/required rules, type/profile/corruption/resource rejection and every I/O/
cancellation failure. The 93-byte prefix is a synthetic golden; it was not observed
in the independent files. Jet 4 composite key mutation stays blocked; otherwise qualified
non-key Jet 4 Int32 edits have an exact unchanged-index/byte proof. All ACE writes
remain blocked. ACE 16 [integral tuple previews](testing/ACE16_WRITE_PREVIEWS.md#integral-composite-index-updates)
now supply separate existing-page mutation evidence. The CLI labels constraints
`Checked: exact unique/required`.

The [performance record](testing/NATIVE_BENCHMARKS.md) compares the frozen prior
build and current single-key controls, plus current sequential/compound workloads.
Text/float/date/Decimal index encodings, composite seek bounds, automatic
index selection, key mutation and native Access qualification remain separate.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/CompositeIndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/CompositeIndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_composite_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

## Native Int32 range selections

`read_table_index_range` accepts `Int32IndexSelection::All`, `Null`, or `Range`
with standard-library `Bound<i32>` endpoints. This API requires an Int32 key,
including `All`/`Null`; Int64 full reads use `read_table_index`. No Int64 range API
is implicit in this typed Int32 entry point. Bounds use ascending numeric order for both index
directions; numeric ranges exclude NULL, and reversed/empty ranges
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
empty-table reads return zero rows; Text/ignore-NULL selections reject. Numeric
composites now have the separate [complete tuple qualification](#complete-numeric-composite-indexes)
below, including the existing original two-Int32 recipe.
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

[ACE 16-family evidence](NATIVE_ACE16.md) now qualifies the bounded existing read
algorithms for unencoded code-5/code-6 files. Feature/type restrictions remain.

## ACE 16 Int64 index qualification

Complete single-Int64 reads/checks now use width-qualified signed integer keys,
without floating conversion. ASC/DESC value and NULL markers, sign-bit encoding,
compressed prefixes and row/child references retain the existing validation.
Native scalar eligibility gates Int64 to code-5/code-6 fixed eight-byte fields.
Unique/required checks retain exact full-range keys, including adjacent values
beyond double precision and repeated NULLs in ordinary unique indexes.

The original [Int64 fixture recipe](testing/Int64IndexFixture.java) creates fresh
V2016/V2019 files with 600 mixed nullable/duplicate/extreme values, ascending and
descending indexes, a unique table and an empty table. The
[comparison runner](testing/compare_int64_indexes.py) matches 1,212 ordered rows
per file to independent cursors, verifies both Int64 roots are real node pages,
checks every supported index and tests exact large-value/NULL CLI filters.
Input hashes are in [fixture provenance](FIXTURES.md#original-ace-16-int64-index-fixtures).

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/Int64IndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/Int64IndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_int64_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ranges target/release/examples/index_range \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

Synthetic CI goldens cover both directions/profiles, exact row agreement,
constraint errors, unsupported Int32 range calls, prefix reconstruction,
corruption, limits/cancellation and buffered CLI failures. Existing Int32 seeks
remain covered. Text and other unqualified key trees and Int64 index mutations
remain separate work. These are independent-reader observations, not native
Microsoft Access conformance.

## Native Int64 range selections

`read_table_int64_index_range` accepts `Int64IndexSelection` with exact
`Bound<i64>` endpoints. `All`, `Null` and numeric ranges share the existing
bounded traversal/pruning and partial-validation contracts. The Int32 and Int64
APIs reject each other's key widths even for `All`/`Null`. No floating conversion,
endpoint arithmetic or saturation occurs; Int64 extrema and adjacent values
beyond double precision remain distinct. The existing Int32 API is unchanged.

Synthetic two-level trees split duplicate large keys across a separator and
exercise both directions/profiles, NULLs, inclusive/exclusive/unbounded/empty
ranges, wrong-width calls, corruption, cancellation and limits. A 256-case
fixed-seed numeric model checks arbitrary signed Int64 endpoints. A seek succeeds
with an unvisited corrupt leaf and never reads it; a full read or visited corrupt
leaf fails. This demonstrates pruning, not validation of skipped pages.

The original V2016/V2019 fixtures each match 24 direct selections against the
independent ordered cursor output in addition to the existing 1,212-row checks:

```sh
cargo build -p tablecraft-ace --example index_range --release --locked
python3 docs/testing/compare_int64_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ranges target/release/examples/index_range \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

The Access adapter now also accepts matching exact Int64 query constraints under
the [whole safe-filter rules](QUERY_EXECUTION.md#safe-source-constraints). Other
predicates retain the required complete index path. The runner compares 20 matching
query intervals per format as well as direct selections; native Access comparison
remains unrun.

## Complete exact Currency indexes

Complete `read_table_index`/`check_table` now accept single Currency keys. Private
key metadata distinguishes Currency from Int64 despite their equal encoded width;
typed Int32/Int64 seek APIs reject Currency even for All/Null. A shared carrier
extractor validates the declared type and retains the signed scaled integer.
Nothing passes through floating point or silently changes the result carrier.
Existing signed big-endian/sign-bit key decoding, descending complements, NULL
markers, compressed prefixes, node bounds and exact row-reference checks apply.

Microsoft documents [MONEY's eight-byte scaled integer representation](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types).
The reviewed Jackcess 5.0.4 artifact's IntegerColumnDescriptor and currency writer
confirm the independent key encoding and exact scale-four carrier. Its source
reference above remains format research, without copied implementation or native
Microsoft Access qualification.

The original [Currency recipe](testing/CurrencyIndexFixture.java) generates fresh
inert V2016/V2019 files: 700-row ascending/descending node trees, unique/primary
tables and empty indexes. BigDecimal inputs retain signed extrema, adjacent
ten-thousandths, values beyond floating-point integer precision and NULL. Generation
asserts unique/primary duplicate and primary-NULL rejection.
[Independent comparisons](testing/compare_currency_indexes.py) match 1,426 ordered
rows/profile, complete native checks and NULL/negative/nonnegative SQL filters.
The normalized row SHA-256 is
`9ad67935b4dad7454b4d920e497c856c477d705f560d371c988b5431455a9487`
on each family; [fixture records](FIXTURES.md#original-ace-16-currency-index-inputs)
retain observed file hashes. No original input changes.

Synthetic goldens and a fixed-seed 256-case exact ordering model cover all i64
scaled values, compression, duplicate/NULL rules, empty/corrupt trees, mismatched
keys, budgets/cancellation and strict integer-range rejection. ACE write gates
still reject all ACE updates, and the Int32 editor rejects Currency targets.
A synthetic Jet 4 regression confirms that newly checked, unaffected Currency
indexes permit an otherwise qualified Int32 edit while retaining every Currency
index byte and all bytes outside the exact Int32 field. This is an unchanged-byte
proof, not native Access save/reopen evidence. Exact Currency foreign keys use
the separate [membership audit](NATIVE_CONSTRAINTS.md#exact-currency-relationship-membership).
Currency seek bounds, key mutation, Text and other unqualified indexes and native Access remain separate.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/CurrencyIndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/CurrencyIndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_currency_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

## Byte and Int16 complete indexes

Complete reads/checks now support single-Byte and single-Int16 indexes. Byte
payloads remain unsigned with no sign-bit flip; signed Int16 keys use a sign-bit
flip over two big-endian bytes. Descending value bytes are complemented. Markers,
NULL ordering, compressed prefixes, row/child references, uniqueness/required
checks and all existing structural/resource gates remain shared. The reviewed
Jackcess 5.0.4 Byte/Integer column descriptors and the IndexData source reference
above provide independent format evidence; no implementation was copied.

The original [narrow integer recipe](testing/NarrowIndexFixture.java) creates
separate 700-row Byte/Int16 tables with ascending/descending node trees, unique
tables with boundaries/repeated NULLs and empty indexes. Generation asserts
duplicate rejection. [Independent comparisons](testing/compare_narrow_indexes.py)
match 3,067 ordered rows/profile, unsigned 0–255 and signed extrema, SQL filters
and complete native check coverage on V2016/V2019. The original cursor oracle
now normalizes Java's signed Byte carrier to Access's unsigned value. Original
file hashes remain unchanged.

Synthetic goldens cover both directions/profiles, compressed duplicate entries,
exact row agreement, uniqueness/required failures, corruption, empty tables,
limits/cancellation and 256 generated signed/unsigned key vectors. CLI tests retain
the exact carriers through complete required-index scans and SQL filtering.
The typed Int32/Int64 seek APIs still reject these widths even for All/Null;
narrow seeks, narrow key mutation, Text and other unqualified indexes and native Access
conformance remain separate work.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/NarrowIndexFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/NarrowIndexFixture.java new-2019.accdb V2019
python3 docs/testing/compare_narrow_indexes.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```


## Additional fixed-key engineering writes

Opt-in [Boolean, IEEE and legacy DATE previews](testing/ACE16_WRITE_PREVIEWS.md#boolean-ieee-and-legacy-date-index-write-previews)
now maintain eligible existing indexes through the shared batch writer. This
supersedes earlier blanket target-mutation limitations for those engineering
entry points; ordinary ACE saves and SQL editing remain unchanged. Exact stored
bits/private physical ordering, complete constraints and existing-page capacity
remain mandatory. Relationship actions for these carriers, allocation/splits,
native Access and application-save qualification remain separate work.


## Decimal engineering write boundary

The opt-in ACE 16 preview now reuses the checked tree writer for qualified single
and mixed Decimal indexes, with exact field precision/scale and existing pages.
Stored-field word order remains separate from sorted key encoding. This does not
change Jet 4's legacy Decimal reads or enable its Decimal writes. The
[write contract](testing/ACE16_WRITE_PREVIEWS.md#decimal-slot-and-index-write-previews)
and [fixture evidence](FIXTURES.md#decimal-slot-and-index-update-inputs) distinguish
these engineering candidates from ordinary ACE saves/native Access qualification.


## Explicit projected index snapshots

`read_table_index_projection` takes an `IndexProjection` containing ordinary
output names and `IndexProjectionSelection::All`, `Int32(...)` or `Int64(...)`.
All retains existing qualified complete index profiles; typed selections require
the matching single-key width even for All/Null. Output stays in native logical
order. Missing/duplicate/unavailable selected fields and ineligible indexes fail.

Required key fields join the decode projection even when absent from output.
A borrowed native-position view lets the existing scalar/GUID/Decimal/composite
key policies check actual values without padding omitted fields with NULL.
Validated unrequested keys are removed in place before snapshot publication.
Empty output projections still validate every selected entry/key. Ordinary
`read_table_index` keeps its full-column profile and existing key ordering.

Allocation ownership, row/key correspondence, leaf linkage, exact key domains,
foreign-index qualification and complete count checks retain their existing scope.
Range reads qualify only traversed pages and returned records. They do not audit
skipped subtrees or unselected non-key payloads, and need a stable/offline source.
The query adapter uses this for mixed-table scalar views, retaining explicit
index choice and correct mapping from dense query columns to native key fields.
No new carrier, index collation or write profile is enabled.

[Independent comparisons](testing/compare_index_projection.py) use original
1,000-row mixed schemas with native keys after unavailable collection fields,
including NULL/duplicate/endpoints, ascending/descending/composite reads and exact
64-bit ranges. Synthetic tests cover hidden key corruption, node pruning,
zero-width outputs and projected GUID/Decimal/tuple reconstruction. See the
[fixture record](FIXTURES.md#mixed-schema-index-projection-inputs) for hashes,
reference-builder warnings and native Access qualification limits.
