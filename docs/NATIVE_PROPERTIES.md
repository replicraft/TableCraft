# Native property inspection

`inspect_table_properties` inventories inert table, column and index property
blocks for one explicit local table. CLI: `tablecraft inspect PATH --table NAME
--properties`. The option requires a table and conflicts with indexes/checking.
Output reports scope, key, native type, original flags and Boolean/text values;
other values are `OPAQUE`. Existing buffering/escaping/output limits apply.
Errors and tracing omit values, names and paths. Nothing is executed.

## Implemented profile

The existing unencoded Jet 4/ACE 2007, subversion-zero reader projects only Id and
LvProp from MSysObjects. Unrelated module/object bodies are not decoded. Catalog
property values spend the shared long-value/read budgets. The target's complete
MR2 stream, including unknown types/flags and padding, remains available; this is
metadata retention, not qualification of native save preservation.

Framing, name indexes, lengths and UTF-16 names are checked. Column/index scopes
may share a name. Duplicate keys/groups/names fail. Missing streams/keys remain
absent. Property Booleans use one payload byte, independently of row null masks.
Text/Memo values remain inert expression text. Typed access rejects wrong carriers;
Required is never guessed false because its payload has an unexpected type.
Ordinary schema/row APIs keep their conservative nullability policy.

Unknown block tags are retained only when they fit the understood value framing;
other layouts fail. KKD/Jet 3 and newer ACE streams remain unqualified. Existing
limits include 64 KiB long values; this parser adds 256 property names, 4,096 total
entries and at most one table group plus 256 column and 256 index groups.
Allocations, row visits and work accumulate; cancellation is checked between
catalog/page/row/block/property units. No linked provider or active content runs.

## References and independent comparison

Reviewed [Jackcess PropertyMaps](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/PropertyMaps.java)
and [JetFormat](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java)
for MR2 block/name/value framing, scope tags and Boolean storage. These Apache-2.0
sources are technical references; no implementation was copied. Microsoft field
rule semantics require separate qualification before enforcement.

[PropertyFixture.java](testing/PropertyFixture.java) is an original repository-
licensed inert recipe using the previously reviewed Jackcess 5.0.4 artifact. It
refuses existing outputs and declares Required, AllowZeroLength, DefaultValue,
ValidationRule/Text, Unicode Caption and an opaque binary FutureFlag.

```sh
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/PropertyFixture.java new.accdb V2007
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/PropertyFixture.java new.mdb V2000
python3 docs/testing/compare_properties.py --cli target/debug/tablecraft --jar path/to/jackcess-5.0.4.jar --ace new.accdb --jet new.mdb
```

All 11 declarations per format match the independent reader, including type/DDL
flags, quoted defaults and Unicode. Canonical sorted cells have SHA-256
`6789bc96c462395e112408c2b87dc376d0f081450867f78f00a63f6a04702d8a`.
Real-fixture tests retain Required/AllowZeroLength/opaque GUID metadata without
changing bytes. Independent goldens cover shared column/index names, unknown
values/flags, malformed framing/references, duplicates, cancellation and budgets;
a 256-case fixed-seed arbitrary-stream property checks bounded decoding.

Generated files/JARs are not tracked or CI inputs. [Fixtures](FIXTURES.md) identifies
the observed inputs. [Bounded rule audits](NATIVE_CONSTRAINTS.md) are now implemented separately.
Default evaluation, native mutation, application preservation and Microsoft
Access comparison remain unfinished.
