# Modern ACE 16 scalar histories and scattered indexed reads

The original [recipe](ModernScalarHistoryFixture.java) creates fresh inert
ACE 2016 and ACE 2019 layout-family files with the reviewed Jackcess 5.0.4 writer.
Both contain Large Number fields; the 2019 layout additionally contains
Date/Time Extended. The generator explicitly disables expression evaluation and
executes no SQL, VBA, macro or external provider. Native Access qualification
remains separate from this independent engineering evidence.

Each file inserts 20,000 identities in scattered order, adds fields halfway
through, deletes every 101st original identity, updates surviving multiples of
97 through the current schema, then appends 100 identities. Each retains 19,902
rows. Fourteen initial fields become seventeen in the 2016 layout; the initial
and later Extended fields make fifteen/nineteen in the 2019 layout. Both cross a
null-mask boundary. Later Large Number fields exercise historical fixed-prefix
changes alongside existing variable text/payload fields and Boolean masks.

Large Numbers include values near both Int64 endpoints, above and below the
exact-integer Double boundary, outside Int32 and ordinary small values. Updates
reverse their sign without overflowing. Extended dates include years 0001/9999,
leap days, seven fractional digits and one-tick changes; added dates differ by
nine ticks. NULLs, absent added fields, false Booleans, empty OLE, Unicode and
grown Memo/4,097-byte OLE payloads are intentional. The original bytes are retained
under ignored persistent repository storage; no binary is committed.

## Observed index issue and implementation

The first full WideAsc query failed with StructureLimit after exhausting a
single 4,096-page-read allowance while following scattered record references.
This was separate from the explicit retained-row and cumulative resource limits.
Sequential cursors and exact independent getters already agreed on all values.

Explicit IndexReadRequest calls now finish tree/map validation under its original
pass quota, then fetch referenced records in predetermined 256-row groups. Each
group has the existing 4,096-read quota. The caller's byte/row/work pools accumulate
through every pass/group and are never refilled. Unsupported/corrupt indexes still
fail without sequential fallback. One current data-page buffer can survive group
boundaries; validation of each root, relocated target, row and hidden key remains.
The entire result stays unpublished until completion.

Legacy index entry points retain their operation-wide quota and default table
ceiling. Mapped-page/depth/key/row ceilings remain independent. This is bounded
snapshot loading, not a streaming index API, coherent live-file view, cache or
performance optimization. Explicit CLI native/execution budgets still govern
these requests. No new library, data carrier, write permission or format is added.

## Verification

The [runner](compare_modern_scalar_histories.py) passes 37 controls. Python derives
every expected value and identity independently; read-only Jackcess getters
establish stored values, sequential order and index order. Engine cursors at
127/257 rows and reversed-name projected cursors at 131 rows match those getters.
Complete CLI streaming, late buffered pages, appended/updated TOP queries,
counts/null predicates and Large Number sequential/indexed exact seeks agree.
The 2019 profile also checks Extended date ordering and retained fractions.
Default native resource refusal retains empty buffered stdout. Both registered
hashes remain unchanged; only TableCraft executes SQL.

Two additional Rust tests use 20,000 transposed physical records across 100 pages,
with increasing/decreasing keys in all four priority read layouts. They verify
complete hidden-key projections under explicit budgets and refusal when a later
group exhausts the original byte pool. ACE 16 cases also inject late row/key
corruption and cancellation around group boundaries and the final record.
No failing operation returns a partial snapshot. Existing tree/projection/I/O,
adapter cache and CLI index checks remain applicable.

Native Access SQL/coercion, independent writer index maintenance versus Access,
other history shapes/types, broader file locality, coherent shared-file reads
and save/reopen remain unqualified. These files do not establish all databases
up to the one-million-row consumer ceiling or arbitrary modern-field evolution.

```sh
python3 docs/testing/compare_modern_scalar_histories.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --rows target/debug/examples/table_data \
  --directory .cache/tablecraft-fixtures/modern-scalar-histories
```

Fresh generation uses the Java recipe with a new output path and V2016/V2019.
Existing paths are refused. Generation timestamps can change hashes; review and
register new identities before comparison reuse rather than weakening hash checks.
[Provenance](FIXTURES.md#modern-ace-16-scalar-history-inputs) records this generation.
