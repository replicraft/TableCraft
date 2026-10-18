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
stale values and indexed-field updates produce no output. This is independent
engineering evidence, not native Microsoft Access compatibility.

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
headers, exact independent tagged carriers, CLI query behavior, explicit unsupported
BigInt index/calculated-column errors and unchanged input hashes. Synthetic Rust
fixtures separately test profile/width gates, malformed timestamps and CLI output
atomicity. Neither generated nor synthetic fixtures establish native Access
conformance; a Microsoft Access oracle remains unrun.
