# Exact floating sums

Floating Sum/DSum now share the bounded binary accumulator used by Avg/DAvg.
Submitted finite Single values convert exactly to Double. Floating values are
summed exactly in exponent bins, then converted once to Double with nearest-even
rounding. This is the current numerical profile, not native ACE aggregation
precision qualification. Microsoft documents [Sum](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/sum-function-microsoft-access-sql)
over numeric field expressions with NULL records omitted; that does not specify
our exact accumulator or result-carrier choices.

For example, summing `1e308`, `1e-300`, `-1e308` now retains `1e-300`.
`MAX_DOUBLE + MAX_DOUBLE - MAX_DOUBLE` as aggregate inputs returns MAX_DOUBLE;
a final total outside finite Double still fails with Overflow. Ordinary binary
addition retains its existing operator behavior. Aggregate arguments evaluate
once in encounter order. Their operational failures propagate before final sum
conversion; a deferred floating total overflow cannot hide a later argument
failure. Results and CLI stdout remain buffered.

## Carriers and mixed inputs

Pure floating sums are independent of record encounter order. Exact cancellation
and all-zero input produce positive zero. NULL does not contribute; empty/all-NULL
groups return NULL. Successful Single/Double sums retain Double result metadata.
Preparation reads no rows to choose a result or accumulator.

Integral, Boolean, Currency and Decimal sums retain their checked scalar fold.
An integral prefix is checked as Int64, then must convert exactly to Double when
floating mode begins. Later integral values retain the strict exact-conversion
guard. Integer-only intermediate overflow is still an error. An explicit Decimal
after a floating prefix first materializes the rounded Double prefix, then uses
the existing checked Decimal conversion/fold. Once Decimal is selected, later
floating values keep that Decimal policy. Currency mixed with Single/Double now
uses the existing CDbl converter for Currency operands and for a checked Currency
prefix when floating mode begins. This is exact scaled-decimal to nearest binary64
conversion, rather than casting the scaled integer before division. Both encounter
orders work; a pure Currency sum retains Currency. A fixed Currency prefix can
still overflow before a later float, and converting a combined prefix can round
differently from converting its individual operands. This does not establish
order independence for heterogeneous inputs. Dates, Text and other
unsupported carriers do not become numeric or NULL. These mixed-type rules and
full Variant/native aggregate promotion remain unfinished.

## Resources and shared ownership

The aggregate feature owns the shared binary Scratch module and the dedicated
Sum state; the mean keeps its own Decimal/wide-total behavior. The execution-local
numeric scratch pool now serves sums and means, including domain calls, groups
and aggregate columns. Nested active accumulators acquire separate scratch.
There is no second row buffer or second argument-evaluation pass.

About 38 KiB of bin/seen/occupied storage is charged before fallible allocation.
Only occupied bins are reset on reuse. Bins have i128 coefficients bounded by the
checked i64 input count. Final bounded BigInt buffers and visits share the query
allocation/work budget and cancellation callback; num-rational owns final
binary64 conversion. Scratch dies with the execution. No dependency, public core
carrier, native layout or writer boundary changes.

## Verification

Rust controls cover signed extremes, intermediate-overflow cancellation,
subnormal residuals, ties-to-even, all finite exponent bins, order permutations,
all-NULL groups, mean/sum pool reuse under a 1 MiB budget, exhausted allocations,
every observed cancellation checkpoint and volatile arguments evaluated once.
Fixed-point overflow, Currency extrema/conversion, integral precision guards, Decimal
bridging and late operand failures retain explicit tests.

Synthetic native rows cover Jet 4, ACE 2007 and both ACE 16-family layouts, with
ordinary and domain sums and metadata-only binding. The original
[Fraction runner](compare_floating_sum.py) passes 155 independent controls:
131 explicit/random/permuted CLI cases and 24 native query controls over
registered 20,000-row files. Raw Jackcess Single/Double getters supply input bits;
Python Fraction sums them exactly and rounds once. Sequential and ascending/
descending ID-index reads agree, including expressions that cancel the large
field values and expose the remaining subnormal total. All original hashes/bytes
remain unchanged.
Only TableCraft executes SQL. These are independent numerical/native-reader
observations, not Microsoft Access execution, error-order or application evidence.

