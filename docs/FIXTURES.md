# Candidate Access fixtures

The original, synthetic SQL syntax corpus is recorded separately in
[SQL_PARSER.md](SQL_PARSER.md), including its Microsoft grammar references.
It contains no copied third-party fixture bytes or extracted Access SQL and is
not native Access comparison evidence.

## Access_Example_VBA.accdb

- Path: `docs/examples/Access_Example_VBA.accdb` (user-supplied on 17 October 2026).
- Size: 3,387,392 bytes.
- SHA-256: `3cd1fc88d70bc93909ae6d190fa9692e709d5641b205fd2f66185c905e42cfbf`.
- Host file identification: Microsoft Access Database.
- Status: candidate application fixture; bounded metadata/scalar reading and read-only query tests observed on disposable copies.
- Source: [Access-projects/Access-examples](https://github.com/Access-projects/Access-examples), supplied by the user.
- Verified upstream commit: `d0ffd681d431f38d91c138559763c30df0699c55`.
- Pinned file: [Access_Example_VBA.accdb](https://github.com/Access-projects/Access-examples/blob/d0ffd681d431f38d91c138559763c30df0699c55/Access_Example_VBA.accdb).
- Upstream license: MIT, copyright 2019 Anthony Duguid. The original notice is
  retained verbatim in [Access-examples-MIT.txt](licenses/Access-examples-MIT.txt).
- Verification date: 17 October 2026. The pinned upstream binary was downloaded
  for a read-only comparison and matches the local file byte for byte, including
  the recorded size and SHA-256.

The upstream README describes examples covering VBA, queries, macros, forms,
reports and ribbon XML. This describes intended coverage, not verified local
object contents: VBA and application object contents have not been decoded or executed.
The filename and host file identification do not establish its engine generation,
encryption, schema or object compatibility.

Source identity and the upstream MIT notice are recorded. Keep the upstream notice
with any redistributed copy; the fixture retains its original attribution rather
than the TableCraft source copyright.

The creation recipe, Access/ACE version and a real Access inspection result are
not yet recorded. This third-party application example supplements the original,
recipe-generated minimal fixtures needed for compatibility testing. Qualification
remains incomplete pending those fixtures and oracle evidence. Retain the original bytes and perform
future experiments on copies.

The [application compatibility inventory](testing/ACCESS_APPLICATIONS.md) records
this candidate with application-level operation outcomes unrun. Its corpus policy covers
additional application selection, sanitization, per-profile evidence and native
save/reopen qualification. Header inspection of a disposable copy now reports format code 2/subversion 0,
4,096-byte pages, 827 pages and a zero page-encoding key. See
[NATIVE_READING.md](NATIVE_READING.md). The catalog reader inventories 146 metadata records; the five selected native
metadata fields match an independent Jackcess 5.0.4 read-only comparison exactly.
This does not qualify application behavior, encryption support or native editing.

[Native table evidence](NATIVE_TABLES.md) records schema comparison for 36 tables /
187 columns and 1,488 scalar rows across seven tables against Jackcess.
[Native query tests](NATIVE_QUERIES.md) exercise supported CLI retrieval on copies.
Object/application behavior and Microsoft Access comparisons remain unqualified.

## Original generated relationship inputs

[RelationshipFixture.java](testing/RelationshipFixture.java) is an original
repository-licensed inert recipe using the reviewed Jackcess 5.0.4 artifact.
Compared V2007 output SHA-256:
`5e09e133be78fcb0c39986ba760cb6d97443c0f4cc465e988b3b76cad4d544d5`;
V2000 SHA-256:
`f29a2cc0d211f63aff3b9a5030397dc98960f9a456cc9a5ef2ab2a128e428d1a`.
Fresh metadata can differ. [Relationship evidence](NATIVE_RELATIONSHIPS.md#references-and-evidence)
records five relationships/six pairs per format and the observed composite-NULL
comparison limitation. Generated files/JARs are not tracked or CI dependencies.
Original candidate bytes remain unchanged; no Access execution evidence is claimed.

## Original generated property inputs

[PropertyFixture.java](testing/PropertyFixture.java) is an original repository-
licensed inert recipe using the previously reviewed Jackcess 5.0.4 JAR. It refuses
existing output paths; generated files/JARs are not tracked or CI dependencies.
Observed ACE V2007 output SHA-256:
`32276f9fc8190f69691f213c3000b2fe96b02bc24448cf1e7ac94dc310d01720`.
Observed Jet 4 V2000 output SHA-256:
`393ee170a8cd51275ca3e133bb5327228654b1fb534edb632aea42c7efdad6b3`.
Fresh generated metadata may differ. All 22 declarations match Jackcess;
[scope and canonical comparison hash](NATIVE_PROPERTIES.md#references-and-independent-comparison)
record coverage. Original registered bytes remain unchanged. No Microsoft Access
runtime comparison or native save is implied.

## Original generated index-constraint inputs

[ConstraintFixture.java](testing/ConstraintFixture.java) is an original repository-
licensed inert recipe using the separately reviewed Jackcess 5.0.4 artifact. It
refuses existing output paths, verifies expected constraint rejections, and offers
a read-only --inspect mode. Generated databases/JARs are not tracked or CI inputs.

Observed V2007 file: 389,120 bytes, SHA-256
`8dbe8d5b1ef1b8747a4c760e22be419077814ee562716abbd86479cb611f4b95`.
Observed V2000 file: 159,744 bytes, SHA-256
`17bf714ee4bcfa7fba171f64756dbbe2eba832d8a5a78a012876b69edc981ea3`.
These identify the disposable comparison inputs; fresh metadata can differ.
Both formats match all nine tagged rows: UniqueValues hash
`4345e67c72047e4af1365126453f1c17d609adbb86dcac7ab04b3ead4cb460fc`,
RequiredValues `81c043e10988f8af45c74c2e5cba757ed275b7adbb7d7e9dbcbaae46e48c97fa`,
PrimaryValues `5528b4670cae4fb431c27c63319e8b9d816910f892818fa641e0ed3a5581d5f9`.
[Constraint evidence](NATIVE_CHECKS.md#qualified-index-constraints) records scope;
native Access remains unrun and the registered original remains unchanged.

## Original generated chained-definition inputs

[DefinitionFixture.java](testing/DefinitionFixture.java) is an original TableCraft
engineering recipe under the repository license, using the separately reviewed
Jackcess 5.0.4 artifact (Apache-2.0; identity in NATIVE_READING.md). It refuses to
overwrite an existing path and creates only inert Wide/EmptyWide tables. No third-
party fixture or implementation is copied; generated binaries and JARs stay local.

The observed disposable V2007 output is 421,888 bytes, SHA-256
`463a7ad91d45c568c00badc700ac344ea6a5dc9ee69a41f4eb4f0a5094356949`;
V2000 is 192,512 bytes, SHA-256
`f0d4529ab0fcb45eb3598656ae3f82dfa202d1ff155fac1c106c75b03bdc012d`.
Fresh files can differ in generated metadata; these hashes identify the measured
inputs, not a deterministic-output promise. [Definition evidence](NATIVE_TABLES.md#bounded-chained-table-definitions)
records the schema/index/row comparisons. Original registered fixture bytes remain
unchanged; no native Access qualification is implied.

## Original generated usage-map inputs

[UsageMapFixture.java](testing/UsageMapFixture.java) is an original repository-
licensed inert recipe using the same external Jackcess 5.0.4 artifact. It refuses
existing output paths. Each generated table contains 2,000 rows with 12 short
Binary fields of 240 bytes, forcing 2,000 owned data pages and indirect usage maps.

Observed V2007 input: 8,568,832 bytes, SHA-256
`db25aa22d4f8fd137a246c7fcbaec3bd8f81e8dbfe0d697b3586d5a301b61814`.
Observed V2000 input: 8,339,456 bytes, SHA-256
`0dbeac6855be9a718308879eabfba577242424ee49794dee48266cda1e64c77e`.
Hashes identify measured disposable files; fresh metadata may differ. No generated
binary/JAR is tracked. [Usage-map evidence](NATIVE_READING.md#usage-map-ordering-and-ownership-bounds)
records the full tagged-row comparison and larger engineering budget. The normal
CLI limits, original fixture and lack of native Access qualification are unchanged.

## Original validation-rule fixtures

[ValidationFixture.java](testing/ValidationFixture.java) is an original MIT recipe
using the reviewed Jackcess 5.0.4 artifact; generated outputs remain untracked.
The successful V2007 input has SHA-256
`8498dbaf475ded63675e781175f227d3783b8d098da127d19c17664bb3a8915f`;
the V2000 input has SHA-256
`0f61ccd6401c04bb480674574ce1669a490ee19d7e48fecc855bd2dfadc41699`.
[Comparison](testing/compare_validation.py) verifies all six table row counts
and rule-coverage states per format, retaining unchanged bytes. Ranges and Bounds
assert rejection of invalid writes by Jackcess. Nullable explicitly allows NULL.
Calls/EmptyCalls demonstrate unsupported function rules independently of row count.
UnknownNull deliberately inserts NULL before attaching a rule, testing diagnostic
uncertainty rather than claiming ordinary validated writes accept that record.
Native Microsoft Access execution and save/reopen remain unrun.

## Original fixed Int32 update fixture

[UpdateFixture.java](testing/UpdateFixture.java) is an original MIT recipe using
the already reviewed Jackcess 5.0.4 artifact. The untracked V2000 input SHA-256 is
`591b31b94a442d03b57c208f632a1b1a07ebd19fe2bad077db809c0746325332`.
[Comparison](testing/compare_updates.py) changes Score 20 to 27 on a fresh copy,
whose SHA-256 is
`15f3b4da6b39d35d2c578a211b562802fe81f42f0dc313f9455e757dafd38740`.
Exactly one byte changes. Three rows retain Unicode memo and 12 KiB binary
payloads; Jackcess row reopen and native primary-index checks pass. Invalid rules,
stale values and unqualified indexed-field updates originally produced no output. This is independent
engineering evidence, not native Microsoft Access compatibility.

A subsequent fresh input from the same recipe has SHA-256
`89df2b3abcea00499f89681e19a6b3e1b244028ed3b245241c5ecd63fb35f8ee`.
The current runner verifies the unchanged Score case and qualified indexed Id
2-to-27 replacement. The indexed copy SHA-256 is
`27201a30ea9cd3a0f1c093df8ec2f3d29072c3f5b682a5ee4c817f41ef2cea50`.
Jackcess's public index cursor returns 1/3/27; memo/binary records reopen unchanged.
Only the selected fixed field and existing root entry-data span may differ.
Native/CLI outputs match; duplicates, rules and stale values publish no file.
The original input remains unchanged. This does not qualify ACE writes or native
Microsoft Access save/reopen.

## Original existing-tree index update fixture

[IndexUpdateFixture.java](testing/IndexUpdateFixture.java) is an original MIT
recipe using the same reviewed Jackcess 5.0.4 artifact. Its untracked V2000 input
SHA-256 is `4c2db844ba947bd386e760ed2d8c631727b78211a9df4c0bfe1d0df508a3c2e8`.
There are 1,000 manual Int32 keys, ascending primary and descending unique node
trees, and Unicode memo/binary records including long external payloads.

[The comparison](testing/compare_index_updates.py) independently checks row/index
order, memo/binary digests, native/CLI equality, exact byte footprints and duplicate
rejection. Disposable output SHA-256 values, against that exact original:

| Change | SHA-256 |
| --- | --- |
| First key 2 to 2001 | `749cdbbc2ec718761f1209f809fc3df844313097049fb051be91e114a361d3cc` |
| Last key 2000 to -1 | `d60352779ae09ac92010c8dfb0f46ba394b4785662d129172aab13274f7b87ca` |
| Middle key 1000 to 1999 | `5768890c4bd8cd2d616b35d7c773e75b9a830ea0364cd5d141111c16cf598ab5` |
| All keys replaced by 2002 minus original | `df54d9544b4b8103f1f630b9c8a509c04b09d3d99b0648a0161d1d0f8c0c0696` |

Both roots have three leaves; page/allocation/header/bitmap/padding bytes remain
identical. All experiments use fresh outputs and the original hash remains
unchanged. No Access-generated fixtures or native Microsoft Access comparison
are implied; ACE writes remain blocked.

## Original ACE 16-family read fixtures

[ACE 16 comparison](NATIVE_ACE16.md) reuses the original MIT Scalar/Index/Property/
Relationship/Validation/Definition recipes with reviewed Jackcess 5.0.4 V2016 and
V2019 templates. Generated files remain untracked; they are independent engineering
fixtures, not Microsoft Access-authored files. Observed input SHA-256 values:

| Recipe | V2016 | V2019 |
| --- | --- | --- |
| Scalar | `80249acce429e4a3dc4696c11d9951c1bf0573661163575b5d8c0e74718bfff1` | `0993586b20d3b314482b9807d8c249a41a6ea2c1b8494a3818d554a768edc70e` |
| Index | `ab7f3a3daaaac76b4b0d5a4f91c07e1c840d58c7304f5b1e9784117436da4137` | `135c22a8b3cc52748fa87d4206b97cb5c66cb081b0593725b6da03ac44ec3f38` |
| Property | `6242681095edb0e721197e02e735c0be1fc028687042319db765211745c476e4` | `d5d3ede3a1492b2177d1045a3f5da04f347f657a0531dfbc72f922b78c1a7d89` |
| Relationship | `7a0fe81bbdabcb76cc922e711032772f8144ad41671974fdc1e5343d7b0f6f46` | `1304f6fd954349967afac3d94883007c10e25dc6b89c5b19b18c88dcfa3ddeb7` |
| Validation | `73168e84dcba2f2b446e5d13ef6f73dd07b32382f7cbcc78e0ca9cd296a6af33` | `75bb86338b07483b05c036259c6afb1bd7930b65145a362686a321a3d8b692d2` |
| Definition | `cf4cbd5bef503aff0a7e29004d25d269dc2fb05844cacdc5c6a138274846eb1d` | `0d5390ff251668b8bc7f65d1bb435b582787c156d83f7a4875127fa3d6b1db0b` |

All twelve hashes remain unchanged after catalog/scalar/property/relationship/
constraint/index/chained-definition comparisons. V2019 adds built-in navigation
relationships retained by both the engine and the complete independent inventory.

## Original ACE 16 modern scalar fixtures

[ModernScalarFixture.java](testing/ModernScalarFixture.java) is original MIT
repository test code, generated with the reviewed Jackcess 5.0.4 artifact above.
Fresh V2016/V2019 files contain Large Number extremes and values beyond double
precision, NULLs, a physical BigInt index and an empty calculated-column table.
V2019 additionally contains extended dates spanning years 1–9999 and 100ns ticks.
No Access application code is executed. Generated files remain untracked.

| Profile | Input SHA-256 | Tagged row SHA-256 |
| --- | --- | --- |
| V2016 (7 rows) | `41ca0f872f07ded32da69f8f7da9fcd978c8c9558c6596be9389c95917ae24f8` | `501a0e419d8cddd5d2f9349d095f2154e51602ad893991f1b65bc0377ac3446a` |
| V2019 (13 rows) | `e829d4875d8a2d25df43eba2f94200c492f396a83f26ac17b921b30f6f47b45e` | `c1f1d1f9c7916f057abdd9b9e0fe0d55e86d01018c953d97f02fd1c67dd7ef81` |

[The comparison runner](testing/compare_ace16_scalars.py) verifies original family
headers, exact independent tagged carriers, CLI query behavior, complete BigInt
index checks, explicit calculated-column errors and unchanged input hashes. Synthetic Rust
fixtures separately test profile/width gates, malformed timestamps and CLI output
atomicity. Neither generated nor synthetic fixtures establish native Access
conformance; a Microsoft Access oracle remains unrun.

## Original ACE 16 advanced field metadata

[AdvancedFieldFixture.java](testing/AdvancedFieldFixture.java) is original MIT
repository test code generated with the reviewed Jackcess 5.0.4 JAR. Fresh
V2016/V2019 files include calculation metadata, multivalue/attachment child schemas,
append-only version-history fields, rich-text/Hyperlink/input-mask properties and
AutoNumber markers. They have no active application code and remain untracked.

| Profile | Original input SHA-256 |
| --- | --- |
| V2016 | `84b35fd0d7c4c22a83d0a5b507f71ed6c782df7edbb9fc0f75d17da2864cadbc` |
| V2019 | `03dacb57f6613a0139ce9811de25aae29dc9e3b289677396f5815d76cf49db15` |

[The comparison](testing/compare_advanced_fields.py) matches ten columns/profile
and rejects scalar queries on empty calculated/complex tables with no partial
stdout. Normalized declaration digest for each profile is
`86a60b47dbf0879e948a1c64ea3931611c32ff4e7af155ef1438644cb0460316`.
Complex presence is independently matched; raw IDs use synthetic golden bytes.
All inputs remain unchanged. This qualifies inert metadata comparisons only,
not structured child-value decoding, calculation, native saving or Access behavior.
Generation is fresh-output-only; incomplete earlier generation experiments were
not reused as qualified fixtures. Repeat generation with a fresh path and the
V2016/V2019 argument, then run the comparison with --jar/--cli/--ace2016/--ace2019.

The advanced-field comparison additionally resolves three complex schemas/profile
through the independent reader's public complex API and system declarations.
Normalized exact reference-row SHA-256 is
`b3e28d17d6a6fd865763cb72012c857c36c90028275eb68d801222bbbb62cdc1`
for V2016 and
`164017af842c53cd0bdbb55205b42dbaebc916b2524eba6d477c6ad87ba53fe9`
for V2019. Original file hashes above are unchanged. Mapping resolution is separate
from the earlier presence-only field comparison and from child-value decoding.

## Original populated complex-child storage fixtures

[ComplexRowsFixture.java](testing/ComplexRowsFixture.java) is original MIT test
code using the reviewed Jackcess JAR, creating fresh V2016/V2019 files with three
parents and three rows each for multivalues, encoded attachment storage and
version history. Histories use explicit fixed timestamps; parent Notes start NULL.
The generated child table names are canonicalized for comparison digests. No VBA
or application content is executed; generated files remain untracked.

| Profile | Original SHA-256 |
| --- | --- |
| V2016 | `b45ac9491991903414d415473e3481bf34f5b5de201f25ff73ef197110af4379` |
| V2019 | `0886d900287680fd2acec8302d08b1df15be11a1b545a0b006ecb80eb6e72fd1` |

Both nine-row normalized tagged outputs have SHA-256
`385407dfff6c995502203cb34e465b712f9dbaedffccc2f34d5548973bd10fdf`.
[The runner](testing/compare_complex_rows.py) checks exact independent raw rows,
CLI counts, unchanged inputs and continued rejection of parent collections as
scalar snapshots. Raw attachment bytes are encoded storage, not extracted file
content. These are independent-reader comparisons, not Microsoft Access evidence.
An earlier failed history-deletion experiment was abandoned; it is not a fixture
or a supported TableCraft/native history-deletion claim.

The recipe's read-only `--links` mode independently reports scoped parent keys
and child counts. [Link comparison](testing/compare_complex_links.py) matches nine
parent links per family across all three complex columns and rejects twelve
corrupt disposable copies/profile without partial CLI output. Sorted normalized
link output SHA-256 is
`7ed873ac68964513734c4006689fe5828dc89cbabfbe13c325f9fe24c941ff92`
for each family. Existing original database hashes remain unchanged; no new
third-party input or extraction/conformance claim is introduced.

## Original ACE 16 composite constraint fixtures

[CompositeConstraintFixture.java](testing/CompositeConstraintFixture.java) is
original MIT fixture code using reviewed Jackcess 5.0.4. Fresh files contain six
parents, six many-to-one children and three one-to-one children, with exact partial
and all-NULL cases. Generation asserts native-independent acceptance/rejection
behavior without executing VBA/application content. Files remain untracked.

| Profile | Original SHA-256 |
| --- | --- |
| V2016 | `6014b0ee7dfed632f96af9bab40cc24e8ce4d1014e58cdd3dca20622576cf475` |
| V2019 | `06ed6bccf6bad36fbe53e9c728fe076468b818be824691f0ad5012ed4bcedf66` |

Both fifteen-row tagged outputs have digest
`edeeaec8929864afe12f16d3ba2b687a35eca0f1949839167b9cc26d885a77a7`.
[The runner](testing/compare_composite_constraints.py) checks exact independent
records and audit coverage, then verifies four corrupt disposable copies/profile:
missing parent, unmatched partial-NULL child, duplicate parent and duplicate
one-to-one child. Original hashes remain unchanged. Physical composite indexes,
cascade writes and actual Microsoft Access behavior remain separate qualification.

## Original ACE 16 predicate validation rules

[PredicateRuleFixture.java](testing/PredicateRuleFixture.java) is original MIT
fixture code generated with the reviewed Jackcess JAR. Fresh V2016/V2019 files
contain six tables for forward/reversed BETWEEN, IN/NOT IN, empty text rules and
an existing unresolved NULL membership result. Generation asserts independent
valid/invalid numeric writes; the uncertain NULL rule is attached after inserting
the probe record rather than asserted as accepted. No active project code is run.

| Profile | Original SHA-256 |
| --- | --- |
| V2016 | `555dceb34dd3226f4f31171324489b1b1d8193618f28ed579b2ce6f951d1da9a` |
| V2019 | `29ef769ac4d1abe4cb973578fa39b283a6eea82fbc8e2c25e804f492579aeb41` |

Both thirteen-row tagged outputs have digest
`f6a86d4274618970448f991677df8d0cf6954b173ee0132559e4c8bfdec0c34e`.
[Comparison](testing/compare_predicate_rules.py) checks exact independent rows,
all six coverage reports and unchanged originals. Text/unknown NULL remain
unsupported, not silently accepted. These are independent-reader fixtures;
native Microsoft Access remains unrun. Generated files remain untracked.

## Original ACE 16 Int64 index fixtures

[Int64IndexFixture.java](testing/Int64IndexFixture.java) is original MIT repository
test code generated with the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019
files contain 600 records, real node/leaf Int64 trees in both directions, duplicate
and NULL keys, exact integer extremes/adjacent values beyond double precision,
a 12-row unique table and an empty table. Generation verifies duplicate rejection.
No active Access content executes; generated files remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `408aa138ace738662bec60bf56894a22b963211a0e81eb99140993185d85a25a` |
| V2019 | `f1d9ff999901e626c40eb2adafaf1e3ffdef172bc2b9a979c72c7a1831fe0bd7` |

[The runner](testing/compare_int64_indexes.py) verifies 1,212 ordered cursor rows
per file, 24 direct exact Int64/NULL/range selections, 20 additional matching query
intervals, full native check coverage,
NULL/precision-sensitive CLI filtering and
unchanged inputs. Normalized ordered output SHA-256 is
`c336eebf5d61dd4a7260baf018be4eb2541fb510138488bca8ddcd9e1acd00c5`
for each family. The earlier 2,400-row laboratory probe exceeded the unchanged
CLI check allocation budget; these 600-row files still prove actual node/leaf
traversal. No native Access qualification is inferred.

## Original ACE 16 Byte and Int16 index fixtures

[NarrowIndexFixture.java](testing/NarrowIndexFixture.java) is original MIT test
code using the reviewed Jackcess 5.0.4 artifact. It generates 700-row Byte/Int16
tables with real ascending/descending node trees, separate unique tables including
unsigned/signed boundaries and repeated NULLs, and empty tables. Duplicate inserts
are independently rejected during generation. Files remain untracked and inert.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `2ef10c1fc28362b6c57fe085b7178674a9e33be4ad779bcada65435566c7f4d5` |
| V2019 | `7c56a90e64b55e271f35836d3aad9b9250ed1d70c9acd98bb9cc721d7219cb97` |

[The runner](testing/compare_narrow_indexes.py) matches 3,067 ordered rows/profile
and SQL filters against public independent cursors; native complete checks cover
all supported indexes. Java Byte values are rendered unsigned. Tagged ordered-row
SHA-256 is `94c719e52cd6ba4d77b7bd7f350b25b6474727f5c2ddeac9c1d1a0c29596be34`
per family. Input hashes remain unchanged. Narrow ranges/writes and actual
Microsoft Access conformance remain unqualified.

## Original ACE 16 GUID relationship fixtures

[GuidRelationshipFixture.java](testing/GuidRelationshipFixture.java) is original
MIT test code using the reviewed Jackcess 5.0.4 artifact. Fresh inert V2016/V2019
files contain six tables/32 rows with GUID/Int64 tuples, exact boundaries, NULLs,
all-zero GUIDs and one-to-one declarations. Generation asserts duplicate/orphan
rejection, including alternate GUID input case for duplicates. Files stay untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `760ead3e7b7ca568343092fc54edc9d25775d801caf65165f6154699be5f3dbb` |
| V2019 | `c3b9571cf1640608cac3717b2ea7d44b4176b8623f2260d2ab077f6b977ada8c` |

[The runner](testing/compare_guid_relationships.py) matches 32 exact tagged rows
and six audits/profile, six corrupted-copy failures and all-NULL exemption.
Tagged-row SHA-256 is `f9990ac3c56a760c6b6a4057b2399b10de556789f974475974ab463ac26b3196`
for both families. It verifies immutable source hashes and retains a read-only
default-cursor case probe: Jackcess returns false for lowercase and true for
canonical uppercase GUID lookup. Canonical inputs avoid that observed oracle
limitation; an initial partial generation is not qualified. This does not establish
GUID physical-index ordering, native Access, cascades or write compatibility.

## Original ACE 16 Byte and Int16 relationship fixtures

[NarrowRelationshipFixture.java](testing/NarrowRelationshipFixture.java) is original
MIT code using reviewed Jackcess 5.0.4. Inert fresh V2016/V2019 databases each
contain nine tables/59 records, separate Byte/Int16 keys, mixed tuples, one-to-one
declarations, partial/all NULLs and signed/unsigned boundaries. Generation verifies
invalid insert rejection; databases stay untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `d73beac6fcadd1e475a6f37f2d49ca4087137167acfb6767e3d387dfc50bdfe5` |
| V2019 | `cbbe94586c1a28812d7c76f8cff4f84ae5a5d9256fdf860ca93c95380233396e` |

[The runner](testing/compare_narrow_relationships.py) matches all 59 typed rows
and nine audits, with six corrupted copies/profile and a separate all-NULL
exemption check. Tagged-row SHA-256 is
`e0e11f4821a700153e4e341ec8676185d6a33e2a503932e075e18de840517bcc`
per family. Input hashes remain unchanged. No native Access/cascade/write
qualification is implied.

## Original ACE 16 Int64 relationship fixtures

[Int64RelationshipFixture.java](testing/Int64RelationshipFixture.java) is original
MIT repository test code, generated with the reviewed Jackcess 5.0.4 artifact.
Fresh V2016/V2019 files contain six tables and 35 records: exact full-range and
adjacent Int64 values, mixed Int32/Int64 tuples, partial/all-NULL keys and enforced
one-to-one relationships. Generation asserts invalid insert rejection. No active
Access code executes; generated databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `571ce07d06dd2951432a9d76535a0bf07185d4b3f98798c0984ab8fad8b34a87` |
| V2019 | `97f58d77ce9fa2853b3a6299fd24bf430d54c607e2542930770a83607b00f4af` |

[The runner](testing/compare_int64_relationships.py) verifies all 35 exact rows,
six audit reports and four corrupt-copy rejection cases per family. Tagged rows
SHA-256 is `43820899492e9744c2113ae4b33b6aa04247aa51920df24c719c25f9a075e29a`
for each family. Originals remain unchanged. This is independent-reader evidence,
without native Access, cascade or composite physical-index qualification.

## Original ACE 16 stored-calculation fixtures

[CalculatedStorageFixture.java](testing/CalculatedStorageFixture.java) is original
MIT repository test code generated with the reviewed Jackcess 5.0.4 artifact.
It explicitly disables expression evaluation and writes seven known Int32 caches,
including a deliberately stale result, NULL and exact extremes, plus an empty
table. Fresh V2016/V2019 files remain untracked; no active Access code executes.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `7b98d314b4d0ce5d22dee5847751aa0198c3d3e2aca05773a47459ecc9304263` |
| V2019 | `d16d63cc07cb7a356a0fa21c415af668ffc0a95b9e50bd1ad75ef15cb6d99bc9` |

[The runner](testing/compare_calculated_storage.py) verifies all stored results,
empty output, rejection of ordinary query reads and three corrupt disposable
copies per family. Tagged rows SHA-256 is
`b2fd6c79beb488efac9f4f82b8225127fe3eac666857cb87d3ca52c1fd584fb7`
for each family. Original hashes remain unchanged. Results qualify explicit
stored-value inspection only; native Access and recalculation remain unrun.

## Original ACE 16 opaque catalog payload fixtures

[ObjectPayloadFixture.java](testing/ObjectPayloadFixture.java) is original MIT
code using reviewed Jackcess 5.0.4. It creates fresh inert table objects with
known opaque catalog byte patterns, NULLs, empty data and a 65,537-byte limit case.
The arbitrary bodies are not valid VBA/application content; generated files stay
untracked. The [read-only oracle](testing/ObjectPayloadOracle.java) accesses only
raw native binary fields without enabling expression evaluation.

| Profile | Input SHA-256 | Matched bounded objects | Payload bytes | Normalized payload SHA-256 |
| --- | --- | --- | --- | --- |
| V2016 | `e5493b66f3746e8436397341670a4dc8f2245c0280df86ea9fe15a79b86bdd32` | 37 | 16,582 | `73fe33efb50b3f110195310268ec4bed132e394a1805988253fc5ece62843cf6` |
| V2019 | `5ceeecde670461c6a5f5a3f7999cb7f79358a8162a6034dcba73c0302e243c50` | 41 | 14,596 | `e4d5cb12960c5ef9ffc02a5b024e66b5953b8770fe26a9361cba5fb4eee35bbd` |

[The comparison](testing/compare_object_payloads.py) verifies exact tagged bytes,
empty/NULL distinctions, immutable sources and one oversized-object rejection
without partial stdout/profile. The registered original Access example also
matches all 146 catalog records/24,693 bytes; normalized payload SHA-256 is
`50d613df681fefa2339e48a1a9ff5bfefa0e320bbe65510c37a593b31afdb31b`.
Its twelve module declarations all have NULL catalog payloads: actual VBA storage
is not discovered by those fields. Original source provenance/hash remain unchanged.
No active content executes, and no VBA extraction or Access save/reopen is qualified.

## Original ACE 16 stored scalar calculation fixtures

[CalculatedScalarFixture.java](testing/CalculatedScalarFixture.java) is original
MIT repository code using the reviewed Jackcess 5.0.4 artifact. It disables
evaluation and writes six cached projections (Boolean, Double, Currency, legacy
DateTime, uncompressed/Unicode-compressed Text), five rows and empty declarations.
No Access-generated or active application content is used; outputs remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `494160be568484db2ecb9fc08d14bbf2dd8446b8bc9d16e893743128dd01c757` |
| V2019 | `b59973b300ad3cff2e73506dc3a7b7ee121dc4611180533274b7359d05772c41` |

[The comparison](testing/compare_calculated_scalars.py) matches 30 cached cells
per family, empty declarations, exact CLI formatting and buffered rejection of
an unknown Boolean cache on a disposable copy. Tagged independent calendar rows
SHA-256 is `622a2f5aadbb9664ca734a4389d0fdd33402f403887df0751ca900f51c5bc155`
for each family. Legacy date values additionally match exact serial bits through
the original [date-bit oracle](testing/StoredDateBitsOracle.java), avoiding a
Jackcess calendar conversion that rounds away sub-millisecond precision.
TableCraft's nanosecond rendering retains an observed 65 ns fraction; this is
documented in the [stored-scalar scope](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-scalar-calculations).
Original hashes remain unchanged. Native Access, formula execution, ordinary
query eligibility and writes remain unqualified.

## Original ACE 16 attachment payload fixtures

[AttachmentFixture.java](testing/AttachmentFixture.java) is original MIT repository
code generated with the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019 files
contain six attachment files on one parent plus an empty parent: empty bytes,
all 256 byte values, Unicode UTF-8, inert PNG/ZIP signatures and a 65,536-byte
compressible payload. Four wrappers use zlib and two are direct, verified by
independent public encoded-data access. Nothing opens or executes file content.
Generated databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `36fdc9136cb13464a279b43f53b1dd3e8485c7f42dc8498d74c839b28e9bfd30` |
| V2019 | `c38881cdd305656af2ee8101e5491fd048437ab4a611dfeeb648cf9199a54960` |

[The runner](testing/compare_attachments.py) matches six original files and type
labels per family, totaling 65,810 bytes each, and verifies unchanged inputs.
Normalized decoded-row SHA-256 is
`4c038c940feaaa94445e667830b6b53007814d42345a594b2ca49f5b897c0995`
for each family. This is independent-reader evidence without native Access,
attachment write/reopen or large streaming qualification. The separate
`attachments.rs` CI golden uses Python zlib over original literal wrapper bytes.

The recipe's read-only `--metadata` mode and [typed collection comparison](testing/compare_attachment_columns.py)
now match all six stored metadata/encoded-payload records per family, with canonical
SHA-256 `ab8850199b35f9146a6442e92dea93c9ec6111ad25126eb9daac2126dccfa5f2`.
Exact DATE bits use the exported ordinary column writer in memory, preserving
DateExt with UTC settings and expression evaluation disabled. Six disposable
mutations cover NULL fields/payloads, a late checksum failure, unknown template
flags/names and orphan references. NULL and corrupt-wrapper metadata still matches
the independent getter; explicit decode errors publish no stdout. The 65,810-byte
default decode comparison remains unchanged, as do original input hashes.
Native Access, collection mutation and application save/reopen remain unqualified.

## Original ACE 16 Currency index inputs

[CurrencyIndexFixture.java](testing/CurrencyIndexFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. It refuses existing
output paths and generates inert V2016/V2019 tables with ascending/descending
Currency node trees, unique/primary declarations, signed extrema, adjacent scaled
units, duplicate/NULL keys and empty indexes. BigDecimal inputs never pass through
double. Generation asserts duplicate and primary-NULL insert rejection.
Generated databases remain untracked; nothing executes application content.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `2581de772a345fa3185bf546ae53e0607fd42abf6595b87725cd3d61299da414` |
| V2019 | `444ce6b3223943b7d1729b22de4278f7bb5b0f6e0468366e8ffc706754113cba` |

[The runner](testing/compare_currency_indexes.py) matches 1,426 ordered rows per
family, SQL filters and complete table/index check coverage. Normalized output
SHA-256 is `9ad67935b4dad7454b4d920e497c856c477d705f560d371c988b5431455a9487`
on both families. All original hashes remain unchanged. This is independent
engineering evidence; native Access, Currency mutation/seeks and application
save/reopen remain unqualified.

## Original ACE 16 Currency relationship inputs

[CurrencyRelationshipFixture.java](testing/CurrencyRelationshipFixture.java) is
original MIT repository code using the reviewed Jackcess 5.0.4 artifact. It refuses
existing paths and generates six inert tables/profile with exact Currency and
mixed Currency/Int32 keys, extrema, adjacent scaled units, partial/all-NULL tuples
and one-to-one declarations. BigDecimal inputs remain exact. Generation asserts
missing-parent, componentwise/partial-NULL and duplicate-child rejections.
Generated files are untracked; no application content executes.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `17c939417295c0045954f8272a8123454aa704d5f1a88d78cbf95a28eb35d57b` |
| V2019 | `0c41c93c534d3789b0dbc59cc1a1f7536c3ae991bd7916a5c41a337941a8ae3e` |

[The runner](testing/compare_currency_relationships.py) matches 48 exact rows,
six table audits and eight corrupt disposable-copy failures per family. Normalized
tagged rows SHA-256 is
`7eb875f3a826c4e7c45d3ae301ee65a3def8e224f5dd62cb8568bb9732f7414d`
on both families. Original inputs remain unchanged; native Access, cascades,
physical composite indexes and Currency write qualification remain separate.

## Original ACE 16 compound numeric index inputs

[CompositeIndexFixture.java](testing/CompositeIndexFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019
outputs contain five-/ten-component numeric node trees, all supported integer
widths, exact scaled Currency, mixed directions and reordered keys, partial/all
NULLs, unique/primary pairs and empty/unqualified indexes. Generation asserts
duplicate/NULL constraint rejections. No active content executes; generated
databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `7f5c569ad9989dbc69ed6595fdf3e9e35118df4091d7a0e294bb242b1a0bc098` |
| V2019 | `498de3bd2b3381d395931d18189e32f7a599f47e1e28daed55e6e3768000e41a` |

[The runner](testing/compare_composite_indexes.py) matches 4,513 exact ordered
rows/profile and required-index residual queries, complete check coverage,
maximum-width 98-/94-byte node/leaf entries, and explicit failure without fallback
for corrupt roots, Text and ignore-NULL profiles. Normalized rows SHA-256 is
`2706f9166905577243c6c2874b1dc560c4b9fb1577302d0ca5966a8765b3907f`
on both families. The ten-column trees have three node/45 leaf pages and maximum
prefix two; the separate 93-byte compressed prefix golden is synthetic CI evidence.
The existing [ACE16 corpus runner](testing/compare_ace16.py) also matches 1,000
original two-Int32 composite rows/profile without regenerating any input.
All original hashes remain unchanged. Native Access, composite mutation/seeks,
macros, concurrency and application save/reopen remain unqualified.

## Original ACE 16 GUID index inputs

[GuidIndexFixture.java](testing/GuidIndexFixture.java) is original MIT repository
code using the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019 files contain
1,000-row ascending/descending GUID node trees, unique/primary tables, all-zero/
high/group-boundary GUIDs, repeated NULLs and empty/unqualified composite indexes.
Input spellings are canonical uppercase to avoid the separately recorded default-
cursor case limitation. Generation asserts duplicate and primary-NULL rejection.
Nothing executes application content; generated databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `6805dbcf59eaa8d1ac88c7d5fd710eb131f0382a0d4ce712d7d1c80e1267b079` |
| V2019 | `4300ae1cec647871237c33f454ec7a5ce06f764dc4a874a6251e878bd1f2b900` |

[The runner](testing/compare_guid_indexes.py) matches 2,034 ordered rows/profile,
complete check coverage, required-index NULL filters and buffered rejection of a
corrupt selected root or unqualified GUID composite. Oracle text is formatted in
the CLI's lowercase canonical spelling; this does not define SQL collation.
Normalized rows SHA-256 is
`e8fd47e4a0ef3459e909c84e93797303b93f390612b78e3edc9b4a2e6e5f71ab`
on both families. Original bytes remain unchanged. Native Access, SQL GUID order,
GUID seeks/mutation and application save/reopen remain unqualified. GUID-bearing
composites now have the separate tuple evidence below.

## Original ACE 16 GUID tuple index inputs

[GuidCompositeIndexFixture.java](testing/GuidCompositeIndexFixture.java) is original
MIT repository code using the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019
outputs contain reordered mixed GUID/Currency/Int64/Int16 and ten-GUID node trees,
direction combinations, exact scaled extrema, partial/all NULLs, unique/primary
tuples and empty/unqualified profiles. Generation asserts exact/partial duplicate
and any-NULL primary rejection. No active content executes; databases remain
untracked. Canonical uppercase GUID input retains the earlier cursor-case boundary.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `58426808a5a292be96a455f27ee8dcde967f1a18453a04f5d6fcfb6f52c4539b` |
| V2019 | `3bfdf15fe9b113f8365d3f344129cf90b8bf549d2c8c3861bf7d8fa58afa8e5c` |

[The runner](testing/compare_guid_composite_indexes.py) matches 4,512 ordered rows/
profile, complete checks, required-index NULL filters and buffered failure for a
corrupt selected root or unqualified GUID/Text tuple. Ten-GUID trees contain nine
nodes/96 leaves across three indexes, maximum expanded entries 198/194 bytes,
depth-two explicit final-child roots and maximum observed prefix 19. The 193-byte
prefix is a separate synthetic golden. Normalized row SHA-256 is
`dc6a371702b44119fab81255803cd35952476ac56d8a93340721f6d41aefe7f5`
on both families. Original bytes remain unchanged. Native Access, composite seeks/
mutation, application save/reopen and broader index encodings remain unqualified.

The earlier [single-GUID runner](testing/compare_guid_indexes.py) now additionally
checks its empty GUID/Int32 composite without changing any original input or its
2,034-row canonical comparison hash.

## Original ACE 16 Boolean index inputs

[BooleanIndexFixture.java](testing/BooleanIndexFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. Fresh inert V2016/V2019
outputs contain 1,000-row single and Boolean/Currency/GUID/Int16 mixed node trees,
500 ten-Boolean rows, both directions, unique/primary/empty indexes and partially
NULL unique Boolean/Currency tuples. Generation asserts stored NULL-to-false
conversion and duplicate true/false/NULL-input rejection. No active content runs;
generated databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `ae9ef759d914e58bd54dcd6e0126f15f7624d6db7336497363199097404cb714` |
| V2019 | `27489674902f14335482eedad35ebcd3729d11c1927c393a88d63c3bb0eb9b32` |

[The comparison runner](testing/compare_boolean_indexes.py) matches 4,509 ordered
rows/profile, actual node roots, Boolean equality/IS NULL residual queries and
complete checks. Corrupt selected roots fail without output or fallback. Canonical
row SHA-256 is `949b831bc890d38f47fba4a5c196384c74277471ba415b9ccef9c1d8ce62aa9b`
on both families. It changes only host Boolean/GUID spelling when comparing,
without coercing values or imposing SQL collation. Original bytes stay unchanged.
Native Access conformance, Boolean foreign keys/seeks/mutation and application
save/reopen remain unqualified.

## Original ACE 16 floating index inputs

[FloatingIndexFixture.java](testing/FloatingIndexFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. Fresh inert V2016/V2019
version-two outputs contain Single/Double node trees, separate finite-only controls,
mixed floating/Boolean/Currency/GUID tuples, ten-Double keys, unique and empty
indexes. Values include signed zeros, subnormal/normal boundaries, finite extrema,
infinities and signed quiet NaNs with different payloads. Generation asserts
exact duplicate rejection, distinct zero/NaN-key acceptance and repeated unique
NULLs. These are artifact observations, not Microsoft Access write qualification.
No active content runs; generated databases remain untracked. Earlier private
version-one experiments remain outside the qualified corpus.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `f52b791d2c1e5a151cfb6e7d629bdf4e9cf2ca379436b8b12c90b4a8400ea6eb` |
| V2019 | `8f51e5d6c1ed947db798e78f25a879d11149f5d2fb7affad673ee1aa7be1af3a` |

[The runner](testing/compare_floating_indexes.py) independently matches 10,540
ordered rows/profile with exact tagged IEEE bytes and sequential controls. It uses
the original [TableDataOracle.java](testing/TableDataOracle.java)'s new explicit
index mode; existing sequential behavior is retained. Finite-only controls pass
required-index CLI queries and residual filters; non-finite tables retain query's
explicit buffered rejection. Complete checks pass and corrupt finite roots fail
without fallback. Normalized row SHA-256 is
`0a814faf5384c5d39ff67dd0da180125afa8239590c54c668db80bcdc197ce65`
on both families. Original inputs remain unchanged. Signaling NaNs are covered by
separate synthetic arbitrary-bit tests, not these Jackcess-generated originals.
Native Access, SQL non-finite semantics, floating seeks/mutation and application
save/reopen remain unqualified.

## Original exact DATE precision inputs

[DateBitsFixture.java](testing/DateBitsFixture.java) is original MIT repository
code using the reviewed Jackcess 5.0.4 artifact. Fresh inert version-two outputs
contain 14 ordinary and 14 explicitly cached DATE values per family: NULL, signed
zeros, subnormal serials, positive/negative day-zero aliases, negative dates,
sub-millisecond precision and valid calendar endpoints. Expressions are declared
but evaluation stays disabled. Normal DATE serialization independently verifies
cached carriers without relying on their wrapper offsets. Generated databases
remain untracked; earlier private version-one experiments are unqualified.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `f5b94dae401bc8bd74429f94fd5960839308ec488b61f3ae59c7632553bcaa4c` |
| V2019 | `a64dba44e398ff35bb8574941c3d6ce6063a9049b44cf68543edac7d5f52da5d` |

[The runner](testing/compare_date_bits.py) compares both native reads and the
corrected [bit oracle](testing/StoredDateBitsOracle.java) against independent
golden serial bits. It matches 28 cells/profile with normalized SHA-256
`bf551831ff280021cce1a92527e12e7f566eaf6c6237e2032de7dc03948e9320`.
Generation proves that `toDateDouble` changes at least one precision probe.
The oracle now uses read-only, in-memory DATE serialization instead, validating
ordinary widths and calculated prefix/length/trailer bytes before extracting bits.
No reflection, database writes or expression execution occurs during comparison.
The earlier 30-cell calculated-scalar runner still passes on both unchanged inputs.
Production Rust behavior is unchanged; native Access conformance remains unrun.

## Original ACE 16 DATE index inputs

[DateIndexFixture.java](testing/DateIndexFixture.java) is original MIT repository
code using the reviewed Jackcess 5.0.4 artifact. Fresh inert V2016/V2019 outputs
contain 1,000-row single and mixed DateTime/GUID/Currency/Boolean node trees, 500
ten-DateTime rows, unique/primary/empty indexes, exact negative serials, signed
zeros, subnormals, calendar aliases, sub-millisecond precision and date endpoints.
Generation asserts physical alias/zero distinction, exact duplicate rejection,
repeated unique NULLs and primary NULL rejection. Expression evaluation stays
disabled; generated databases remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `bc7b2887bfc775042a12bc77febae61ca6fc4b4da158fc03a541799f6d46d78a` |
| V2019 | `814ecb93c44ef2c26b716eaa5ddfc81d42a8491663581f007b922d1e20fbb49a` |

[The runner](testing/compare_date_indexes.py) independently matches 4,528 ordered
rows/profile, exact tagged serial bits, sequential controls, complete checks and
actual node roots. Required-index CLI comparisons retain alias equality and
civil-time ORDER BY separately from physical serial order. Corrupt roots fail
without output/fallback. Normalized SHA-256 is
`b7e26002302c699c869702c4b81d252140e8854ed6b97e3476635385e5078973`
on both families. Read-only DATE serialization uses the reviewed carrier's exact
bits, avoiding calendar conversion; original inputs remain unchanged. Extended
DateTime indexes, temporal seeks/mutation, native Access alias/constraint behavior
and application save/reopen remain unqualified.

## Original ACE 16 Decimal index inputs

[DecimalIndexFixture.java](testing/DecimalIndexFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. Fresh inert V2016/V2019
outputs assert corrected numeric index encoding, then construct coefficients
through BigInteger/BigDecimal without floating conversion. Three 1,000-row tables
use precision 28/scales 0, 4 and 28 with ascending/descending node trees, unique/
primary/empty controls, beyond-i64/2^53 neighbors and signed 28-digit extrema.
A precision-four/scale-two table and an originally unqualified empty Decimal tuple complete
the corpus. Generation asserts exact duplicate/primary NULL rejection and
ordinary unique NULL acceptance. No expressions execute; databases stay untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `ee6bf835f79670ef70adaf7128fe7609ae3ddd2be0ba6ed3d590304ed1a5b11e` |
| V2019 | `174a36153e459f69418bf40f80acb628ff190ef37720af57fba9752ee6d34fae` |

[The runner](testing/compare_decimal_indexes.py) independently matches 6,116 ordered
rows/profile, exact tagged coefficients/scales, sequential controls, complete
checks and actual node roots. Required-index CLI queries/NULL residuals pass;
corrupt roots fail without output/fallback. Normalized
row SHA-256 is `afdab22197e5112b69506d38f841e2bcd018fbdad89eedacb851d14a00795fd1`
on both families. Original inputs remain unchanged. Its empty Decimal/Int32 tuple
now additionally qualifies without changing the canonical hash. Populated Decimal
tuples have separate evidence below. Old numeric variants, seeks/mutation, native
Access and application save/reopen remain unqualified.

## Original ACE 16 Decimal tuple index inputs

[DecimalCompositeIndexFixture.java](testing/DecimalCompositeIndexFixture.java) is
original MIT repository code using the reviewed Jackcess 5.0.4 artifact. Fresh
inert V2016/V2019 outputs assert corrected numeric encoding and disable expression
evaluation. They contain mixed ten-component Decimal/GUID/Currency/Int64/Byte/
Int16/Single/Boolean/legacy DateTime node trees, ten Decimal fields at ten scales,
unique/primary/empty controls and unqualified Decimal/Text. Coefficients include
signed 28-digit extrema and exact beyond-i64/2^53 neighbors, constructed through
BigInteger/BigDecimal without floating conversion. Unique all-NULL exemption,
partial duplicates and any-NULL primary rejection are asserted. Databases remain
untracked and no active content executes.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `b988d40df160c21701a4d9289db80f6dccb55792aa3254f79760f38813d6a100` |
| V2019 | `ce9a997f1623c7ce151727949d40646cf394d9a1b552e4415b2bb065909d8269` |

[The runner](testing/compare_decimal_composite_indexes.py) independently matches
3,512 ordered rows/profile, exact tagged coefficients/scales and GUID/IEEE/DATE
bits, sequential controls, actual node roots and complete checks. Required-index
residual queries pass; corrupt roots and unqualified Text tuples fail without
output/fallback. Ten-Decimal geometry spans nine nodes/92 leaves, maximum expanded
entries 188/184 bytes and observed prefix 162; a separate synthetic golden covers
prefix 183. Normalized row SHA-256 is
`37c67da73393436cbb7523bbacf90eb8b10b25d043ef2bcbc78376882879cc48`
on both families. Original inputs remain unchanged. Decimal foreign keys now have
separate evidence below; tuple seeks/mutation, old numeric variants, native Access and application save/reopen
remain unqualified.

## Original ACE 16 Decimal relationship inputs

[DecimalRelationshipFixture.java](testing/DecimalRelationshipFixture.java) is
original MIT repository code using the reviewed Jackcess 5.0.4 artifact. Fresh
V2016/V2019 outputs contain 12 inert tables/profile with precision-28 Decimal
single keys at scales 0/4/28 and mixed Decimal/Int32/Decimal tuples. BigInteger/
BigDecimal inputs preserve extrema and adjacent coefficients beyond f64/i64;
NULL shapes and one-to-one declarations are explicit. Generation asserts orphan,
componentwise/partial-NULL and duplicate-child rejections. Outputs refuse existing
paths, remain untracked and contain no active content.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `cd1be7df981b25e22dae73e6cf929bddc13d3c5fa950a2a74e11ae10e162ee09` |
| V2019 | `420e976683f1df527e1a9cb65c7e693367abb06d422ec600ff98657d04bec2f1` |

[The runner](testing/compare_decimal_relationships.py) matches 116 exact rows,
12 table audits, 17 corrupt-copy failures, two all-NULL precision/scale mismatch
rejections and a separate all-NULL exemption per family. Normalized tagged row
SHA-256 is `bd60cfb4d961affa706c549808b51c301e6a695c1cd1fc37b69d5c74b3c828f4`
on both families; originals remain unchanged. Actual Access, cascades, cross-scale
membership, Decimal mutation and application save/reopen remain unqualified.

## Original ACE 16 multivalue collection inputs

[MultivalueFixture.java](testing/MultivalueFixture.java) is original MIT repository
code using the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019 outputs contain
eight scalar collection types, four parents each, exact IEEE/GUID/Decimal carriers,
Unicode/control text, stored duplicates/NULLs, empty parents, a 100-value boundary
and a separate 101-value overflow control. Inert row-source metadata is explicit;
no lookup or active content executes. Generated files refuse existing paths and
remain untracked. Decimal's built-in conceptual template has width 9, precision
18/scale 0 and zero rows; its generated child values have ordinary width 17.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `9417112bf18ef96824bc2653bc36aef361164daa6a04e4164a7f9baea1f2dc9d` |
| V2019 | `0c4eb87f037daa84d062af850b77cdb33d777831081b1271a231f73e255125aa` |

[The read-only oracle](testing/MultivalueOracle.java) uses native collection APIs
with expression evaluation disabled. [The runner](testing/compare_multivalues.py)
matches 32 parent collections/137 stored values per family, rejects 12 disposable
orphan/template-hint copies without output, distinguishes typed overflow rejection
from raw 101-item membership, and rechecks earlier populated complex inputs above.
Canonical column-tagged collection SHA-256 is
`a509aee1a4e181767cff52473c80ed1f569800f33f310ead6007e1c4bafec9de`
on both families; original bytes remain unchanged. Native Access, `.Value` SQL
expansion, lookup display semantics, mutation and application save/reopen remain
unqualified. The separate synthetic Large Number collection test is not an
independently generated BigInt multivalue fixture.

## Original ACE 16 stored version-history inputs

[VersionHistoryFixture.java](testing/VersionHistoryFixture.java) is original MIT
repository code using the reviewed Jackcess 5.0.4 artifact. Fresh V2016/V2019
outputs contain Notes/Details histories in two tables, with six parents/18 versions,
an ordinary Memo without history, empty membership, duplicate text, Unicode/NUL,
NULL text/time and an equal-time order control. The reference builder permits one
append-only Memo per table; no native Access restriction is inferred. Exact
DateExt carriers and deliberate Date overloads preserve 13 literal serial probes,
including signed zero, subnormals, negative aliases, sub-millisecond precision
and calendar endpoints. Generated files refuse existing paths, remain untracked
and execute no application content.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `0fabc26956897e117dffb19ca1e575713172988452a01e8ff6a81ba79db3a8d0` |
| V2019 | `5f92a714c9e920459c313e70965b537b509e2d87da32499fcc43e640fb921b92` |

[The read-only oracle](testing/VersionHistoryOracle.java) compares flat storage
order and separately observes public history API order with expression evaluation
disabled. [The runner](testing/compare_version_history.py) matches six collections/
18 versions per family, exact text/DATE carriers, nullable metadata and visible-
Memo bindings; public API membership agrees while its tie-ID order differs.
Six corrupt-copy failures, one preserved NULL timestamp mutation and the earlier
populated complex/history corpus are rechecked without changing original bytes.
Canonical Memo-tagged history SHA-256 is
`7791c023ea5586710e7b715722132aee2ae6070adb70a442c097615c7b3d1d5e`
on both families. Native Access, current-value reconciliation, default ordering,
history mutation and application save/reopen remain unqualified.

## Original ACE 16 saved-query fragment inputs

[SavedQueryFixture.java](testing/SavedQueryFixture.java) is original MIT repository
code using the reviewed Jackcess 5.0.4 artifact. It refuses existing paths and
writes explicit inert MSysObjects/MSysQueries rows into fresh V2016/V2019 files.
Eleven definitions/75 fragments cover all nine query-object flag families, signed
native IDs/flags/extra values, unknown attributes, Unicode/NUL/control text,
NULL/empty distinctions, nonmonotonic order bytes and an empty definition. The
reference's required ObjectId/Attribute/Order index constrains original inserts;
illustrative fragments are not qualified executable SQL. No action, expression,
provider or application content executes; generated files remain untracked.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `297e7d3aa5c27dd6ba77c1d93466723785fc09e32b9f1c89e54cc96a89a1b2b4` |
| V2019 | `f1fc4ad48d91f05bf60c5f88f092aaa0b09be2cf66080c74058d3af8fb6cf19d` |

[The read-only oracle](testing/SavedQueryOracle.java) never invokes toSQLString or
execution. [The runner](testing/compare_saved_queries.py) matches all 75 fragments/
family and six disposable-copy outcomes: four retained inspection variants
(unknown tag, NULL attribute/order, unbound row) and two buffered carrier/geometry
failures. NULL key fields violate native index constraints; successful inspection
does not qualify integrity or execution. Canonical report SHA-256 is
`cd2a2cb03a394894d277e28a85f79c0ac9ba3a59f1b47aa19e4970637a401f8e`
on both families.

The same runner also matches all 59 saved queries/332 fragments in the unchanged
registered ACE-code-2 Access example, with report SHA-256
`1ac317028e1171c63a351da32263a2fe9dfe2a2b98b060ce6d9eb383799d6c52`.
That sample's existing provenance/license/hash above remain authoritative. That
fragment corpus does not qualify SQL reconstruction/execution, native Access or
application save/reopen.

## Original ACE 16 saved SELECT inputs

[SavedSelectFixture.java](testing/SavedSelectFixture.java) is original MIT repository
code using the same reviewed Jackcess 5.0.4 artifact. It refuses existing paths and
creates thirteen local SELECT definitions through explicit fresh system-row writes
per V2016/V2019 family. Constants, source/column aliases, multiple projections,
wildcards, all SELECT modifiers, grouping/having, multiple sort keys, Unicode,
unresolved functions, a saved-query source and an omitted type row are represented.
Expression evaluation stays disabled; no SQL or active application content runs.
Generated files remain untracked and mutations use disposable copies.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `c3835339c5b54b164ddc8e041cd18d38e15ae5dc358b5380aa885d52ef86d294` |
| V2019 | `db05f622b34776278943b5fb862130c20a85ba6ed496851bc7e481ebaabeea66` |

[SavedSelectOracle.java](testing/SavedSelectOracle.java) opens read-only and invokes
toSQLString without execution. [Comparison](testing/compare_saved_selects.py) feeds
those independently assembled spellings through the bounded SQL parser, matching
all thirteen adapter models per family through both the engineering host and
`query saved` CLI. Query-name-prefixed canonical SQL SHA-256:
`71cf898b621915f2f302330707f99494d399912066d53fd5a32e7ecaf2a5709e`.
Eight copies per family reject unknown attributes/flags, conflicting native type,
unhandled clause alias, duplicate/reversed/NULL projection order and late declared
row counts, without stdout through both hosts. Missing/table namespaces also reject.

The registered example separately asserts an observed bare leading-digit name in
the reference rendering and compares the adapter with an explicitly quoted expected
source identity. That spelling is outside the SQL parser's current profile; this
regression is not counted among whole-renderer comparisons. Original sample bytes
and hash remain unchanged. [The contract](NATIVE_SAVED_QUERIES.md#owned-select-reconstruction)
records the narrow metadata/order/identifier profile. Saved-query execution,
native Access conformance and application save/reopen remain unqualified.

## Original ACE 16 saved join inputs

[SavedSelectFixture.java](testing/SavedSelectFixture.java) `--joins` is original MIT
repository code using the same reviewed artifact, fresh-path and disabled-expression
policies. Nine additional definitions cover INNER/LEFT/RIGHT, reversed table
storage, ASCII endpoint case variants, explicit aliases, self-joins, Unicode table
names with ASCII aliases and reversed unaliased endpoint orientation. Thirteen
baseline SELECT definitions remain, for twenty-two models per family. Declared
tables/fields exist in these original fixtures, but no query execution occurs.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `3c1bd36f7a0597fdc5cf37191125c32203ebbd2f910369e2f39fc05aa43bd0f2` |
| V2019 | `92e2221be9bab8c6cbfd27e05eec9c50709c9c6103e264603c621d665e564a94` |

[The runner](testing/compare_saved_joins.py) matches the independent renderer's
twenty-two normalized models through both the engineering host and public CLI.
Query-name-prefixed canonical SQL SHA-256 is
`25fdf506c0243a5d3960caecf7e301da3dc315ed16a81927e48fb23bab9caa68`
on both families. Eight mutations per family retain inspectable raw field carriers
but fail reconstruction without stdout: unknown/NULL kind, NULL left/right/condition,
undeclared endpoint, ambiguous aliases and non-ASCII comparison labels. Originals
remain unchanged; generated files stay untracked. [The join contract](NATIVE_SAVED_QUERIES.md#two-source-native-joins)
keeps compound/larger graphs, Cartesian sources, non-ASCII label matching, actual
Access and query execution unqualified. No production ACE write support is enabled.

## Original ACE 16 saved parameter inputs

[SavedSelectFixture.java](testing/SavedSelectFixture.java) `--parameters` is original
MIT repository code using the same reviewed artifact, fresh-path and disabled-
expression policies. Eight parameterized definitions add twelve scalar type tags,
literal prompts/punctuation, NULL/zero/positive/max-i32 Text syntax arguments,
ordering/owner-access and a parameterized join to the thirteen baseline SELECTs.
No prompts, native query evaluation or application code run. Generated files remain
untracked; all mutation experiments use disposable copies.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `72d4ed8567e85437a91a086bfd3b1d33858b3e1687cb00cbc526f66d3cd2e6fa` |
| V2019 | `8b1d1e3d2a0a81a2ce364f4e51bf3a0d6896e24e4a5f3fe32fc0851b67f9426d` |

[The runner](testing/compare_saved_parameters.py) matches all twenty-one independent
normalized models per family through the engineering host and public CLI.
Query-name-prefixed canonical SQL SHA-256:
`ea448de23733f8f2dccb47770a1044e1aeeca7e13838b24148bd9db2d7d18801`.
Eight changes/family retain inspectable raw carriers but reject reconstruction
without stdout: unqualified type, NULL type/name, negative Text argument, unhandled
non-Text extra, duplicate name/order and unused non-NULL empty Name2. Name mutation
checks exact original encodings and replaces equal-width bytes. Duplicate ordering
keys do not qualify native index integrity merely because inspection succeeds.

The [parameter contract](NATIVE_SAVED_QUERIES.md#native-parameter-declarations)
keeps type/name gaps, runtime Text-value limits, native Access and query execution
qualification explicit. Text arguments are syntax, not stored-field width evidence.
Current Rust tests separately pack synthetic system rows across bounded data pages
to check the 255-declaration policy ceiling, cancellation and allocation/work limits.
Existing registered example bytes/hash remain unchanged; no ACE write support is enabled.

## Original ACE 16 compound join inputs

[SavedSelectFixture.java](testing/SavedSelectFixture.java) `--compound-joins` is
original MIT repository code using the same reviewed artifact/fresh-path policy.
Six definitions cover two/three distinguishable same-pair conditions for each
INNER/LEFT/RIGHT kind, alongside the twenty-two earlier SELECT/simple-join controls.
Expression evaluation remains disabled and no native query or application code runs.

| Profile | Input SHA-256 |
| --- | --- |
| V2016 | `02c5d9c4fe4c44100ec1ebea8d06de4c58159337bd9286c9b3c559cfcba69f27` |
| V2019 | `2409d297ef87533229aa4f18ddcbf9b968ce6ecf208863f94c866682cf704e2a` |

[Comparison](testing/compare_compound_joins.py) matches all twenty-eight normalized
independent models/family through both hosts, including reversed condition order
and owned conjunction association. Query-name-prefixed canonical SQL SHA-256:
`df42f32423054ac50ac9252de8e29157d5bff6a18385cabdb86d38627aaa2374`.
Eight copies/family keep raw carriers inspectable but reject reconstruction without
stdout: mixed/NULL kinds, NULL endpoints, reversed/other condition endpoints and
duplicate/reversed ordering keys. The runner verifies original endpoint/order
encodings before mutation. Originals remain unchanged and generated files stay
untracked. [The condition contract](NATIVE_SAVED_QUERIES.md#multiple-conditions-on-the-same-pair)
keeps the sixteen-part consumer ceiling, metadata/name/graph gaps, actual Access
and native execution/error-order qualification explicit. No ACE write support is enabled.

## Jet 4 and ACE 12 current-profile inputs

Original inert recipes DecimalIndexFixture, DecimalCompositeIndexFixture,
DecimalRelationshipFixture and SavedSelectFixture now accept V2000; Decimal recipes
also accept V2007. Generated fresh on 19 October 2026 using the previously reviewed
Apache-2.0 Jackcess 5.0.4 JAR (SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`,
source pin `be0a2139b087a227d14cafd4ea231306a5a3a862`). No expressions, saved SQL,
VBA or providers were executed. These files remain untracked independent fixtures,
not Microsoft Access-authored/native qualification. Originals are retained unchanged;
mutation experiments operate on temporary copies only.

| Generated input | SHA-256 |
| --- | --- |
| `tablecraft-jet4-decimal-index-v1.mdb` | `2dd14697aca3a63b181152bb3e79f4a4a8bebec54e49b97abb119a84f2e13503` |
| `tablecraft-ace12-decimal-index-v1.accdb` | `5342a9a6368c66df17a7c664368af79a7a4acfa718929b4afea087db63fd30b2` |
| `tablecraft-jet4-decimal-tuples-v1.mdb` | `5f322a9591a701d7f374c1581045c3a668e12cdc5e8246c562af3a0a8ef56277` |
| `tablecraft-ace12-decimal-tuples-v1.accdb` | `93f3e9d409336e383265801c6b4a0d4a5b1fe2b4482e23cf73eefd1a3cc60c1b` |
| `tablecraft-jet4-decimal-relations-v1.mdb` | `c9733cb9549c772f7df931d3a244365562702d27a930b7bd19e2119b6a743f64` |
| `tablecraft-ace12-decimal-relations-v1.accdb` | `c3f0d637ab7f8829d943f01151c16795c5af85af4b0dd298940974e58a35bffd` |
| `tablecraft-jet4-saved-compound-v1.mdb` | `4910f3936a60fdc0b9d8543a7c14a8b030e1d297b144d719c9fae8ae3636756a` |
| `tablecraft-jet4-saved-parameters-v1.mdb` | `26fd0cd792034976c82998acdf2fcaed48be9cbe482ea8e4f6eb12b0f868eeec` |

The three Decimal comparison runners match independent cursors/carriers and
complete checks with 6,116 single-index visits, 3,512 composite visits and 116
relationship rows per family. Jet 4 ordering hashes differ deliberately for legacy
descending sign groups. Older mixed fixtures substitute Int32 for unsupported
Large Number. Existing V2016/V2019 inputs remain regression controls.

- `tablecraft-jet4-decimal-index-v1.mdb` canonical row SHA-256: `d5c9e5aba815cfedab29f6f05b9d7f2de1a632c10abd8da9ddbcbfd24cac95f7`.
- `tablecraft-ace12-decimal-index-v1.accdb` canonical row SHA-256: `afdab22197e5112b69506d38f841e2bcd018fbdad89eedacb851d14a00795fd1`.
- `tablecraft-jet4-decimal-tuples-v1.mdb` canonical row SHA-256: `0f8ac315ae525b8d007b7e95519944797b3b99b8c3c51744953b2831ab627359`.
- `tablecraft-ace12-decimal-tuples-v1.accdb` canonical row SHA-256: `1278189dda20b2b59d2d349f83af10217267601b83673e0b4161868ee91d0308`.
- `tablecraft-jet4-decimal-relations-v1.mdb` canonical row SHA-256: `bd60cfb4d961affa706c549808b51c301e6a695c1cd1fc37b69d5c74b3c828f4`.
- `tablecraft-ace12-decimal-relations-v1.accdb` canonical row SHA-256: `bd60cfb4d961affa706c549808b51c301e6a695c1cd1fc37b69d5c74b3c828f4`.

Jet 4 saved compound and parameter models match the existing canonical hashes
`df42f32423054ac50ac9252de8e29157d5bff6a18385cabdb86d38627aaa2374` and
`ea448de23733f8f2dccb47770a1044e1aeeca7e13838b24148bd9db2d7d18801`.
Each runner rejects eight malformed metadata copies without stdout.
The Jet 4 offline Decimal4/ID edit produces a disposable image SHA-256
`fb4cf72c5e62537c6d8cbd1fb1c077c60e86f177ba7fbd69c7c1e90ab1dae501`,
with sequential/two-index row SHA-256
`d1bc3a77aa2fca4327c3382045c95f4a67eab869b087042518a07f13eee5e086`.
The [profile record](NATIVE_JET4.md) separates native/remaining scope.

## ACE 16 write-preview inputs

UpdateFixture creates fresh inert V2016/V2019 inputs with Required/range properties,
a primary Int32 index, three rows, Unicode Memo and OLE payloads. Expressions are
not evaluated. Generation uses the previously reviewed Apache-2.0 Jackcess 5.0.4
JAR SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`
and source pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. Originals and candidate
copies remain untracked; no third-party code is copied and no native Access
creation/qualification is claimed. The normal ACE write API remains blocked.

| Input family | Original SHA-256 | Score candidate SHA-256 | Id/index candidate SHA-256 |
| --- | --- | --- | --- |
| `tablecraft-ace16-preview-v1-2016.accdb` | `45928dd403fd97f2acb2dead3da7fb9a412e9738a229b71453dc17b93d5476cf` | `979d2d3f7033fd61b3b6a1cc3772c527c0e08f4275b5c6499e133e56e4f2233e` | `a5ad00046a6cfb4d518734bcf934b6c7ae0e0a1b0e7d96179d80998962080383` |
| `tablecraft-ace16-preview-v1-2019.accdb` | `2b4fbc738907ec8e29caa1d3b9f4bf67e8dcb77ef812e11f0e14b8fc41a7524b` | `31b94bda901868e26005dfca7a7e9b93eb9912a1fe8538ac257ef48a1b097daa` | `8de5d804e76120e8d7a5d6bd99292979af6cbca5974c031f2eeb3e55e8f0c145` |

The [comparison runner](testing/compare_ace16_write_previews.py) verifies exact
field/index-entry spans and independently reopened rows, long-value lengths and
index references; candidates retain every unrelated byte. It rejects rule/stale/
duplicate requests without files, refuses existing outputs and checks the normal
CLI ACE gate. Original hashes remain unchanged. [Preview scope](testing/ACE16_WRITE_PREVIEWS.md)
records separate native/macro/production qualification requirements.

## ACE 16 exact fixed scalar preview inputs

The original inert FixedUpdateFixture recipe generates V2016/V2019 files with
nine fixed scalar carriers, an Id primary index, three rows and Memo/OLE payloads.
Expression evaluation is disabled; no application code is executed. Generation
uses the previously reviewed Apache-2.0 Jackcess 5.0.4 artifact SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, source
pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. Files remain untracked; independent
creation is not native Access provenance. Mutation outputs are disposable copies.

| Input | Original SHA-256 | Canonical compared rows SHA-256 |
| --- | --- | --- |
| `tablecraft-fixed-updates-v1-2016.accdb` | `4a988d93558a043f34eb59991e1f404c561a374ba81e1c693943831fe4a8ffee` | `4f41c7553585ce7cb510e11d39b8d90c2e8fe88c45fd3392a33ab216e5c6a02f` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `75180233acd2dc1d875af3ac51530e6b95757261c78f713126f6ea5049ea5a2d` | `4f41c7553585ce7cb510e11d39b8d90c2e8fe88c45fd3392a33ab216e5c6a02f` |

| Input | Changed field | Candidate SHA-256 |
| --- | --- | --- |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Tiny` | `3194decb46e8dd39d258be7944e05f6590bf1614eb992cda0af2d4a2ea797ad1` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Narrow` | `c825d91dbb48d50dd11a2d444cf5f71206a4b6c46aa36998ab0563f6ce262bde` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Number` | `68cea4f9e491f0a1d4a80269f2abad46ff670286d251bdf2eac791ce511b9c0a` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Large` | `73ba49fd43a13c4976715aa344ae52504e54bc67c872afc4e055b43c5dae1486` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Amount` | `e5904d664ec24fa708def3927f2369be10b7417b03e049d61a9e63686e7b8e91` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `S` | `dfb63bd4fe687e310447f086d09501f95455aa97fe6923a493136dde79eb56c4` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `D` | `3ed9315c6a3be91ec35d8d0b7ee4e603fa9a29c50822bc41d98849cafa36625f` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `Moment` | `330c3fd3c4d9769061d085861a7126290092bf6166e1d70f0c8a5c6ecd800347` |
| `tablecraft-fixed-updates-v1-2016.accdb` | `G` | `e27c0560ac130ce6d88c0325aad0880158fffbca41eb28d5c4f675b7e2c6b2d3` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Tiny` | `c59b724221ea7a11134abea1471044ccf3a0c48a0adf01cfefd01520fe78b502` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Narrow` | `1b747b6e89b51ff29f29ec77d0460a8c344c21ddfa0032e1e83ff4a478693dca` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Number` | `4399a9db6fc3affca7b44cd95e2920b5b645e066e95bd57def8d5ad20a6e59f6` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Large` | `0adb3fecc2e1d362f3a053ef1686d2b1ea25ab53a994e607eef71ed752d14ab6` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Amount` | `1e19551e34b99097393a05b192dfbfd01a093b8b048d5307e4502582db8ccd5c` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `S` | `72888b6bfced7fae658a09745112c2f40a93789b732e2d5b0286f7e6f1d3b6e6` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `D` | `5cbbb09b91bf0106f8b8fc8022d7659bd5b3f2ad5d5db242fe6b0d9f1623ee2f` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `Moment` | `b6db5d7f9f8a1b843a10b93df87b68335b8707980cbf7ae41a477fff11583a64` |
| `tablecraft-fixed-updates-v1-2019.accdb` | `G` | `cec449d63ac5f0ee46802bc849914643644c99648daf9d8070fca3e13bd08eee` |

All eighteen candidates match exact independent carriers and complete Memo/OLE
contents. Every byte outside the selected fixed field remains unchanged; the
unrelated primary index is checked and retained. Wrong carrier input creates no
output, existing output paths are never overwritten and original input hashes
remain identical. [Contract and reproduction](testing/ACE16_WRITE_PREVIEWS.md#exact-fixed-scalar-candidates)
retain the optional/non-journalable boundary and Windows/macro qualification gaps.

## ACE 16 Boolean and NULL-mask preview inputs

The original inert [MaskUpdateFixture recipe](testing/MaskUpdateFixture.java)
generates V2016/V2019 inputs with Boolean columns across mask-byte boundaries,
nine nullable fixed carriers, required/range rules, Memo/OLE payloads, an enforced
relationship and an indexed Boolean control. Expression evaluation is disabled.
Generation uses the reviewed Apache-2.0 Jackcess 5.0.4 JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, source
pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source is copied;
originals and disposable candidates remain untracked. This is independent
creation, not native Access provenance.

| Input | Original SHA-256 | Canonical compared rows SHA-256 |
| --- | --- | --- |
| `tablecraft-mask-updates-v1-2016.accdb` | `62dbe695e55420e525c7fef58ae2cde2a05ed3e5b8200df9897fbe87fb67553f` | `ca5146cfe4a068496b1dfcb86b409687164d29631b4ab945ceb0b6d6d04dae40` |
| `tablecraft-mask-updates-v1-2019.accdb` | `d67d2e8e873c58cebce49c1531c7f4d7429f5a12459242df68767d7c176a5e6d` | `ca5146cfe4a068496b1dfcb86b409687164d29631b4ab945ceb0b6d6d04dae40` |

[The comparison runner](testing/compare_mask_updates.py) passes 42 candidates per
family, including fourteen exact no-ops, with complete independently reopened
rows, long values, selected payload/mask spans and unchanged neighboring bits.
Eleven rejected requests per family now cover required/rule failures, wrong/stale
carriers, Boolean NULL, unsupported indexed targets, missing rows and orphan keys.
Existing outputs, original hashes and both 42-candidate row hashes remain
unchanged. The automatic type-2 index is retained. The previously rejected
valid-parent child edit now belongs to the checked foreign preview profile; its
broader independent evidence is recorded below. Native Access, data macros and
normal ACE saves remain unqualified; see the
[mask contract](testing/ACE16_WRITE_PREVIEWS.md#boolean-and-null-mask-transitions).

## Reciprocal foreign-index inputs

The original inert [ForeignIndexFixture recipe](testing/ForeignIndexFixture.java)
at commit `d28143d` created V2016/V2019 restrict, cascade-update/delete, SET NULL, composite,
self-reference and shared-primary-alias cases. Expressions are not evaluated.
Generation and the original [read-only oracle](testing/ForeignIndexOracle.java)
use the reviewed Apache-2.0 Jackcess 5.0.4 JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, source
pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source is copied.
Generated databases and disposable malformed copies are untracked; independent
creation is not native Access provenance.

| Input | Original SHA-256 | Canonical compared links SHA-256 |
| --- | --- | --- |
| `tablecraft-foreign-index-links-v1-2016.accdb` | `74a6c80d03177009463782a13bf5f4014659bef6a4aae5e18e098424d615dc47` | `be559eeda05ad7c5906102bf0ef12ff65f91f6f1e340dceb09ff8ca666bb6e3b` |
| `tablecraft-foreign-index-links-v1-2019.accdb` | `cc43635f85bc48800f803e56aba0a61d2f6e8ca009a9bf8bab765c9ef1fe5499` | `503241eae2141e7a65b4e1e7076eec12140f6319eb96cd1d5fc259567534507c` |

The v1 runner matched twelve directed links
per generated family and rejected sixteen corrupted copies without partial
output. Two registered-sample links match canonical SHA-256
`5813a664056056b01f5a23f57c9d768ddfcbc3f7ef33a8229c240b1166b9985f`.
All originals remain unchanged. This first evidence qualified reciprocal metadata;
the current runner requires the expanded v2 inputs below. Physical foreign-index
reads/writes and native Access behavior remain separate work. See the
[inspection contract](NATIVE_INDEXES.md#reciprocal-foreign-index-links).

## Relationship-catalog agreement inputs

The [ForeignIndexFixture recipe](testing/ForeignIndexFixture.java) at commit `a8dfeaa` added
one-to-one and non-enforced controls to the v1 cases. Fresh v2 originals use the
same reviewed artifact, source pin and disabled-expression policy above. They do
not replace or modify v1 originals. No third-party source or generated database
is tracked.

| Input | Original SHA-256 | Canonical compared links SHA-256 |
| --- | --- | --- |
| `tablecraft-foreign-index-links-v2-2016.accdb` | `c222583ba5cf319c0d6ca59a055c8767da0e95a9881c3955a7f81f14b777474b` | `1b88ee390d33f44ed8d8f16446205d19c7b1232bdecd3075f7f39b720f564457` |
| `tablecraft-foreign-index-links-v2-2019.accdb` | `60e289c46ca3c86c52ffd9a1b121bcdebbe18ab787101d727b9038a36f156e4f` | `616ca8a81bf7496cc0d7df7b080dc7f84fc94bd23b907e413f1b20116e3ed7f1` |

The current [oracle](testing/ForeignIndexOracle.java) checks catalog endpoints,
column-pair mappings, actions, uniqueness and complete enforced coverage before
publishing tags. [The runner](testing/compare_foreign_indexes.py) matches fourteen
directed links per family, two unchanged registered-sample links, and positive
composite declaration-ordinal permutations. Thirty-one malformed copies per family
reject with empty output, including mutually consistent pointers/actions that
disagree with the catalog, missing enforced links, crossed composite mappings
missing one-to-one uniqueness and a catalog alias of the selected source page.
Originals remain unchanged. This qualifies
metadata agreement, not row membership, physical foreign-index traversal/writes
or native Access behavior.

## Foreign physical read inputs

The current [ForeignIndexFixture recipe](testing/ForeignIndexFixture.java) adds
duplicate dependent keys and 1,000-parent/1,200-child multi-page trees to the v2
metadata controls. Generation uses the same reviewed Apache-2.0 Jackcess 5.0.4
artifact/source pin above and disables expression evaluation. Fresh v3 inputs do
not replace or mutate v1/v2 originals. No third-party source or generated database
is tracked; independent creation is not native Access provenance.

| Input | Original SHA-256 | Canonical foreign-index rows SHA-256 |
| --- | --- | --- |
| `tablecraft-foreign-index-links-v3-2016.accdb` | `ae4f6924be2de8871beb6e2ce631aa9a5e7e4dad1c73afb75476e06ae9c3a224` | `80e2c86485692915a8f1ad58f09d36b4048823699e553b2c71e82f54a0e7c734` |
| `tablecraft-foreign-index-links-v3-2019.accdb` | `518a1d7df248e54ef2a707f2ab0bdd42acd0a6253328c91333ad400e8b5e78e8` | `80e2c86485692915a8f1ad58f09d36b4048823699e553b2c71e82f54a0e7c734` |

The [read runner](testing/compare_foreign_index_reads.py) matches exact index order,
all rows and complete index-check coverage against the existing read-only
[TableDataOracle](testing/TableDataOracle.java). Each v3 family matches sixteen
foreign indexes/2,225 returned rows, including two node roots; four malformed
copies per family reject through both read and check without output. The registered
sample matches two indexes/952 rows (one node root), canonical row SHA-256
`554a13c1193eda5c1dade1582cac8492b87e19c3ed7f52d38979c7a782914538`.
Existing mask v1 inputs retain their original hashes above and each match two
nullable Large Number foreign indexes/three rows, canonical SHA-256
`bfbebddefd774a90df5a0b2813647fe4e24f44f00e02733f980f8355463870f5`.
All originals remain unchanged. This qualifies physical read/declared-index checks,
not foreign row membership, cascade execution, mutation or actual Access behavior;
see the [read contract](NATIVE_INDEXES.md#foreign-physical-traversal-and-checks).

## Non-unique index update inputs

The original inert [NonUniqueUpdateFixture recipe](testing/NonUniqueUpdateFixture.java)
creates fresh V2016/V2019 and V2000 controls with duplicate/NULL keys, two physical
ascending/descending Score indexes and a shared logical alias, 1,000-row trees,
non-monotonic identities, long Memo/OLE values and separate unique/rule guards.
Expressions are disabled. Generation uses the reviewed Apache-2.0 Jackcess 5.0.4
JAR SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`,
source pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source or
generated database is tracked; independent creation is not Access provenance.

| Input | Original SHA-256 | Seven-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-nonunique-update-v1-2016.accdb` | `9bbc348cae7f55f35627a13f0e2df270b7df02afaeff33fe3455797f26b4ba89` | `9367c613120f93634666a82ed157467697b02084b16473f638695b932f0bab05` |
| `tablecraft-nonunique-update-v1-2019.accdb` | `cd30574d29172a6fcbc5c13978491139e627e180b0c12ae0f30f81bc5d09ca21` | `e22e31736601fd52de540a886ed832599a5b142372614161891a0853ee431905` |
| `tablecraft-nonunique-update-v1-2000.mdb` | `542050cb3e8525b7d34ce6766687609030d0112cb5ff2ac1ae838b13f3475b91` | `e6b019cf9430286b21d34d58d4e0affb49b4997ead4142aee8666ad4b005f927` |

[The comparison runner](testing/compare_nonunique_updates.py) independently expands
original native prefixes and binds row references/entry spans. Twenty-one complete
candidates match independent rows, duplicate tie order, long values and unchanged
bytes outside selected spans. Each family includes one exact no-op, five rejected
requests and preserved existing output paths. Jet 4 CLI output matches all seven
engine copies; both ACE families still reject normal CLI editing. Each tree has
three logical node roots, with Asc/Alias sharing physical data. Manifest hashes
cover ordered candidate SHA-256 strings joined with newlines and no trailing
newline, in runner case order. All originals remain unchanged. Native Access,
foreign/indexed-NULL mutation and resizing remain unqualified; see the
[write contract](NATIVE_WRITES.md#non-unique-int32-replacements).

## Indexed Int32 NULL preview inputs

The original inert [IndexedNullFixture recipe](testing/IndexedNullFixture.java)
creates fresh V2016/V2019 ordinary/unique/required/rule controls, ascending/
descending indexes with shared aliases, NULL and duplicate keys, 1,000-row trees
and Unicode Memo/OLE values. Expressions are disabled. Generation uses the reviewed
Apache-2.0 Jackcess 5.0.4 JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, source pin
`be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source or generated
database is tracked; independent creation is not native Access provenance.

| Input | Original SHA-256 | Fifteen-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-indexed-null-v1-2016.accdb` | `eb520858bfd08bfd3101e4b6d3aa96963c2e63365dd88cc28e1e19b3459c1f9d` | `8439e7213067bf508734046c5b51c1fd52180efa12fb3a40cdeea93dd7cb4aac` |
| `tablecraft-indexed-null-v1-2019.accdb` | `8c091357504232679402a36a0081e83ec1e912c74db4abe9e27a19cc32bd6821` | `f2f6aed471751857ff25b846b7009b553a9d3f4190f6c5bbb7bb3ae6ee8c37e0` |

The [runner](testing/compare_indexed_nulls.py) matches thirty complete candidates,
including six exact no-ops, against independent row/index cursors. It checks exact
row-mask bits, retained ignored payloads, native prefixes/bitmaps/free bytes,
unchanged per-page entry counts, child links, page sets and row-reference sets;
all unrelated bytes and full Memo/OLE values remain intact. Fourteen rejected
requests cover unique/required/rule, stale/missing and primary-key NULL failures.
Existing output paths and all originals remain unchanged. Manifest hashes cover
ordered candidate SHA-256 strings joined with newlines and no trailing newline,
in runner case order. This does not qualify actual Access, data macros, production
saves, foreign/wider indexed mutations or allocation/splits; see the
[preview contract](testing/ACE16_WRITE_PREVIEWS.md#indexed-int32-null-transitions).

## Integral and Currency index update inputs

The original inert [IntegralUpdateFixture recipe](testing/IntegralUpdateFixture.java)
creates fresh V2016/V2019 Byte, Int16, Int64 and Currency ordinary/unique/required
controls, ascending/descending indexes with shared aliases, exact extrema and
adjacent large integers, NULLs/duplicates, 1,000-row trees and long Unicode Memo/OLE
values. Expressions are disabled. Generation uses the reviewed Apache-2.0
Jackcess 5.0.4 JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, source pin
`be0a2139b087a227d14cafd4ea231306a5a3a862`. The recipe is repository-authored;
no third-party source or generated database is tracked. Independent generation
does not establish Microsoft Access provenance.

| Input | Original SHA-256 | Twenty-eight-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-integral-updates-v1-2016.accdb` | `10a2a5b78a17ed616f92bd1a17d95993c33edf17330223c7db58593634822c06` | `d3e5e99df89f15b3009e20fa05e56b7d3350ca7ff313b009993a3982de19ed82` |
| `tablecraft-integral-updates-v1-2019.accdb` | `d60e38adeebeac82f32d340ff8ebeecf580398cb8503320881bd8d3ad6e1dd27` | `5e98e7745903d6811f2fc76d0a6c263feaf8a911f08b92bfcf555ba4d32cf321` |

The [runner](testing/compare_integral_updates.py) matches 56 complete candidates,
including eight exact no-ops, with independent exact row/index cursors. It proves
unchanged per-page entry counts, child links, page and row-reference sets; only
admitted field/mask/index regions change. Complete Memo/OLE values survive.
Thirty-two rejected requests cover duplicate keys, required NULLs and stale values.
Existing output paths and original input bytes remain unchanged. Manifest hashes
cover ordered candidate SHA-256 strings joined by newlines without a trailing
newline, in runner case order. This does not qualify native Access/data macros,
production saves, composite/foreign/non-integral mutation or allocation/splits;
see the [integral preview contract](testing/ACE16_WRITE_PREVIEWS.md#integral-and-currency-index-updates).

## Checked foreign-index update inputs

The original inert [ForeignUpdateFixture recipe](testing/ForeignUpdateFixture.java)
creates fresh V2016/V2019 Byte/Int16/Int32/Int64/Currency parents and 1,000-row
dependent tables, ascending/descending ordinary aliases, automatic reciprocal
foreign indexes, one-to-one/required controls, multiple child relationships,
declared Currency cascades, self-references and Unicode Memo/OLE values.
Expressions are disabled. Generation uses the reviewed Apache-2.0 Jackcess 5.0.4
JAR SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`,
source pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source or
generated database is tracked; independent creation is not native Access provenance.

| Input | Original SHA-256 | Thirty-nine-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-foreign-updates-v2-2016.accdb` | `22dbff4badb1f6b2f9aef539b172b220274c558da98e0be7e05354e1f7e4e2b7` | `9be8448ac91f4976437bf7943a2f57d8bd120773292181c6cd4a6d9aa3398a8f` |
| `tablecraft-foreign-updates-v2-2019.accdb` | `30c66e413e88750950552ad3232fd88df2d76028cfb9924d9b302231874da5c9` | `5a1a508c5b080272af88cec43e863968f1d690976b36fc70bab758059aa4b158` |

The [runner](testing/compare_foreign_updates.py) compares 78 complete candidates
(twelve exact no-ops), preserving related tables, exact key/row cursor order,
page/count/child/row-reference sets and all bytes outside admitted field/mask/index
spans. Complete Memo/OLE values remain intact. Fifty-four requests reject without
output for missing parents, required/primary NULLs and one-to-one duplicates;
referenced parent changes reject even where cascades are declared. Existing
outputs and originals remain unchanged. Manifest hashes cover ordered candidate
SHA-256 strings joined by newlines without a trailing newline, in runner order.
This proves neither required cascade execution nor Microsoft Access/data-macro,
production-save or mixed-client qualification; see the
[foreign preview contract](testing/ACE16_WRITE_PREVIEWS.md#checked-foreign-index-updates).

## Integral composite index update inputs

The original inert [CompositeUpdateFixture recipe](testing/CompositeUpdateFixture.java)
creates fresh V2016/V2019 mixed Byte/Int16/Int32/Int64/Currency indexes with per-key
directions and shared aliases, a 1,000-row tree, ten-column indexes, unique/
required controls, composite parents/children and Unicode Memo/OLE values.
Expressions are disabled. Generation uses the reviewed Apache-2.0 Jackcess 5.0.4
JAR SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`,
source pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No third-party source or
generated databases are tracked; this is independent creation, not Access provenance.

| Input | Original SHA-256 | Eighteen-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-composite-updates-v1-2016.accdb` | `16ae05023855c47bce9dc319e9e6cec9322b415062dd6d064594f5ef1a8d8f62` | `e3043696ad580c84bd20bea975107ba54ffb45a854848fa64ab22832d49ee7ef` |
| `tablecraft-composite-updates-v1-2019.accdb` | `0e2d4ea61bcc47be08ccb6c79e3a905f2d39c0a9027bcb7aa44594f0f2dd5767` | `0e54966e8014c1115bd73407ca2975469520744dce5227bb8329bbc2956f03b2` |

The [runner](testing/compare_composite_updates.py) matches 36 complete candidates
(four exact no-ops), including composite foreign updates and all/partial NULL
tuples. Fourteen requests reject with expected duplicate/required/orphan/stale
errors and no output. Independent exact rows/index cursors, related tables and
long values agree; page/count/child/row-reference sets and bytes outside admitted
field/mask/index regions remain unchanged. Original inputs and existing output
paths are retained. Manifest hashes cover ordered candidate SHA-256 strings
joined by newlines without a trailing newline, in runner order.

The explicitly selected bounded `check_table` library example completes full
structural and field/relationship checks. The CLI inspector's 16 MiB budget
cannot complete the 1,000-row mixed-table check; it was not increased or silently
treated as passing. The original 21 single-column non-unique candidates also
retain all three previously recorded family manifest hashes after the writer
refactor. Native Access, data macros, required cascades, multi-column assignments,
allocation/splits and production saves remain unqualified; see the
[composite contract](testing/ACE16_WRITE_PREVIEWS.md#integral-composite-index-updates).

## Explicit fixed-field batch inputs

The [batch runner](testing/compare_batch_updates.py) reuses the original
[composite](testing/CompositeUpdateFixture.java) and
[Boolean/NULL-mask](testing/MaskUpdateFixture.java) recipes and their recorded
sources. Both disable expression evaluation during generation. The reviewed
Apache-2.0 Jackcess 5.0.4 JAR SHA-256 is
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`, with source
pin `be0a2139b087a227d14cafd4ea231306a5a3a862`. No new third-party fixture/code
or generated database is tracked. Independent creation is not Access provenance.

| Input | Original SHA-256 | Ordered batch-candidate manifest SHA-256 |
| --- | --- | --- |
| `tablecraft-composite-updates-v1-2016.accdb` | `16ae05023855c47bce9dc319e9e6cec9322b415062dd6d064594f5ef1a8d8f62` | `6fd083983be01677ce71872de91d138b7029baf59f540e992be97d2be62f7792` |
| `tablecraft-composite-updates-v1-2019.accdb` | `0e2d4ea61bcc47be08ccb6c79e3a905f2d39c0a9027bcb7aa44594f0f2dd5767` | `671def8b5928b0e0f8ac82027bf0bc64162d9f1f3236c5f2c0077d522e429b48` |
| `tablecraft-mask-updates-v1-2016.accdb` | `62dbe695e55420e525c7fef58ae2cde2a05ed3e5b8200df9897fbe87fb67553f` | `5d99f5f7d9da11da6c32f125e39ce352086b39fad10bba82650eb3396801c1b1` |
| `tablecraft-mask-updates-v1-2019.accdb` | `d67d2e8e873c58cebce49c1531c7f4d7429f5a12459242df68767d7c176a5e6d` | `7c19633be3538263676be45a15e2aed7a547d457dbe6243ad8d1969dd7b32d6c` |

Eight composite batches per family cover simultaneous tuple/key changes,
composite parent/child edits, full foreign-key clearing, all/partial NULLs,
two-to-ten-component masks, multi-field tree updates and one exact no-op.
Three mask batches per family cover shared Boolean/presence bytes, GUID/IEEE/DATE
and exact integer carriers plus explicit scalar parent/child changes. All 22
complete candidates and 22 reversed-order controls agree, including every
related/untouched table in the comparison corpus, long values, index key/row
order and unchanged page/count/child/row-reference sets. Only selected payload,
mask-bit and admitted index regions change. Fourteen invalid requests reject
without output; original inputs and existing output paths remain intact.

Manifest hashes cover ordered candidate SHA-256 strings joined by newlines without
a trailing newline, in runner order; reverse-order controls are byte-identical
and are not counted twice in manifests. The earlier 84 single-column mask
candidates retain both original canonical row hashes after staging refactoring.
These results qualify explicit in-memory candidate construction, not native
transactions, implicit cascade execution, data macros, production saves or
Microsoft Access interoperability. See the
[batch contract](testing/ACE16_WRITE_PREVIEWS.md#explicit-fixed-field-batches).


## Declared update-cascade inputs

[CascadeUpdateFixture.java](testing/CascadeUpdateFixture.java) is an original,
inert TableCraft recipe generated with the already reviewed Jackcess 5.0.4
artifact (Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`).
No third-party source or fixture bytes are copied into the repository. Fresh
outputs and disposable candidate copies stay outside tracked files. The recipe
creates three-level and overlapping relationship paths, mutual cycles, RESTRICT,
composite keys and independent child uniqueness with different index directions.
Notes/OLE include Unicode and long-value storage. No VBA or active content runs.

| Original | Bytes | SHA-256 | Seven-candidate manifest |
| --- | --- | --- | --- |
| `tablecraft-cascade-update-v1-2016.accdb` | 634,880 | `049c7644091391ace4112d8893cb5a902e1c8f8287ef7a1e6c9b3ba0a7437e74` | `6066675c6d592aa4bdfb029eb24ee82ebe5db1a6ebcd5d0d8367b3a6f5077059` |
| `tablecraft-cascade-update-v1-2019.accdb` | 700,416 | `17dfb758b50b18cc8aea8ad570a26ee294e9e0d9aa5adcd4d291b817cbe8346a` | `496dcd0762f0189c972c01c677c8e40cff2bf1c2d1b373d7dc8db46debb3da89` |

The [runner](testing/compare_cascade_updates.py) independently specifies complete
literal cell closures. Fourteen candidates (two exact no-ops), fourteen
reverse-order controls, and sixteen invalid requests were observed. Every table,
including untouched components, agrees with the independent complete-row reader;
long values and all native index cursors agree. Discovered closures are byte-equal
to explicit full batches. RESTRICT key swaps, conflicting/pinned requests, stale
expectations and repeated cells reject with typed errors, no stdout and no output
file. Existing outputs and original bytes are preserved. Candidate manifests hash
newline-joined candidate SHA-256 strings in runner order, without a final newline;
reverse controls and explicit reference batches are not counted twice.

Run with the reviewed artifact, built CLI and engineering examples:

```sh
javac -cp "$JACKCESS_JAR" -d "$CLASSES" docs/testing/CascadeUpdateFixture.java
java -cp "$JACKCESS_JAR:$CLASSES" CascadeUpdateFixture /tmp/tablecraft-cascade-update-v1-2016.accdb V2016
java -cp "$JACKCESS_JAR:$CLASSES" CascadeUpdateFixture /tmp/tablecraft-cascade-update-v1-2019.accdb V2019
python3 docs/testing/compare_cascade_updates.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_batch --checker target/debug/examples/check_table --ace2016 /tmp/tablecraft-cascade-update-v1-2016.accdb --ace2019 /tmp/tablecraft-cascade-update-v1-2019.accdb
```

Use fresh absent output paths and an existing classes directory; use the platform's
classpath separator. This is independent storage/closure evidence, not actual
Access execution, SQL update ordering, data-macro, delete-action, shared-file or
production-save qualification.


## GUID index update inputs

[GuidUpdateFixture.java](testing/GuidUpdateFixture.java) is an original inert
recipe generated with the reviewed Apache-2.0 Jackcess 5.0.4 artifact (SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`).
No third-party source/fixture bytes are copied. The fixture combines a 1,000-row
GUID tree, unique/ascending/descending keys, GUID and GUID/Int64 foreign keys,
declared update cascades, exact NULL distinctions, Unicode Memo and OLE payloads.
Use fresh absent paths and keep originals immutable; candidates remain disposable.

| Original | Bytes | SHA-256 | Twelve-candidate manifest |
| --- | --- | --- | --- |
| `tablecraft-guid-update-v1-2016.accdb` | 909,312 | `eb0093b9a6c52bfa91460d048d17861528a96cf12b3261c2271f15930842a85d` | `1a24866594a28e52c23d3cde34eebcbb73991ccc160e3a26439757e9d9d691cb` |
| `tablecraft-guid-update-v1-2019.accdb` | 974,848 | `d380dfbbea6d81abd482ab8c3ad99babb460265ec1f98fa1fe961f5b9fe9d543` | `48d0943d955b69a0063af70e2a2623f5b15fe84662d74334c68f9cc12e43010d` |

The [runner](testing/compare_guid_updates.py) uses literal full closures and
requests the parent changes through the update-cascade API. Twenty-four candidates
(two exact no-ops), twenty-four reversed-order controls and ten typed rejections
were observed. Every related/untouched table, exact carrier, long value and native
index cursor matches independent readers. An independent tuple/segment parser
checks page topology, entry counts, child links and physical row-reference sets;
only exact selected field/mask/index spans may differ. Parent swaps, complete
composite-key clearing, unique swaps and within-page NULL growth/shrink succeed.
A divergent key requiring a split, duplicate key, orphan, stale expectation and
repeated cell reject without output. Original bytes and existing outputs remain
unchanged. Manifests hash newline-joined candidate SHA-256 strings in runner order,
without a trailing newline; reversed controls are not counted twice.

```sh
javac -cp "$JACKCESS_JAR" -d "$CLASSES" docs/testing/GuidUpdateFixture.java
java -cp "$JACKCESS_JAR:$CLASSES" GuidUpdateFixture /tmp/tablecraft-guid-update-v1-2016.accdb V2016
java -cp "$JACKCESS_JAR:$CLASSES" GuidUpdateFixture /tmp/tablecraft-guid-update-v1-2019.accdb V2019
python3 docs/testing/compare_guid_updates.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_batch --checker target/debug/examples/check_table --ace2016 /tmp/tablecraft-guid-update-v1-2016.accdb --ace2019 /tmp/tablecraft-guid-update-v1-2019.accdb
```

Use the platform's classpath separator and an existing classes directory. The
shared batch verifier now accepts explicit profile lists and optional discovered
requests; its default composite/mask corpus retains its frozen manifests. The
fixed-key tree parser recognizes GUID segment tags separately from stored widths.
Independent evidence does not qualify native Access execution, data macros,
allocation/splits, delete actions, shared-file safety or production saves.


## Boolean IEEE and DATE index update inputs

[compare_scalar_index_updates.py](testing/compare_scalar_index_updates.py) reuses
the original [Boolean](testing/BooleanIndexFixture.java),
[floating v2](testing/FloatingIndexFixture.java) and
[DATE](testing/DateIndexFixture.java) recipes already reviewed above. They were
generated with the same reviewed Apache-2.0 Jackcess 5.0.4 artifact (SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`).
No source fixture bytes are changed or redistributed. Candidates are fresh copies;
no active content executes and no third-party source is copied.

| Original | Bytes | SHA-256 | Candidate manifest |
| --- | --- | --- | --- |
| `tablecraft-boolean-indexes-v1-2016.accdb` | 729,088 | `ae9ef759d914e58bd54dcd6e0126f15f7624d6db7336497363199097404cb714` | `9f0b2f5b8c1076102729e317b3d4e2b8cab24240d772d7515f786c5ebd2ca277` |
| `tablecraft-boolean-indexes-v1-2019.accdb` | 798,720 | `27489674902f14335482eedad35ebcd3729d11c1927c393a88d63c3bb0eb9b32` | `df1312d297fba1cd0208322f46c1a0473bc063ae0e6f28d036a60c67d9a977c6` |
| `tablecraft-floating-indexes-v2-2016.accdb` | 1,040,384 | `f52b791d2c1e5a151cfb6e7d629bdf4e9cf2ca379436b8b12c90b4a8400ea6eb` | `5d7d7b74be385b64552aefe85eea70bb41c1666e680b5c3860d516deb34fd9b7` |
| `tablecraft-floating-indexes-v2-2019.accdb` | 1,110,016 | `8f51e5d6c1ed947db798e78f25a879d11149f5d2fb7affad673ee1aa7be1af3a` | `87d82f98b456167287466f02a746853c64ed1083352423004a1b09b22f9000e1` |
| `tablecraft-date-indexes-v1-2016.accdb` | 835,584 | `bc7b2887bfc775042a12bc77febae61ca6fc4b4da158fc03a541799f6d46d78a` | `ca700eeaba9e0f6f4967a608f76ac5d35fd29f326a5d279d0883374541012a83` |
| `tablecraft-date-indexes-v1-2019.accdb` | 905,216 | `814ecb93c44ef2c26b716eaa5ddfc81d42a8491663581f007b922d1e20fbb49a` | `aa6d41aa18d4f6d30d37fd3cb36b90b1cc38737800712287565b4204a4042069` |

The observed comparison contains six Boolean, nine IEEE and eight DATE candidates
per family: forty-six total, including six exact no-ops. Forty-six reversed-order
controls agree, and eighteen invalid requests reject without stdout or an output
file. Full 1,000-row trees and complete selected/untouched tables agree with
independent readers. Exact bits, masks, index cursors, topology/entry counts,
child/physical row references and admitted field/mask/index spans are checked.
The independent metadata adapter now distinguishes a Boolean's zero-byte stored
payload from its one-byte index payload. No unrelated source type is reclassified.

Cases include Boolean primary/unique swaps, false in partial-NULL tuples, shared
Boolean masks, signed-zero/NaN-payload uniqueness, exact DATE aliases and sub-
millisecond replacements, mixed GUID/numeric keys, NULL growth/shrink and stale
expectations. Manifests hash newline-joined candidate SHA-256 strings in runner
order, without a trailing newline; reverse controls are not counted twice. Existing
outputs and immutable inputs are retained. Reproduce against existing reviewed
originals and built examples:

```sh
python3 docs/testing/compare_scalar_index_updates.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_batch --checker target/debug/examples/check_table --boolean2016 /tmp/tablecraft-boolean-indexes-v1-2016.accdb --boolean2019 /tmp/tablecraft-boolean-indexes-v1-2019.accdb --floating2016 /tmp/tablecraft-floating-indexes-v2-2016.accdb --floating2019 /tmp/tablecraft-floating-indexes-v2-2019.accdb --date2016 /tmp/tablecraft-date-indexes-v1-2016.accdb --date2019 /tmp/tablecraft-date-indexes-v1-2019.accdb
```

This is independent physical mutation evidence, not Access statement execution,
Boolean/IEEE/DATE relationship actions, data macros, allocation/splits, shared-file
coordination or production-save qualification.

## Native CLI browsing inputs

[compare_browsing.py](testing/compare_browsing.py) reuses the original MIT
[ScalarFixture.java](testing/ScalarFixture.java), CatalogOracle.java,
TableSchemaOracle.java and TableDataOracle.java with the previously reviewed
Jackcess 5.0.4 artifact (SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`).
No third-party implementation or database bytes are copied into production or
redistributed. The generated databases/JAR remain local; original sources are
never modified by comparison or CLI reads.

Fresh originals observed on 19 October 2026:

| Original family | SHA-256 |
| --- | --- |
| V2000 Jet 4 MDB | `9d42dcf68d83b1c92eeb6ee6ec74025203756b587ef705b1f308aed0028886b4` |
| V2016 ACE ACCDB | `6be0caf829aa4131c48caea9f2dbd7b82c21bb1aef11a293a78cbdca634799d5` |
| V2019 ACE ACCDB | `1f9b1fff0d297f91d40a251762d3ac20be9f2c499923a5dee5bc1f8c20b6798f` |

Each family matches the complete independent table-name inventory, five page
controls (default, first row, second row, end-of-file and maximum offset) and an
ordered SELECT over all three scalar rows/14 columns. All original bytes remain
identical afterward. The fixture exercises exact Decimal/Currency extrema,
Unicode and long text, long binary/OLE, GUIDs, dates, Boolean, numeric and NULL
values. This does not establish native Microsoft Access or shared-file behavior.

Build `tablecraft-cli` first, then supply reviewed local inputs and a fresh output
directory:

```sh
python3 docs/testing/compare_browsing.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --directory /tmp/tablecraft-browsing-inputs --generate
```

Input hashes identify the observed originals, not a guarantee of byte-identical
regeneration by the fixture generator. The runner verifies independent content
and unchanged original bytes on each run.

## Larger native browsing-page inputs

[BrowsingFixture.java](testing/BrowsingFixture.java) is an original MIT recipe
using the already reviewed Jackcess 5.0.4 artifact (same SHA-256 as the preceding
browsing section). It creates 20,050 records with a spaced table/column name,
Unicode/escaped text, nullable 28-digit Decimal extrema and long Memo values.
Generated files remain local; no third-party fixture/implementation is copied.

Fresh originals observed on 19 October 2026:

| Original family | SHA-256 |
| --- | --- |
| V2000 Jet 4 MDB | `1789e8b8fc349ed55c1ed3b284f75461f311a5d3785d595eade3cefe76eed01d` |
| V2016 ACE ACCDB | `b7c898ef4a15deae28fc9c169e0081966f704e39ab17dbdb82dde7708c8f006e` |
| V2019 ACE ACCDB | `a5b5b724cef64b1a9e0299493e2021f9da096e1377ece00ea3001d9d86b129c6` |

[compare_browsing_pages.py](testing/compare_browsing_pages.py) enumerates all
20,050 records per family through the independent Java oracle, then checks ten
CLI page controls per family: first/default-sized pages, page-boundary ranges,
long Memo records, offsets beyond 16,384 and end/out-of-range requests. These
controls compare 117 returned rows per family (351 total), not every payload in
the large table. All original bytes remain identical. The earlier small scalar
inventory/page/SELECT comparisons also pass unchanged. Native Microsoft Access,
Windows and concurrent-file behavior remain unqualified.

Build the CLI first and use a fresh generation directory:

```sh
python3 docs/testing/compare_browsing_pages.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --directory /tmp/tablecraft-browsing-pages --generate
```

The hashes identify observed originals; generation is not claimed byte-identical.
The runner verifies original family headers, independently derived page contents
and unchanged source bytes each time. Skipped/unvisited payload qualification and
complete query snapshots are outside this page-reader evidence.

## Native LIKE query inputs

[LikeFixture.java](testing/LikeFixture.java) is an original MIT recipe using the
previously reviewed Jackcess 5.0.4 artifact. Its table contains 28 Text/NULL rows:
empty strings, ASCII case/range examples, digit strings, bracket/wildcard literals,
Unicode/astral characters, newline and trailing space. Generated MDB/ACCDB files
and JARs remain local; no third-party fixture or implementation is copied.

Fresh originals observed on 19 October 2026:

| Original family | SHA-256 |
| --- | --- |
| V2000 Jet 4 MDB | `47ce4d248c9c5ba768f4ffdbccf88f17b2a99d99f473843fe59c0adb50c6ddf4` |
| V2016 ACE ACCDB | `92605c9d82df8e63c3aeec662e80abe79ffaa204b7e845b9dbafa3b16aed10fd` |
| V2019 ACE ACCDB | `0e276feec1c0e1dce389ceabea50024481091ccb85c0302a6eae2f2141e43b0a` |

[compare_like.py](testing/compare_like.py) reads stored labels independently with
TableDataOracle.java, then derives ordinal matches through explicit Python
regular expressions. Per family it checks 68 projection/filter controls over
seventeen pattern pairs and both modes, two NULL-pattern controls and six atomic
invalid-pattern rejections: 76 controls per family, 228 overall. LIKE and NOT LIKE
projection values and filtered IDs agree. All original bytes remain unchanged.
This proves the documented TableCraft scalar profile over native stored text;
Jackcess expression evaluation stays disabled. Neither the Java reader nor the
Python regex models establish native Microsoft Access SQL/locale conformance.

Build the CLI first and use a fresh directory:

```sh
python3 docs/testing/compare_like.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --directory /tmp/tablecraft-like-inputs --generate
```

Hashes identify observed originals, not guaranteed byte-identical regeneration.
The runner verifies original family headers, independent expected results and
unchanged source bytes each time. [Pattern contracts](PATTERN_MATCHING.md) retain
explicit limits for Unicode units, locale expansions and numeric/date coercion.

## Native browsing metadata inputs

[BrowsingMetadataFixture.java](testing/BrowsingMetadataFixture.java) is an original
MIT recipe using the previously reviewed Jackcess 5.0.4 artifact. Each family
contains one ordinary local scalar table and an unresolved non-ODBC linked-table
declaration pointing to a generic unavailable filename. The comparison never
opens that target or a provider. Generated databases/JARs remain local; no copied
third-party fixture/implementation is introduced.

Fresh originals observed on 19 October 2026:

| Original family | SHA-256 |
| --- | --- |
| V2000 Jet 4 MDB | `47ec16bf88a11ebd8ed548fa56144aeaa25839e8ad1a5c289fb2fde7ac3f98d3` |
| V2016 ACE ACCDB | `350f3085b97696d0e08975ab97a6d0319dfcf926d892f3d20d25149ecd2a18a7` |
| V2019 ACE ACCDB | `f60ab46f10eabdb8ef8ac99d305a7a0a8c20e6cc9d2ef05598814568d970a461` |

[compare_browsing_metadata.py](testing/compare_browsing_metadata.py) independently
reads all native table declarations with CatalogOracle.java and local column
metadata with TableSchemaOracle.java. Detailed CLI inventories and column reports
agree; both linked schema inspection and linked row retrieval fail without stdout.
All original bytes remain unchanged. Authored golden metadata separately covers
ODBC declarations, unknown column support, corrupted data pages, row-free schema
inspection and cancellation/resource failures. These are independent/golden
checks, not native Microsoft Access, provider or Windows qualification.

Build the CLI first and use a fresh generation directory:

```sh
python3 docs/testing/compare_browsing_metadata.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --directory /tmp/tablecraft-browsing-metadata --generate
```

Observed hashes do not promise deterministic byte regeneration. The runner checks
original family headers, independent metadata and unchanged bytes on each run.
Metadata support does not prove readable stored records; see [browsing scope](CLI_BROWSING.md).

## Decimal slot and index update inputs

[DecimalUpdateFixture.java](testing/DecimalUpdateFixture.java) is an original MIT
recipe using the previously reviewed Jackcess 5.0.4 artifact. It creates six tables
and 1,020 rows per ACE 16 family: Decimal fields interspersed with long Memo/OLE
values, precisions/scales 4/2, 28/4 and 28/28, ascending/descending and mixed
Decimal/Int64/GUID indexes, a 1,000-row node tree and declared single/composite
update cascades. Expression evaluation stays disabled. No third-party database
or implementation is copied; generated originals/JARs remain local.

Observed immutable originals and ordered candidate-manifest SHA-256 values:

| Original family | Original SHA-256 | Candidate manifest SHA-256 |
| --- | --- | --- |
| V2016 ACE ACCDB | `4312b2ab8c699dd93214b464a4fdbfe4f229aa3c8f2569e67911cd7b59362475` | `1ed780cc56bcedddaf547f24250babf369c3c96d708f7471cb891ecd84f8274b` |
| V2019 ACE ACCDB | `a0a9e221243c29bd9e57666f03bfc9ff02ed3299a9cdb03ef03b8d8688cadb75` | `6ea0fc87b6f18eaad05acd81ded9eaaeb4a8a59302552f0f25f84db341c4dc2b` |

[compare_decimal_updates.py](testing/compare_decimal_updates.py) uses the shared
batch runner and independent Java complete-row/index oracles. Twenty-two complete
candidates include four exact no-ops; 22 reverse-order controls produce identical
bytes/counts. Fourteen invalid precision/scale/resize/duplicate/FK/stale requests
fail without output. Complete related/untouched tables and long values, index
cursors, topology/reference sets and independently derived field/mask/index spans
agree. Existing output paths are retained and original input bytes never change.
The earlier 21 numeric ACE/Jet 4, 24 GUID and 46 Boolean/IEEE/DATE candidate manifests
remain unchanged under the widened private update atom.

The independent field-span proof now decodes controlled native variable offsets
for Decimal slots instead of treating them as fixed-region fields. The controlled
tree proof accepts the documented 17-byte Decimal key payload, separately from
its marker; this does not import production encoder output as expected bytes.
Goldens/properties additionally test native words, directions, precision/scale,
zero-length NULL rejection, retained-slot restoration and negative-zero refusal.

Build the CLI and opt-in engineering examples, then supply fresh paths for the
recipe and immutable originals for comparison:

```sh
java -cp "$JACKCESS_JAR" docs/testing/DecimalUpdateFixture.java new-2016.accdb V2016
java -cp "$JACKCESS_JAR" docs/testing/DecimalUpdateFixture.java new-2019.accdb V2019
python3 docs/testing/compare_decimal_updates.py --jar "$JACKCESS_JAR" --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_batch --checker target/debug/examples/check_table --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

Hashes identify observed originals/candidates, not deterministic fixture generation.
These are engineering byte-operation comparisons, not native Microsoft Access SQL,
save/reopen, data-macro, journal or mixed-client qualification. Ordinary ACE saves
remain blocked; the [preview contract](testing/ACE16_WRITE_PREVIEWS.md#decimal-slot-and-index-write-previews)
retains all limits and the immediate read/query priority is unchanged.

## ACE 2007 browsing priority regression

A fresh local `Scalar-V2007.accdb` was generated on 19 October 2026 with our MIT
[ScalarFixture.java](testing/ScalarFixture.java), reviewed Jackcess 5.0.4 and its
previously recorded artifact/license. No third-party fixture bytes were copied.
Header code is 2, subversion zero, unencoded; size is 364,544 bytes and SHA-256 is
`67680aa053fea89b1dd31df465367fe27a962596007902663b74b13cd7bb2d87`.
The binary remains untracked. The recipe contains three rows and fourteen scalar
columns, including exact Decimal/Currency, dates, Unicode and long text/binary.

[compare_browsing.py](testing/compare_browsing.py) now accepts `--profile V2007`;
its default three-family matrix is unchanged. Independent catalog flags/names,
five bounded page cases and an ordered SELECT match the Java metadata/row oracles.
Inputs retain identical bytes after comparison. The registered example continues
to pass its CLI row/query regression and reports `Ace2007`; its original recorded
hash remains unchanged. Jet 4 and both ACE 16 original scalar/metadata matrices
were also rerun with the new `Kind` column, comparing both native system bits.

Reproduce using the reviewed jar and a fresh local output directory:

```sh
python3 docs/testing/compare_browsing.py --jar "$JACKCESS_JAR" \
  --cli target/debug/tablecraft --directory "$FRESH_FIXTURE_DIR" \
  --profile V2007 --generate
```

These checks qualify the stated bounded readers against an independent
implementation; no native Microsoft Access SQL/locale/application conformance
or shared-file behavior is established.

## Cached calculation page regression

[compare_cached_pages.py](testing/compare_cached_pages.py) reuses the original
[stored Int32](#original-ace-16-stored-calculation-fixtures) and
[stored scalar](#original-ace-16-stored-scalar-calculation-fixtures) recipes and
reviewed Jackcess 5.0.4 artifact. No new binary/source dependency or copied fixture
is adopted. Original hashes remain:

| Recipe/profile | SHA-256 |
| --- | --- |
| CalculatedStorageFixture V2016 | `7b98d314b4d0ce5d22dee5847751aa0198c3d3e2aca05773a47459ecc9304263` |
| CalculatedStorageFixture V2019 | `d16d63cc07cb7a356a0fa21c415af668ffc0a95b9e50bd1ad75ef15cb6d99bc9` |
| CalculatedScalarFixture V2016 | `494160be568484db2ecb9fc08d14bbf2dd8446b8bc9d16e893743128dd01c757` |
| CalculatedScalarFixture V2019 | `b59973b300ad3cff2e73506dc3a7b7ee121dc4611180533274b7359d05772c41` |

Thirty-eight controls compare mixed cached/ordinary pages against independent
stored rows, five ranges per file, empty declarations, ordinary row/query refusals,
bordered labels and bad Boolean wrappers on disposable copies. Carrier coverage
is Boolean/Int32/Double/Currency/DateTime and both Short Text encodings. NULL/empty,
exact numeric bits, stale results, original input bytes and column order agree.
An independent exact DATE serialization oracle supplies the retained bits for a
Python nanosecond display model; no Java millisecond round trip is substituted.
No formula, application object, native Access provider or active content runs.
This is independent reader/presentation evidence, not native Access qualification.

## Mixed multivalue page inputs

[MultivaluePageFixture.java](testing/MultivaluePageFixture.java) is original MIT
repository code generated with the previously reviewed Jackcess 5.0.4 artifact.
Both fresh, untracked inputs have three mixed parent rows and an empty matching
table: ordinary ID/Text, Int32/Text collections and inert cached Int32 results.
Evaluation is disabled. Cached 41 deliberately disagrees with `[ID]*2`; duplicate
and NULL elements, empty collections, Unicode, tabs/newlines and NULL scalars
exercise separate stored states. No third-party fixture bytes are copied.

| Profile | Bytes | SHA-256 |
| --- | --- | --- |
| V2016 | 475,136 | `52d4199f99383cc1bac13020922a9a11addb1881009abdeb8beb4c100e50580b` |
| V2019 | 544,768 | `b1c637eaba2f6a5d432bba9eceac0f887a8a8da9f76d0260b087ecd1ac650105` |

[compare_multivalue_pages.py](testing/compare_multivalue_pages.py) compiles our
[page oracle](testing/MultivaluePageOracle.java) with the existing
[exact collection encoder](testing/MultivalueOracle.java) in a temporary directory.
It also reuses the [eight-carrier originals](#original-ace-16-multivalue-collection-inputs)
without changing their hashes. Independent public scalar/collection reads agree
on 46 page/empty/default-refusal/stale/pretty/overflow/corruption controls across
all four files. Corruption is confined to disposable copies; original bytes stay
unchanged. NaN text is checked as a NaN, without claiming it exposes payload bits.
Other IEEE values round trip to their exact tagged source bits.
No lookup/query/formula or native Access provider runs. This qualifies the bounded
reader/CLI profile against an independent implementation, not native Access
collection query, mutation, application save/reopen or concurrency semantics.

Generation uses fresh paths and the reviewed artifact:

```sh
java -cp "$JACKCESS_JAR" docs/testing/MultivaluePageFixture.java "$FRESH_2016_PATH" V2016
java -cp "$JACKCESS_JAR" docs/testing/MultivaluePageFixture.java "$FRESH_2019_PATH" V2019
```

The runner's `--matrix2016`/`--matrix2019` point to the existing MultivalueFixture
outputs and `--mixed2016`/`--mixed2019` to these new inputs; it also requires
`--jar` and `--cli`. Generated database binaries and Java classes remain local.

## ACE 2007 complex key and multivalue inputs

The existing original MIT [MultivalueFixture.java](testing/MultivalueFixture.java)
and [ComplexRowsFixture.java](testing/ComplexRowsFixture.java) recipes now accept
V2007, with expression evaluation explicitly disabled. Fresh files were generated
using the reviewed Jackcess 5.0.4 artifact; no binary fixture/source is copied.
Files remain local/untracked and originals are immutable. Both headers are code 2,
subversion zero, unencoded. Runtime evaluation settings do not change existing
recorded V2016/V2019 original bytes or hashes.

| Original | Bytes | SHA-256 |
| --- | --- | --- |
| MultivalueFixture V2007 | 589,824 | `79949dc78757784fd828ce89c1df8e83868af27d968e13e7a25bbc2a3384c122` |
| ComplexRowsFixture V2007 | 434,176 | `b5185d278eadd9a27ef3c53520bbd471e23244282c7e9b3792ba69499ab3871a` |

The multivalue file has eight scalar collection types, four parent rows, 137
stored values across 32 collections, 100-item and 101-item controls, exact IEEE/
GUID/Decimal, Unicode, NULLs and duplicates. The complex file has three parent rows
with Text choices, encoded attachments and raw stored versions; its nine scoped
key/count pairs match independent public APIs. Raw encoded storage is distinct
from typed attachment/history interpretation or content execution.

The existing runners accept optional ACE 2007 inputs while retaining the original
ACE 16 cases. `compare_multivalues.py --ace2007 ... --complex2007 ...` matches all
32 collections/137 exact values and twelve orphan/hint corruption rejections.
Its canonical collection hash remains
`a509aee1a4e181767cff52473c80ed1f569800f33f310ead6007e1c4bafec9de`.
`compare_complex_links.py --ace2007 ...` matches nine memberships and twelve
atomic corrupt-copy refusals, with canonical raw-link hash
`7ed873ac68964513734c4006689fe5828dc89cbabfbe13c325f9fe24c941ff92`.
`compare_multivalue_pages.py --matrix2007 ...` adds eleven page/overflow/refusal/
corruption controls, bringing the unchanged original matrix's total to 57.
NaN payloads remain exact in native values; CLI text only identifies NaN.

These results support the bounded ACE 2007 key/multivalue profile and preserve
Large Number, cached-result, typed attachment/history and write gates. They do
not establish native Access collection semantics, lookup evaluation, `.Value`
query expansion, application round trips or concurrent behavior.

Generate on fresh paths with the reviewed artifact, then pass these files and
the existing ACE 16 inputs to the runners above:

```sh
java -cp "$JACKCESS_JAR" docs/testing/MultivalueFixture.java "$FRESH_VALUES_PATH" V2007
java -cp "$JACKCESS_JAR" docs/testing/ComplexRowsFixture.java "$FRESH_COMPLEX_PATH" V2007
```

## ACE 2007 stored attachment and history inputs

The original MIT [AttachmentFixture.java](testing/AttachmentFixture.java) and
[VersionHistoryFixture.java](testing/VersionHistoryFixture.java) now accept V2007
and disable expression evaluation explicitly. Fresh files were generated with the
previously reviewed Jackcess 5.0.4 artifact; no fixture bytes or third-party source
are copied. Both headers are ACE 2007 code 2, subversion zero, unencoded. Binaries
and corruption experiments remain local/untracked; originals remain immutable.

| Original | Bytes | SHA-256 |
| --- | --- | --- |
| AttachmentFixture V2007 | 376,832 | `aec86b0a37e3e4e38e7a32cf5fe1da6293e6ebf681125940f69584c9508aee85` |
| VersionHistoryFixture V2007 | 434,176 | `732d146d17a64cb90b2364c697f12a52f49b295019b409fe732c322c1f7fc382` |

The attachment file contains six stored files plus an empty parent: empty data,
all 256 byte values, Unicode, inert PNG/ZIP signatures and 65,536 compressible
bytes. Four encoded streams use zlib and two are direct. Exact named metadata,
encoded bytes and 65,810 original decoded bytes match the independent public APIs.
`compare_attachments.py --ace2007 ...` retains canonical decoded SHA-256
`4c038c940feaaa94445e667830b6b53007814d42345a594b2ca49f5b897c0995`.
`compare_attachment_columns.py --ace2007 ... --complex2007 ...` matches the metadata
hash `ab8850199b35f9146a6442e92dea93c9ec6111ad25126eb9daac2126dccfa5f2`,
rejects six disposable malformed template/type/reference copies and distinguishes
metadata reads from later invalid-wrapper decoding. Existing ACE 16 results and
original hashes stay unchanged. Stored names/URLs never become opened destinations.

The history file contains eighteen exact stored records across six parents in
two visible-Memo bindings. Signed-zero/subnormal/negative aliases, precision and
DATE endpoints, Unicode/NUL text, empty/NULL text/time, empty membership, duplicate
text and tied timestamps are explicit. `compare_version_history.py --ace2007 ...
--complex2007 ...` agrees with raw storage and separate public API membership/order,
retaining canonical SHA-256
`7791c023ea5586710e7b715722132aee2ae6070adb70a442c097615c7b3d1d5e`.
It rejects six disposable corrupted copies and unqualified visible fields without
publishing output. Stored order remains separate from the reference API's newest-
first/tie-ID policy; no latest-value or ordinary Memo contents are synthesized.

All runners preserve their original required ACE 16 input matrix and accept the
new optional ACE 2007 inputs. Their `--complex2007` refers to the existing
[raw complex input](#ace-2007-complex-key-and-multivalue-inputs). Source generation
uses fresh paths:

```sh
java -cp "$JACKCESS_JAR" docs/testing/AttachmentFixture.java "$FRESH_ATTACHMENTS_PATH" V2007
java -cp "$JACKCESS_JAR" docs/testing/VersionHistoryFixture.java "$FRESH_HISTORY_PATH" V2007
```

This is independent stored-data evidence, not native Access application/query,
mutation or concurrency qualification. No expression, provider or attachment
content is executed, and no existing database is modified.


## Stored-collection CLI verification using existing inputs

The attachment/history comparison runners above additionally check the explicit
CLI `inspect --table ... --attachments ...` and `--version-history ...` views via
[collection_cli.py](testing/collection_cli.py). No new fixture bytes or reference
artifacts are adopted. Comparisons decode actual CLI TSV with Python csv and use
the existing independent Java stored metadata/history results as expectations.
They compare parent membership (including assigned empty keys), identities,
nullable text, encoded attachment lengths, raw flags and exact legacy DATE bits.
Existing disposable nullable/invalid-wrapper copies remain readable metadata;
corrupt templates/references and inappropriate field selections publish no CLI
stdout. The original canonical metadata/history hashes remain unchanged.
These are independent reader and CLI evidence, not native Microsoft Access
qualification, attachment extraction or history mutation evidence.
