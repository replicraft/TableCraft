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
