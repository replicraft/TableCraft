# Inert saved-query fragments

`read_saved_query_definition` now reads one explicitly named query's stored
MSysQueries fragments from a stable/offline source. It returns an owned
`SavedQueryDefinition` with its scoped model object, unmodified native origin and
selected `SavedQueryRow` records. It does not reconstruct, validate or execute SQL,
activate a provider, evaluate expressions or start application content.

The catalog lookup requires `ObjectKind::Query`. Native signed object IDs match
MSysQueries.ObjectId exactly, including negative IDs; they are not table/page
addresses and receive no page-number masking. Caller-issued catalog scopes remain
separate from native IDs. A table or absent name reports UnknownObject; an existing
query with no stored rows returns an explicit empty definition.

## Stored field profile

The current system-table profile requires exactly eight named scalar fields:

| Native field | Retained carrier |
| --- | --- |
| ObjectId | Int32 matched exactly to the selected native query ID |
| Attribute | Nullable Byte mapped to QueryAttribute, including Unknown(code) |
| Expression | Nullable owned Text |
| Flag | Nullable signed Int16, without defaulting or interpreting bits |
| LvExtra | Nullable signed Int32 scalar, separate from object payload streams |
| Name1 / Name2 | Nullable owned Text, without identifier/path interpretation |
| Order | Nullable owned Binary, without sorting or coercion |

Native attribute codes 0–11 describe start/type/parameter/flags/remote-database/
table/column/join/filter/group/having/order fragments; 255 is the end tag. Every
other byte remains an explicit unknown tag. NULL, empty text, literal `NULL`,
empty Binary, signed minima and unknown flags remain distinct. Rows retain
sequential storage order; Order bytes do not become an implicit clause sort.
Strings and binary buffers move out of the validated scalar snapshot without
cloning their payloads. Formatting remains at the executable boundary.

The reader scans the complete bounded system table before selecting its rows,
so other queries also consume the caller's read/row/work/allocation allowances.
Selected row-vector allocation is charged and reserved fallibly before conversion;
each scan/field visit checks bounded work and cancellation. Native geometry,
declared row counts, scalar decoding and schema errors publish no definition.
Normal reader ceilings and the qualified unencoded Jet 4/ACE 2/5/6 selector remain.
ACE 16-family original fixtures are the main qualification target; the existing
ACE-code-2 application supplies a separate observed regression case.

This is metadata inspection, not query/index integrity validation. The reviewed
template has a required composite ObjectId/Attribute/Order index. Nullable getters
preserve inspectable stored fields even when those semantic constraints are broken.
Rows without the selected ObjectId are omitted without being coerced to zero;
global orphan fragments, required headers, duplicate clauses, type/flag consistency,
dependencies and executable query completeness are not audited. Empty/unknown/
incomplete metadata does not authorize SQL execution or a later write.

## CLI

```sh
tablecraft inspect copy.accdb --query 'Saved Query Name'
```

`--query` conflicts with all table/catalog/relationship/check modes. The report
buffers query name, native ID, raw flags, fragment count and the seven remaining
fields before stdout. Text is quoted/escaped; NULL is unquoted, empty Text is `""`
and empty Binary is `0x`. Borrowed text/binary formatting retains the existing
1 MiB output cap without cloning payloads. Errors exit 2 without partial output;
verbose diagnostics omit names, paths and expressions.

`query run` still accepts its documented scalar table sources; it does not treat
a saved query name as a table. SQL reconstruction belongs to a future explicit
native adapter that feeds TableCraft's owned syntax/model. Upstream parser ASTs
remain inside the SQL adapter. Action, DDL, pass-through and unknown fragments
are data in this inspection mode and never run.

## References and evidence

The reviewed Jackcess 5.0.4 [QueryImpl.Row source reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
and [QueryFormat reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryFormat.java)
provide native field/carrier/tag facts. Actual artifact bytecode and public
metadata getters were checked; no upstream implementation was copied.

[The original fixture recipe](testing/SavedQueryFixture.java) writes inert system
rows into fresh V2016/V2019 databases through the reviewed artifact. Eleven
definitions/75 fragments per family cover all nine query-object flag families,
unknown attributes, signed flags/extra fields, Unicode/NUL/control text, NULL/
empty distinctions, nonmonotonic order bytes and an empty definition. These are
stored fragment fixtures, not a claim that the illustrative SQL is executable.

[The read-only oracle](testing/SavedQueryOracle.java) uses query identity metadata
and raw stored rows with expression evaluation disabled; it never invokes
toSQLString or query execution. [Comparison](testing/compare_saved_queries.py)
matches all 75 fragments per family and all 59 queries/332 fragments in the
unchanged registered example. Unknown-tag, NULL-attribute, NULL-order and unbound
row copies still match inert inspection. Incompatible carrier/late-count copies
fail without stdout. [Provenance](FIXTURES.md#original-ace-16-saved-query-fragment-inputs)
records original file/report hashes and current limits; no native Access oracle
or application save/reopen has run.

Five native integration tests cover signed/scoped identities, namespace rejection,
unknown/nullable fields, empty definitions, exact carrier failures, geometry,
budgets/cancellation and a 128-case generated representation model. A unit sweep
round-trips all 256 attribute bytes. Three CLI tests verify inert exact output,
NULL quoting, payload-free diagnostics, every conflicting mode and buffered late
failures. The shared output refactor retains existing value formatting tests.

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedQueryFixture.java new-2016.accdb V2016
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedQueryFixture.java new-2019.accdb V2019
python3 docs/testing/compare_saved_queries.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb \
  --sample docs/examples/Access_Example_VBA.accdb
```
