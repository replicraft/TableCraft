# Streaming local scalar SELECT queries

`query run --stream --database PATH SQL` explicitly publishes eligible scalar
SELECT/WHERE results in TSV batches. Normal query commands remain buffered.
The shared query service performs evaluation, the native adapter owns cursor
reads, and the CLI owns transport and stdout failures.

```sh
cargo run -p tablecraft-cli -- query run \
  --database docs/examples/Access_Example_VBA.accdb \
  --stream --batch-rows 8 \
  'SELECT ORDR_NBR,NBR_VALUE FROM ROULETTE_TB WHERE ORDR_NBR<=10'
```

One direct local scalar table, aliases, qualified wildcards, typed parameters and
row-local predicates/functions are supported. Source order is retained. Explicit
text, numeric, date, character, ICU and declared-arithmetic policies use the same
host context as buffered retrieval. Ordinary scalar views of mixed schemas are
supported; unavailable fields and matching wildcards fail without NULL placeholders.

Ordering, TOP, DISTINCT/active DISTINCTROW, groups/HAVING, joins/products, sets,
named/derived sources, subqueries, domains, collections, Eval and volatile
clock/random calls remain outside this progressive profile. Single-table
DISTINCTROW retains its ordinary inactive behavior. Unsupported plans refuse
before user-row scans and output, even for empty inputs or false filters.

`--stream` requires a native database and conflicts with saved-query selection,
SQLite, required/automatic indexes, linked/external mappings and stored advanced
field modes. Explicit table format is rejected before opening the input; auto
format is TSV for streaming, including in terminals. Header-based raw native and
supported package/encryption opening retain their existing qualifications; this
mode does not add a codec, provider or template reader.

## Batches and output

`--batch-rows` selects 1..=1,000 rows, default 128, for native fetches and output
batches. It requires `--stream`. Both layers retain at most their current batch;
filtered rows can require multiple native batches before an output batch is ready.

The CLI formats rows with its ordinary escaped display cells and the `csv` crate's
TSV transport. It writes exactly one header, delayed until a complete first output
batch or checked empty EOF. Each complete batch is encoded and checked against
the remaining output allowance before write/flush. Empty results still publish
one header after successful EOF. A failed operation drops its pending output
batch, without flushing that batch merely because earlier rows succeeded.

Later read, evaluation, cancellation, quota or count-check failure can leave
previous complete batches on stdout. A write or flush failure propagates as a
transport error; a physical partial write cannot be rolled back. BrokenPipe keeps
the normal CLI policy. This is why streaming is explicit. Within-row eager
expression order remains unchanged, while competing cross-row native/evaluation
errors can differ from buffered load-before-evaluation order.

## Independent cumulative allowances

Native bytes/rows/work, query allocation/rows/work, per-Text admission, comparison
services and encoded output retain separate cumulative pools. No allowance resets
at a batch boundary. `--output-bytes` retains its 1 MiB default and explicit
16 MiB maximum, counting final CSV expansion across every batch. Native
Memo/encoded-value and query Text overrides keep their existing independent limits.

Streaming bounds live retained memory; it does not refund cumulative allocation
charges after a row or batch is dropped. Larger operations may require explicit
`--native-bytes`, `--native-rows`, `--native-steps`, `--execution-bytes`,
`--execution-rows`, `--execution-steps` and output allowances. Existing positive
ranges and structural ceilings remain in effect. No incomplete result is
published as a complete table cache.

The source is stable/offline and cursor-exclusive during callbacks. This mode
does not supply coherent live Access reads, a cross-file snapshot or permission
to open an embedded path. See the [query service](PROGRESSIVE_QUERIES.md) and
[native cursor contract](NATIVE_PROGRESSIVE_QUERIES.md).

## Verification

Twelve CLI integration tests cover priority synthetic profiles, real ACE 2007
example data, batch sizes, quoting, empty/filter results, first/late errors,
checked count failures, cumulative quotas, parameter/text/declaration controls,
mixed schemas and argument refusals. Sixty CLI unit/browsing/native/retrieval
checks pass, including new write/flush failure and encoded-output allowance tests.
Scoped strict Clippy passes. Original large-file replay and native Access
qualification are recorded separately; this does not complete read/query work.
