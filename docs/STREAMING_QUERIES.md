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
Global `Count(*)` and `Count(expr)` projections over the same direct table are
also supported, with one checked count row published only after successful EOF.
Pure final expressions and prior projection aliases may compose the counts.
`Sum(expr)` and `Avg(expr)` are admitted under the numeric profile below;
`Min(expr)` and `Max(expr)` use the retained-value profile below. Other
aggregates remain outside this progressive profile.
Flat UNION ALL can combine eligible aggregate and row arms. Static row-local
named and inline derived SELECT chains can supply each arm as described below.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Count(*) AS Population FROM Customers WHERE Active=True'
```

A count arm retains one counter per written Count occurrence and its current
input batch, then computes its final projection after checked EOF. Count operands
run for each qualifying row in projection order; repeated occurrences run
separately. `Count(expr)` excludes SQL NULL, while zero, False, empty Text, Error
values and raw String intermediates count as present. Count operands cannot
reference earlier output aliases or contain nested aggregates. Operand failures
occur during the scan; surrounding scalar projection failures occur after EOF.
Competing failures can therefore differ from buffered aggregate evaluation order. Empty or wholly filtered input produces a zero-count
row. A later error in another UNION arm can retain that count only if its output
batch was already completed; a pending batch is discarded. Native/query/output
allowances remain cumulative, and ordinary buffered counts keep their existing
snapshot behavior.

Source order is retained. Explicit text, numeric, date, character, ICU and
declared-arithmetic policies use the same
host context as buffered retrieval. Ordinary scalar views of mixed schemas are
supported; unavailable fields and matching wildcards fail without NULL placeholders.

Ordering, TOP PERCENT, unproven/Text DISTINCT, active DISTINCTROW, nonaggregate HAVING, joins/products,
duplicate-eliminating sets, nested UNIONs, scalar/predicate subqueries, domains,
collections, Eval and volatile clock/random calls remain outside this progressive
profile. Static named/derived SELECT chains can contain row-local stages and the
same eligible aggregate/grouped/HAVING stages under the
[retained stage contract](PROGRESSIVE_QUERIES.md#stateful-retained-stages). Single-table
DISTINCTROW retains its ordinary inactive behavior. Unsupported plans refuse
before user-row scans and output, even for empty inputs or false filters.

`--stream` requires a native database and conflicts with SQLite, required/automatic
indexes and collection modes. Explicit `--saved`, `--external` and local
`--recomputed-column` selection can supply eligible streaming plans.
Explicit table format is rejected before opening the input; auto format is TSV for streaming, including in terminals. Header-based raw native and
supported package/encryption opening retain their existing qualifications; this
mode does not add a codec, provider or template reader.

## Terminal DISTINCT over exact fixed carriers

A terminal `SELECT DISTINCT` can eliminate duplicate projected tuples with proven
Boolean, Byte, Integer, Long, LongLong, Currency, Decimal or NULL output types.
The active materialized/declared execution profile determines admission; runtime
values never invent a missing type guarantee. Text, floating, Date, GUID, Binary
and Dynamic output profiles remain refused before scans. This does not admit
inner DISTINCT stages, active DISTINCTROW, sorting or duplicate-eliminating UNION.

Every selected projection remains eager. The service retains first-encounter
representatives and a metered exact-key directory, then waits for complete input
and upstream logical EOF before any DISTINCT row is published. Decimal values
compare numerically across scales while retaining the first value's actual scale
and carrier. Numeric/NULL comparison policy cohorts remain conservative; no Text
comparison provider is activated merely for numeric duplicate removal.

```sql
SELECT DISTINCT CustomerId FROM Orders
SELECT DISTINCT TOP 10 CCur(Amount) FROM Orders
SELECT DISTINCT Count(*) FROM Orders GROUP BY CustomerId
```

TOP selects the first distinct rows after duplicate removal. Later input and final
projections still run even when TOP has already found enough representatives.
Source EOF failures, later projection failures and failed upstream stage
completion prevent all output from that DISTINCT arm. A failed final sink stops
later representatives and UNION ALL arms, preserving its original typed error.

An arm's DISTINCT uses its own actual projected cells and active descriptors
before the existing outer UNION ALL conversion. It adds no cast: distinct Int64
values can therefore survive as separate rows even when the outer UNION later
rounds both to the same Double. Static row-local and admitted stateful inputs,
global/grouped aggregates and HAVING can supply the terminal DISTINCT projection.
Memory and work remain bounded by the existing cumulative query limits; retained
representatives do not form a complete source or named-result cache. Normal
buffered DISTINCT behavior is unchanged.

Twelve focused DISTINCT tests pass within a 223-test progressive lifecycle batch.
They include generated 100,003-row input, exact fixed-point representatives,
post-deduplication TOP, active-profile admission, all-arm preflight, logical EOF
failures and sink/cancellation/quota controls. Scoped strict query linting passes. Three new native CLI controls pass within
its 58-test progressive target; see the [implementation verification record](testing/results/read-query-progressive-distinct-2026-10-25.json).
One obsolete grouped-query refusal was updated to an unsupported Double result;
its failed batch is retained separately and not counted twice. Original-file CLI
replay and native Access SQL qualification remain separate evidence.

## HAVING filters

Eligible terminal global/grouped SELECTs can use HAVING with the current
Count/Sum/Avg/Min/Max profile, including aggregates absent from the output list.
The predicate runs after checked EOF. False or NULL suppresses every selected
expression and selected aggregate operation; `SELECT Sum(1/0) FROM Items HAVING
False` therefore does not evaluate the selected division. Projection aliases
remain unavailable in HAVING under the existing binder.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT ID Mod 17 AS Bucket,Count(*) AS N FROM Orders GROUP BY ID Mod 17 HAVING Count(*)>10'
```

