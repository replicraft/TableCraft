# Native index inspection

`inspect_table_indexes` returns a completed IndexInspection with owning table schema
and native logical index definitions. CLI: `inspect COPY --table NAME --indexes`.
It uses the existing bounded, unencoded Jet 4 / ACE-code-2, one-page table-definition
profile. Index pages and rows are not traversed by inspection.

Logical index numbers, physical data numbers, root/usage references, raw type/flags,
column numbers and direction are retained separately. Multiple logical indexes
can refer to physical index data. Native definition order is preserved; it can
differ from Jackcess's logical-number order. Column numbers resolve against native
logical column numbers, including current metadata gaps. This metadata is not a
schema uniqueness/foreign-key validation result or proof of a valid index tree.

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
partial inspection. Chained definitions remain unsupported.

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
fixture bytes remain unchanged. Index writes, native constraints, automatic query
access-path selection and shared-file coordination are not implemented here.
