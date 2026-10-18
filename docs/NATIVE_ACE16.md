# ACE 16 native compatibility work

ACE 16 is the primary 100% compatibility target. Current implementation is a
bounded read profile, not complete compatibility or Microsoft Access conformance.
`ReadProfile::Ace2016` and `Ace2019` admit unencoded subversion-zero files with
header codes 5 and 6. Code 3, unknown subversions and encoded pages remain outside
these readers. Engine release/build and stored layout remain separate identities.

## Working profile

Catalogs, table/column/index metadata, existing supported scalar rows and long
values, MR2 properties, relationship declarations, explicit supported constraints
and selected Int32 indexes/ranges use the existing bounded native algorithms.
The Access adapter and CLI query run consume them without new host dependencies.
Int32-only physical index rules, text collation policy, size/work ceilings and
unsupported feature errors remain unchanged. Every ACE write stays blocked.
Large Number, Date/Time Extended, calculated/complex fields and native text-index
keys require separate scalar/semantic qualification; profile selection does not
turn unrecognized columns into NULL.

The pinned [format reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java)
and reflection over the reviewed Jackcess 5.0.4 artifact show shared page/row/index
geometry for these families. Compared public format constants differ in default
text sort order from the 2007 family. This does not qualify text key decoding or
Access collation; explicit ordinal query collation remains a different policy.
No third-party implementation was copied.

## Reproducible independent comparisons

Six original fixture recipes now accept V2016/V2019: Scalar, Index, Property,
Relationship, Validation and Definition. The reviewed Jackcess 5.0.4 artifact
generates fresh files; they are not retagged older files. Generation refuses
existing output directories/files. Inputs/JAR remain untracked and are not CI
requirements. [Fixture provenance and hashes](FIXTURES.md) identify observed inputs.

```sh
cargo build -p tablecraft-cli --locked
cargo build -p tablecraft-ace --examples --locked
python3 docs/testing/compare_ace16.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --rows target/debug/examples/table_data --ranges target/debug/examples/index_range --directory new-fixture-directory --generate
```

[The runner](testing/compare_ace16.py) verifies original headers and unchanged
hashes, all five catalog fields (480 records across 12 files), exact mixed scalar
rows, 22 properties, complete relationship inventories, ten child-table integrity
reports, twelve validation-table coverage reports, 28 Int32 range cases, and
200-column chained schema/index/query comparisons on populated/empty tables.
The 2019 template contains two built-in navigation relationships; the independent
oracle uses the API that includes both user and system declarations.

Rust regression tests retag disposable copies of the registered shared-layout
example solely to exercise code paths in CI. They are not ACE 16-authored fixtures
or evidence of actual file conversion. They cover catalogs, scalar/index reads,
audits, malformed pages, cancellation/limits and continued write/codec rejection.
CLI tests exercise indexed queries and reject writes without creating output.

Native Microsoft Access comparisons remain unrun. Next: qualify Large Number
and Date/Time Extended carriers, then further advanced types, collation/index
behavior, table data macros and safe native writes with explicit evidence.