HAVING aggregate states scan normally. When selected aggregates have operands,
the query retains input cells under its cumulative allocation budget and replays
those operands only for accepted groups. These queries can need memory
proportional to selected inputs; they do not create a complete source or named
cache. Pure key/scalar and Count(*)-only outputs avoid per-input replay rows.
Queries without HAVING retain their current allocation paths.

TOP counts accepted groups, while later accepted groups still perform eager
SELECT work and can fail after earlier batches. Rejected groups remain suppressed.
Failed source EOF prevents all HAVING results. Aggregate input processing is
row-wise, so competing error order can differ from buffered execution; see
[exact suppression and retained-state scope](PROGRESSIVE_QUERIES.md#terminal-having-and-deferred-selected-operands).
Inner pipeline stages still require row-local SELECTs; HAVING does not enable
inner grouping or expand the admitted group-key families.

## Static named and derived inputs

An eligible arm can read a linear chain of static named queries or inline derived
SELECTs ending at one qualified native scalar table. Each inner layer admits
row-local projections, WHERE and positive unordered TOP. All selected inner
projections remain eager, including outputs unused by an outer filter or count.
Each layer owns its TOP counter; reaching that cap still validates its later
input through checked EOF. Retained intermediate cells preserve their actual
carriers and comparison policies without premature display conversion.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Count(*),Sum(Amount) FROM (SELECT Amount FROM Orders WHERE Active=True) Q'
tablecraft query run --database example.accdb --stream --saved ActiveOrders
```

`--saved` reconstructs the explicitly selected root definition before preparation;
it does not activate other saved content during file inspection. Named definitions
retain their lexical source ownership. Inner aggregates, GROUP BY, UNION,
crosstabs, ORDER BY, DISTINCT/active DISTINCTROW and data-dependent schema discovery
remain refused. These restrictions apply throughout the input chain, even behind
an empty input or false filter. The existing outer global/grouped aggregates and
root flat UNION ALL remain available. Parser, binder depth and other structural
ceilings still apply to every nested query and expression.

The pipeline does not create a complete intermediate table or promote a prefix
into a named-result cache. Errors are checked in pipeline encounter order, which
can differ from buffered materialization order. Earlier complete TSV batches can
remain after a later inner expression, source or output failure.

## Explicit external and recomputed inputs

Explicit external mappings also supply eligible static input chains:

```sh
tablecraft query run --database frontend.accdb --external remote backend.accdb \
  --stream 'SELECT Id,Name FROM People IN "remote"'
```

The exact token selects an already authorized target. Stored paths never open
files, target linked tables remain refused, and native files share the cumulative
read allowance. External named definitions retain their own lexical namespace;
explicit selectors inside them still use the operation root's authorized registry.
No provider or automatic link traversal is added. See
[external sources](NATIVE_EXTERNAL_QUERIES.md).

Local `--recomputed-column VIEW TABLE COLUMN` views can stream through the same
row-local pipeline. Every selected formula stays eager before outer predicates,
and cached values remain separate. Natural recomputed carriers are not coerced
to stored ResultType. This CLI mode remains incompatible with linked/external
mappings; see [recomputed views](RECOMPUTED_CALCULATION_VIEWS.md).

## Explicit linked tables

`--stream --link ALIAS PATH` can stream a qualified one-hop native linked table
through the same explicitly opened target used by buffered queries. Stored paths
remain inert; missing mappings and targets that are themselves linked fail
explicitly. No target names enter the root query namespace.

```sh
tablecraft query run --database frontend.accdb --link Customers backend.accdb \
  --stream --batch-rows 128 'SELECT ID,Name FROM Customers'
```

Targets use the supplied password, stored-calculation opt-in and native read
options. Database comparison retains each target field's own policy and the root
literal default. Native/query/comparison/output budgets remain cumulative across
root and mapped sources and repeated UNION ALL arms. Snapshot retention limits
do not truncate a progressive cursor, and neither EOF nor a failed suffix
publishes a complete table cache. The single-table/flat UNION ALL shape limits
above still apply; mapping a target does not admit joins, collections, network
providers or live cross-file consistency. See [linked reads](NATIVE_LINKED_QUERIES.md).

## Global SUM and AVG

`Sum(expr)` and `Avg(expr)` can share a global aggregate SELECT with Count calls
and pure final expressions. The input is scanned once through checked EOF, using
one independently metered state per written occurrence. Existing numeric kernels
supply exact totals, rounding, carrier admission and prefix-overflow behavior.
NULL operands are omitted; empty or wholly NULL input produces NULL for Sum/Avg.
Unlike Count, numeric aggregates retain their existing refusal of Error data and
unrepresentable raw String operands.

Operands run after WHERE, once per occurrence in written order on each row. After
successful EOF, all aggregate states finish in occurrence order before final
scalar expressions and aliases execute. Competing errors can differ from the
buffered aggregate-major order. No aggregate result publishes before successful
completion of those phases.

Concurrent floating aggregates each need separate scratch storage during the
scan. Byte thresholds can therefore differ from buffered execution, which can
reuse scratch sequentially. Existing cumulative byte/row/work limits remain in
force; streaming does not imply a small fixed allocation for every aggregate.
Sorting, statistics and unsupported inner shapes remain outside this progressive
profile; eligible static aggregate stages can compose through retained cells.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Count(*) AS N,Sum(Amount) AS Total,Avg(Amount) AS Mean FROM Orders'
```

## Global MIN and MAX

`Min(expr)` and `Max(expr)` can share a global SELECT with Count/Sum/Avg,
pure final expressions and earlier projection aliases. They use the same
retained cells and comparison rules as buffered extrema: actual numeric carriers,
String policy/completion, raw String intermediates and admitted Error data survive
until their consumers. NULL operands are omitted; empty or wholly NULL populations
produce NULL. Existing unsupported comparisons still refuse.

Resolver-dependent String candidates remain retained until checked EOF so that
comparison uses the complete actual policy cohort. This can require storage
proportional to the selected population; streaming does not promise constant
memory. Other admitted comparisons retain the selected cell. Native, query and
output quotas remain separate and cumulative.

Operands run in written occurrence order per qualifying row. After checked EOF,
all states finish before final projections and strict scalar publication. No
result from this arm publishes after an input, comparison or final projection
failure; already completed earlier UNION batches can remain. These phases can
produce a different competing-error priority from buffered execution. See the
[query contract](PROGRESSIVE_QUERIES.md).

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Min(Amount) AS Lowest,Max(Amount) AS Highest,Count(*) AS N FROM Orders'
```

## Grouped results

Eligible `GROUP BY` expressions with proven Text, Boolean, integral, Currency, Decimal or NULL carriers
can stream the same native tables, explicit mapped targets and eligible
static named/derived input chains, including eligible aggregate stages. Tuple keys,
nullable values, pure row-local key expressions, prior projection aliases and
flat UNION ALL retain their existing contracts. Floating, date and
other unproven key families and ordering remain outside this progressive
profile. Unsupported shapes refuse before user-row scans; ordinary buffered
grouping remains available.

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT ID Mod 17 AS Bucket,Count(*) AS N,Sum(Amount) AS Total FROM Orders GROUP BY ID Mod 17'
```

For non-Text keys without HAVING, the cursor discards input rows after WHERE,
key evaluation and aggregate updates.
Each encountered group retains its first key tuple and independent accumulator
states. All current Count/Sum/Avg/Min/Max argument behavior remains available;
String extrema can retain candidates until final policy comparison, so memory
is not promised to be constant. Group keys retain complete policy cohorts and
natural carriers without inventing a representative input row for final expressions.
Native/query/comparison/output limits remain separate and cumulative.

Checked EOF precedes every group result. Groups finish and publish in first
encounter order. Unordered TOP caps publication while omitted groups still run
final calculations; a late calculation or sink failure can retain completed
earlier batches. Empty or wholly filtered input produces no grouped rows.
This is a progressive execution contract and does not qualify native Access
error precedence or optimizer behavior. See the [query service](PROGRESSIVE_QUERIES.md).

## Text group membership

Text grouping retains each qualified key tuple until checked EOF. It first
applies the complete comparison-policy cohort to every key, then partitions the
whole population with the existing strict SQL comparer. A later policy can change
which earlier strings belong together; no provisional groups or Text hashes are
used. First spelling, raw String ranges, NULL equality and tuple comparison order
retain their existing contracts. Explicit comparison policies remain required
when an actual comparison needs one; binding does not resolve providers.

```sh
tablecraft query run --database example.accdb --stream --collation database \
  'SELECT Category,Count(*) AS N FROM Items GROUP BY Category HAVING Count(*)>10'
```

Count(*)-only and key/scalar outputs retain key candidates and group counts,
without source-row replay frames. Queries with aggregate operands retain paid
input rows for replay after membership is complete. HAVING still suppresses all
selected work for rejected groups. Key storage includes owned String backing
buffers; it grows with the selected input population even when there are few
groups. Membership can take quadratic work with many distinct Text keys, so
streaming does not promise fixed memory or arbitrary-size grouping.

Source EOF and every membership comparison precede aggregate operands and output
in this mode. A late key-policy error prevents all results for that arm. A later
final projection or sink error can leave earlier completed batches. These phases
are explicit; cross-row error priority need not match other progressive modes
or native Access. Existing numeric-key allocation paths remain separate. See
[the query contract](PROGRESSIVE_QUERIES.md).

## Nested aggregate stages

A static saved/named or inline derived SELECT can group or aggregate before a
later SELECT filters, groups or aggregates its retained results:

```sh
tablecraft query run --database example.accdb --stream \
  'SELECT Sum(N) AS TotalRows,Count(*) AS Buckets FROM (SELECT ID Mod 17 AS K,Count(*) AS N FROM Items GROUP BY ID Mod 17) Q'
```

The inner stage completes its entire input before finishing. Only after every
inner final expression succeeds can an outer aggregate finish. TOP limits delivery
at its own stage while retaining omitted selected expressions' eager work; an
outer false filter cannot hide an inner error. Empty global aggregates deliver one
row, while empty keyed grouping delivers none. Intermediate raw Strings, Error
values, declarations and comparison policies stay intact until their actual
consumers. Only terminal output applies scalar publication and root UNION
conversions. Native scanned-row counts count physical input, not intermediate rows.

The same cumulative allowances apply to every stage. Text grouping, HAVING replay
and String extrema can retain input-dependent state; nested streaming does not
promise constant memory. A late inner error may leave earlier completed row
batches but cannot publish a misleading completed outer total. Inner UNION,
ordering, joins and schema discovery remain unsupported in streaming.

## Explicit stored calculations

`--stream --stored-calculations` admits qualified stored scalar calculation
results through the existing cached-value cursor. Values may be stale: formulas
are never evaluated, and this option does not certify freshness. Unselected cached
fields remain pruned, including for a pure `Count(*)`. Selected malformed caches
fail explicitly; complete earlier TSV batches can remain while the pending batch
is discarded. No progressive scan publishes a complete snapshot cache.

```sh
tablecraft query run --database example.accdb --stream --stored-calculations \
  'SELECT ID,Score FROM Items'
```

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

Thirty-six ordinary progressive CLI integration tests cover priority synthetic profiles, real ACE 2007
example data, batch sizes, quoting, empty/filter results, first/late errors,
checked count failures, cumulative quotas, parameter/text/declaration controls,
mixed schemas, unordered TOP prefixes and argument refusals. TOP tests preserve
late expression/EOF/quota failures and omitted raw String publication parity.
UNION ALL controls additionally check ordered arms, empty results, per-arm TOP,
batches spanning arms, late failures and explicit numeric-to-Text conversions.
Global-count controls additionally cover zero/filtered input, aliases, mixed row
and count arms, EOF-only publication, quotas and batch-dependent retained prefixes.
Operand controls additionally cover NULL/raw String/Error values, occurrence
identity and eager failure prefixes. Numeric aggregate controls cover Sum/Avg/Count
composition, aliases, empty populations, mixed UNION arms, eager failures and
opted-in cached inputs. Stored-calculation controls cover explicit
opt-in, stale values, inert formulas, Count/filter/UNION execution, selected cache
failures and pruning. The preceding stored-stream checkpoint also passed fourteen
calculation-preview regressions. Five alias CLI and nine retrieval regressions
also pass. Earlier CLI integration recorded sixty unit/browsing/native/retrieval
checks, including write/flush failure and encoded-output allowance tests.
Current scoped query/CLI strict Clippy passes. Original large-file replay and native Access
qualification are recorded separately; this does not complete read/query work.

Explicit linked streaming additionally passes thirteen linked-query CLI controls
and thirty-five native linked/progressive controls, with strict Clippy. Four
new CLI tests and eight new native tests cover routing, repeated aggregate/row
UNION arms, late prefixes, pruning, EOF, shared limits and refusal boundaries.
Existing option controls also exercise password authentication, cached-value
opt-in and target field/root literal comparison policies in streaming mode.
See the [scoped verification](testing/results/read-query-linked-progressive-2026-10-25.json).

Global Min/Max adds four CLI and three native cursor controls, including retained
raw/Error consumers, explicit Text comparison, EOF-only publication and failure
prefixes. The complete scoped checkpoint passes 338 distinct query/native/CLI
tests with strict Clippy and minimal query features; see
[verification](testing/results/read-query-progressive-extrema-2026-10-25.json).
