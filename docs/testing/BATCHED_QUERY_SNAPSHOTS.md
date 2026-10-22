# Bounded query snapshots through cursor continuation

NativeSource sequential full and projected loads now call
`read_table_batched` / `read_table_projection_batched`. Each qualifies the native
schema and complete usage metadata, then consumes 256-row cursor batches under
one cumulative caller budget. Selected ordinary carriers, row framing, page
ownership and declared live counts are checked before a complete snapshot is
published. Projection retains native logical order and unavailable declarations.
Indexed/range loads keep their existing explicit eligibility and ownership paths.

This removes the 4,096-address vector from sequential query loads. The physical
page-read quota applies separately to each bounded batch. It does not increase
the 16,384-row complete-snapshot ceiling or refill native byte/row/step allowances.
Batch vectors and aggregate row-vector allocations are both charged. Values move
into the aggregate without cloning text/binary payloads. Failure discards the
partial aggregate; there is no visitor publication or cache entry until the
whole load succeeds, including final cancellation/schema checks.

Complete tables/projections still enter their existing caches. Joins, correlated
subqueries and later scans reuse checked values without repeat I/O. Ephemeral
indexed selections keep their own contract. An explicit retry after failed load
starts a new load and spends the original remaining allowance; there is no
automatic retry or budget refund.

The original `read_table` / `read_table_projection` APIs and their existing
consumers retain materialized map limits. Ordinary CLI queries use the new
sequential loading path, but their default 16 MiB native budget, 16,384 cumulative native
rows, one-million steps and buffered 1 MiB output remain. A large-table refusal
can now report cumulative byte exhaustion rather than an address-list structural
limit. No TOP pushdown, streaming execution, native transaction snapshot or
concurrent/shared-file guarantee is implied. All reads require stable/offline input.
Native `query run --database` now accepts an explicit `--native-bytes BYTES`
override within 1..=268435456. Only cumulative native bytes change; all other
ceilings above remain. [The CLI contract](../NATIVE_QUERIES.md) covers scope.

## Evidence

Focused adapter tests run full scans and sorted TOP queries over synthetic
6,000-page tables in Jet 4, ACE 2007, ACE 2016 and ACE 2019, including mixed schemas.
Repeat scans/projected queries reuse complete caches; cached visitor rejection
does no I/O. A host failure after more than 5,000 pages exposes no rows or cache;
an explicit retry completes and is then cached. Smaller cumulative byte/row
budgets fail without partial rows. Engine tests compare independent sequences
at empty, 255/256/257-row and 16,384-row boundaries, enforce the 16,385-row refusal
before user records, check zero-width records and reject final cancellation.

The unchanged [original large-map corpus](ALLOCATION_MAP_PAGING.md) adds six
controls per profile: complete sorted ID results, descending TOP, filtered tail,
Count(*), a bounded correlated query and refusal under the default native budget.
Expected integer rows come from independent read-only Jackcess getters, with
expression evaluation disabled. Successful engineering calls explicitly supply
64 MiB of native bytes; query execution keeps its ordinary limits. Input SHA-256
is checked before/after. No fixture bytes or dependencies were added.
The runner's optional `--cli` repeats all six controls through the ordinary CLI
with `--native-bytes 67108864`, plus two atomic refusals demonstrating independent
query execution and output byte limits. This gives 24 engineering and 32 CLI
controls across the four profiles, with unchanged hashes. Native Access remains unrun.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-access --example query_integers --release --locked --offline
python3 docs/testing/compare_batched_queries.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar \
  --query target/release/examples/query_integers \
  --cli target/release/tablecraft \
  --directory /tmp/existing-large-table-inputs
```

The engineering example uses clap::Parser and explicit read-only File input,
returning only tagged Int32/Int64 rows under a buffered 1 MiB output bound. Its
`--bytes` option belongs to the comparison tool, not the normal CLI. It opens no
providers and executes no VBA, stored calculations or active application content.
Getter agreement checks the stored-value query path; native Microsoft Access SQL,
Windows/live-file behavior and broader backend acceptance remain unqualified.


## Explicit larger consumer limits

Default batched APIs retain the limits above. A new checked
[scalar materialization profile](../LARGER_NATIVE_QUERIES.md) lets a fresh cursor
produce a larger complete snapshot under caller controls. The Access adapter uses
it only with explicit configuration; it never caches a returned suffix from an
advanced cursor or a failed load as a complete table. Native index/collection
structural limits and query/output budgets remain independent.
