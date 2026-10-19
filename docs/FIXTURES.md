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
all thirteen adapter models per family. Query-name-prefixed canonical SQL SHA-256:
`71cf898b621915f2f302330707f99494d399912066d53fd5a32e7ecaf2a5709e`.
Eight copies per family reject unknown attributes/flags, conflicting native type,
unhandled clause alias, duplicate/reversed/NULL projection order and late declared
row counts, without stdout. Missing/table namespaces also reject.

The registered example separately asserts an observed bare leading-digit name in
the reference rendering and compares the adapter with an explicitly quoted expected
source identity. That spelling is outside the SQL parser's current profile; this
regression is not counted among whole-renderer comparisons. Original sample bytes
and hash remain unchanged. [The contract](NATIVE_SAVED_QUERIES.md#owned-select-reconstruction)
records the narrow metadata/order/identifier profile. Saved-query execution,
native Access conformance and application save/reopen remain unqualified.
