# Mixed scalar storage histories

The original [recipe](MixedScalarHistoryFixture.java) creates offline ACE 16,
ACE 2007 and Jet 4 databases using the reviewed Jackcess 5.0.4 artifact. It does
not execute Access SQL, formulas, macros or VBA. This extends the larger
[scalar query evidence](../access/queries/LARGER_NATIVE_QUERIES.md); it does not change supported
types, limits, concurrency guarantees or write permissions.

Each table starts with fourteen fields: Int32 identity, Boolean, Byte, Int16,
Currency, Single, Double, Unicode Text, Memo, Binary, OLE, GUID, Decimal and legacy
DATE. It inserts 20,000 identities in a deterministic scattered order. Halfway
through insertion it adds Int32, Text and Currency fields, crossing the
sixteen-column null-mask boundary. Earlier records retain absent fields.

Generation then deletes every 101st original identity, updates every remaining
97th identity through the complete current schema, and appends 100 identities.
Updates grow non-NULL Memo and OLE payloads to 768 UTF-16 units plus their prefix
and 8,193 bytes respectively. They also populate previously absent late fields.
Each final table retains 19,902 rows. NULL scalar fields, false Booleans, empty
Binary/OLE values, signed floating zero, exact binary fractions, large Decimal
coefficients, negative legacy dates and Unicode surrogate pairs are intentional.
Historical bytes and native allocation are produced by the reference writer;
this is not an Access-generated schema/save fixture.

The [runner](compare_mixed_scalar_histories.py) checks every stored value against
an independent Python construction, then compares full engine cursors at two
batch sizes, reversed-name projections and complete CLI progressive rows against
read-only Jackcess getters. Expected identity sets are independent of physical
order; cursor and CLI sequence must agree with the reference reader. DATE inputs
use exactly representable days, noon and quarter-day values. The getter's DATE
re-encoding is not a raw-bit oracle for arbitrary fractional native dates.

Buffered rows, descending TOP over appended rows, TOP over grown historical
records, count/sum/null predicates, Currency totals, year groups and deleted-row
absence also pass. Default source limits and explicit byte exhaustion retain
empty buffered stdout. Progressive publication retains its existing separate
contract. Every input hash remains unchanged after all reading.

The three profiles pass 45 controls. These are independent engineering checks,
not Microsoft Access query results, arbitrary row/type evolution, million-row
throughput, encrypted large-file qualification, live-file coherence, native
index maintenance or save/reopen evidence. Those acceptance gaps remain open.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example table_data \
  -p tablecraft-cli --locked --offline
python3 docs/testing/compare_mixed_scalar_histories.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --rows target/debug/examples/table_data \
  --directory .cache/tablecraft-fixtures/mixed-scalar-histories
```

Reuse verifies the [registered identities](FIXTURES.md#mixed-scalar-storage-history-inputs)
before reference/database reads. For fresh engineering generation, supply a new
directory and `--generate`; that directory must not exist. Metadata timestamps
can change generated hashes, so fresh generation reports new identities rather
than silently accepting them as the registered originals. Keep generated inputs
under ignored persistent repository storage, not a temporary OS directory.
