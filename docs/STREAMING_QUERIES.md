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
row-local predicates/functions and positive unordered TOP counts are supported.
Global `Count(*)` projections over the same direct table are also supported,
with one checked count row published only after successful EOF. Pure final
expressions and prior projection aliases may compose the count. `Count(expr)`
and other aggregates remain outside this progressive profile.
Flat UNION ALL can combine these count arms and direct-table row arms.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Count(*) AS Population FROM Customers WHERE Active=True'
```

A count arm retains its counter and current input batch, then computes its final
projection after checked EOF. Empty or wholly filtered input produces a zero-count
row. A later error in another UNION arm can retain that count only if its output
batch was already completed; a pending batch is discarded. Native/query/output
allowances remain cumulative, and ordinary buffered counts keep their existing
snapshot behavior.

Source order is retained. Explicit text, numeric, date, character, ICU and
declared-arithmetic policies use the same
host context as buffered retrieval. Ordinary scalar views of mixed schemas are
supported; unavailable fields and matching wildcards fail without NULL placeholders.

Ordering, TOP PERCENT, DISTINCT/active DISTINCTROW, groups/HAVING, joins/products,
duplicate-eliminating sets, nested UNIONs, named/derived sources, subqueries,
domains, collections, Eval and volatile clock/random calls remain outside this
progressive profile. Single-table
DISTINCTROW retains its ordinary inactive behavior. Unsupported plans refuse
before user-row scans and output, even for empty inputs or false filters.

`--stream` requires a native database and conflicts with saved-query selection,
SQLite, required/automatic indexes, linked/external mappings and stored advanced
field modes. Explicit table format is rejected before opening the input; auto
format is TSV for streaming, including in terminals. Header-based raw native and
supported package/encryption opening retain their existing qualifications; this
mode does not add a codec, provider or template reader.

## Flat UNION ALL

Eligible arms stream in written order through one cumulative query/native
allowance and one TSV batch buffer. A batch may cross an arm boundary. Every
selected schema and arm shape is preflighted before output, and an arm is checked
again before its cursor starts. The existing UNION result conversions occur
before publishing each retained row, including explicit numeric-to-Text policy
requirements and profile-dependent numeric promotion. Output headings come from
the first arm. These are the same conversion kernels as buffered queries, not
new native type or policy qualifications.

TOP limits each arm independently while retaining full EOF/eager validation.
Later conversion, expression, read, output or EOF failures retain only previously
completed TSV batches; the remaining batch is discarded. Source-free/nested UNION
arms and the other unsupported shapes above do not become eligible through an
empty earlier arm or false predicate.

## TOP limits output, not validation

`SELECT TOP n ...` without ORDER BY streams the first n qualifying rows in source
order. It still reads to checked EOF and eagerly computes later WHERE/projection
expressions. Later read, evaluation, count, cancellation and resource errors
remain failures, potentially after an earlier output batch. This limits exported
rows and output bytes; it does not promise less source work. Ordered TOP and
PERCENT remain refused in streaming mode, and TOP 0 keeps its existing binder
refusal. Ordinary buffered TOP behavior is unchanged.

Discarded expression cells are not strictly published after TOP, preserving the
buffered handling of unrepresentable raw String intermediates in omitted rows.
Their construction, eager branch errors and work/allocation charges still apply.
See [the query TOP contract](PROGRESSIVE_QUERIES.md#unordered-top-counts) for the
primary reference and engineering/native qualification boundary.

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

Twenty-three CLI integration tests cover priority synthetic profiles, real ACE 2007
example data, batch sizes, quoting, empty/filter results, first/late errors,
checked count failures, cumulative quotas, parameter/text/declaration controls,
mixed schemas, unordered TOP prefixes and argument refusals. TOP tests preserve
late expression/EOF/quota failures and omitted raw String publication parity.
UNION ALL controls additionally check ordered arms, empty results, per-arm TOP,
batches spanning arms, late failures and explicit numeric-to-Text conversions.
Global-count controls additionally cover zero/filtered input, aliases, mixed row
and count arms, EOF-only publication, quotas and batch-dependent retained prefixes.
Five alias CLI and nine retrieval regressions also pass. Earlier CLI integration
also recorded sixty unit/browsing/native/retrieval
checks, including write/flush failure and encoded-output allowance tests.
Current scoped query/CLI strict Clippy passes. Original large-file replay and native Access
qualification are recorded separately; this does not complete read/query work.
