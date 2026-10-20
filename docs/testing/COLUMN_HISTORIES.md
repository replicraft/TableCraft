# Deleted-column row histories

The reader now handles older rows with deleted fixed/variable declarations when
the surviving logical identities and definition counts establish row geometry.
Column numbers select null-mask bits; fixed offsets locate fixed bytes; variable
indexes select offset pairs. These identities are not visible vector positions.
Present fixed fields bound the required fixed-data region. Missing logical numbers
before/after the stored row count bound how many deleted variable slots could have
existed. Surviving present variable fields also establish a required tail prefix.
The stored variable count must satisfy those bounds, and every offset retains its
existing ordering/bounds checks. No deleted type or payload value is invented.

If zero or more variable slots could have existed but no surviving field requires
a tail, the historical frame is ambiguous and returns Unsupported. This also
applies to zero-width projections: projection does not waive framing validation.
For example, current fields 0 and 3 with deleted fields 1 and 2 do not necessarily
identify whether an old count of 2 included a deleted variable field. The writer
that made the original input knows; the final native declarations do not.

Absent current fields remain NULL or FALSE for Boolean; default expressions do
not run. Deleted bytes are ignored rather than reassigned to visible fields.
Zero/future row counts remain unqualified; duplicate surviving variable slots now
fail as malformed metadata. Equal-count geometry retains its previous acceptance
rules. Native edit gates reject gapped/deleted declarations even when an update
targets a current row. This read qualification does not enable DROP/ALTER, type
changes, index mutation on historical storage or native Access save compatibility.

## Reference and fixture boundary

The previously reviewed/pinned [Jackcess TableImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/TableImpl.java)
locates fields using native fixed/variable positions and writes slots for deleted
variables. [ColumnImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java)
distinguishes column identities and storage offsets. These Apache-2.0 sources are
format references; production code and test recipes are original Rust/Java/Python.
The [upstream deleted-column mutation issue](https://github.com/spannm/jackcess/issues/10)
reinforces the separation between read and write qualification. The reviewed
5.0.4 JAR only creates append-only originals before deletion; getters open derived
files read-only, without appending columns, updating rows or evaluating expressions.

[ColumnHistoryFixture.java](ColumnHistoryFixture.java) creates original histories
in V2000, V2007, V2016 and V2019: ID, removed Int32, removed Text, retained Unicode
Text, Boolean and Int32, with rows inserted between schema additions. A separate
[original Python recipe](compare_column_histories.py) removes selected declaration
blocks/names on fresh copies, updating the active count and definition length.
It keeps maximum logical/variable counts, native identities, all row bytes and
all other pages intact. Indexed/chained table definitions are refused by this
engineering recipe. It is not a production native write API or an Access-generated
deletion/save fixture. Existing opaque property declarations are not rewritten.

FixedGap and VariableGap remove one field each. MixedGap removes both without a
row at the ambiguous intermediate count. AmbiguousGap includes that row and must
fail. [Fixture records](../FIXTURES.md#deleted-column-history-derivatives) retain all
base/derivative identities. Neither original files nor derived inputs are tracked.

Each profile passes twelve positive controls (three tables through full tagged
reads, cursor batches, ordinary CLI rows and projected queries) and three atomic
ambiguous-history refusals. Independent expected values come from read-only
Jackcess getters. Both original and derivative SHA-256 remain unchanged across
checks. Native Microsoft Access, DDL/save/reopen, Windows and broader schema-history
qualification remain unrun; passing independent getters does not establish them.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example table_data --release --locked --offline
CARGO_INCREMENTAL=0 cargo build -p tablecraft-cli --release --locked --offline
python3 docs/testing/compare_column_histories.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --rows target/release/examples/table_data \
  --directory /tmp/new-column-history-inputs --generate
```

Omit --generate to compare existing immutable inputs. Generation refuses an
existing directory; original bases are retained separately from derivatives.