```sh
python3 docs/testing/compare_floating_sum.py \
  --cli target/debug/tablecraft --jar path/to/reviewed/jackcess-5.0.4.jar \
  --directory path/to/registered-large-indexes
```

## Benchmark

The [release harness](../../crates/query/tablecraft-query/benches/floating_sums.rs)
checks warm results, reuses metadata/immutable source rows, excludes setup/binding
from per-query timings and includes result destruction. It measures floating
one-group and many-group cases plus an integral control. Both revisions use the
identical harness; the baseline is `26c1983`. Native I/O is outside this benchmark.
The [raw record](results/floating-sums-2026-10-22.json) retains all samples and
separates exploratory measurements from the final comparison.


Six final alternating release pairs on Darwin arm64 measured about **15 microseconds
more per 4,096-row single-group query**, and **111 microseconds more per
2,048-row/128-group query**. These are small local costs for exact accumulation,
not speed improvements.

| Case | Baseline median | Candidate median | Observed change |
| --- | --- | --- | --- |
| 4,096 floating rows / one group | 0.913 ms | 0.927 ms | +0.015 ms, about 1.6% |
| 2,048 floating rows / 128 groups | 3.193 ms | 3.304 ms | +0.111 ms, about 3.5% |
| 4,096 integral rows / one group | 0.900 ms | 0.891 ms | Within observed spread |

Hyperfine 2.0.0 measured seven whole-harness process runs after two warm-ups:
109.4 ± 1.5 ms baseline and 112.4 ± 3.6 ms candidate. Those process measurements
include setup, binding, warm validation and all timed repetitions; they do not
replace per-query results. Both distributions and peak resident-memory fields
are retained in the raw record. Initial exploratory samples overlapped query
verification/Clippy compilation and are explicitly excluded from the final
comparison, with their complete raw data retained. Final samples ran after those
processes completed. The baseline one-group outlier and all final ties/spread
remain recorded; no broad throughput or native-file claim is made.

```sh
CARGO_INCREMENTAL=0 cargo bench -p tablecraft-query --bench floating_sums --locked
```

## Currency/floating integration

The mixed Currency/floating route reuses the existing explicit Double converter;
it adds no dependency or new decimal implementation. Microsoft's
[Sum reference](https://support.microsoft.com/en-us/access/functions/sum-function)
admits numeric expressions and
[conversion reference](https://support.microsoft.com/en-us/access/type-conversion-functions)
defines CDbl and Currency ranges. Neither establishes this heterogeneous aggregate
carrier/prefix policy; native Access qualification remains pending. Pure Currency
folds, strict integral precision admission and Decimal precedence remain unchanged.

Dedicated Rust controls cover both Single/Double encounter orders, typed Currency
fields including scaled i64 extrema, NULL, DSum and checked pure-prefix overflow.
The independent Fraction runner now passes 331 controls (the prior 155 plus
152 Currency conversion and 24 mixed native query cases). It exercises exact conversion from
scaled decimal strings and dynamically selected native Money/Single/Double fields
in sequential and both ID-index orders. Only TableCraft executes these queries.

A separate [regression measurement](results/currency-sums-2026-10-22.json) against
`4429d23` uses the identical existing harness: six alternating release pairs and
seven Hyperfine runs after two warm-ups. Median one-group floating queries were
0.917 versus 0.914 ms; 128-group floating queries were 3.250 versus 3.280 ms
(about 30 microseconds more); integral controls were 0.889 versus 0.893 ms.
Whole-process Hyperfine means were 108.7 ± 0.3 versus 109.0 ± 1.2 ms. All samples
and executable/harness hashes are retained, with no exclusions. This measures
existing-path regression costs, not new Currency-path throughput: the baseline
refuses those mixed inputs. Native I/O and native Access remain outside the timing.
