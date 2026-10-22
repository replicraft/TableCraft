# Exact floating mean verification and timings

Floating `Avg`/`DAvg` now sum submitted finite binary64 values exactly before one
nearest-even rational-to-binary64 conversion. Single inputs convert exactly to
Double first. Repeated maximum values no longer overflow an intermediate Double
sum, and tiny residuals survive cancellation across the full finite exponent range.
This is an explicit current numerical policy, not native ACE precision qualification.

Each exponent has a signed i128 significand bin. A binary64 significand has at most
53 bits; the existing positive i64 count bounds its accumulated coefficient below
i128 range. Bins use the `2^-1074` unit; only final conversion builds bounded
BigInts, with num-rational owning quotient rounding. No custom big-integer division
or decimal arithmetic is introduced. A single nonzero term with an exactly
representable count uses one hardware division, with the same rounding target.

About 38 KiB of bin/seen/occupied storage is charged before fallible allocation.
The executor reuses scratch between groups and aggregate columns; nested active
means acquire separate storage. Only occupied bins are reset. Final integer
buffers and visits are charged under the same cumulative query budget, with
cancellation checkpoints. No second value buffer or second argument-evaluation
pass is needed. Scratch never survives an execution call.

Pure floating-input means are independent of record encounter order. An earlier
integral/Currency prefix still converts through the existing explicit Double
conversion when floating mode begins. An explicit Decimal operand retains the
separate Decimal policy, including bounded conversion of a preceding floating
total. Those mixed-type/Decimal rules and native signed-zero/precision behavior
are not made complete by this change. Exact cancellation to zero yields positive
zero in this profile. Source values keep their original bits.

## Independent correctness

Original Rust tests cover finite means of `f64::MAX`, cancellation residuals down
to subnormal units, ties-to-even, all finite exponent bins, order permutations,
argument volatility, scratch reuse under a 1 MiB budget, quotas and cancellation.
Synthetic ACE 16-family native rows exercise the same extreme cases with
metadata-only preparation. [Python Fraction controls](compare_floating_average.py)
check 129 source-free CLI cases using exact input binary fractions and one final
conversion, including deterministic full-width random inputs. The existing 69
Currency rational controls also pass. No third-party fixture or Access result
is copied or inferred from these checks.

## Query-level benchmark

The [harness](../../crates/query/tablecraft-query/benches/averages.rs) uses immutable
MemorySource data and reusable plans. It verifies warm-up results, excludes source
construction/parsing/binding from per-query timings, and includes result drop.
Both revisions use exactly the same harness. The baseline engine is `8ae34fc`;
the candidate is this change. Rust 1.99.0 release builds ran on Darwin arm64.
Five paired, alternating process samples each time 20 executions per case.

| Case | Rows / groups | Before median | After median | Observed change |
| --- | --- | --- | --- | --- |
| One group | 4,096 / 1 | 0.836 ms | 0.790 ms | 0.046 ms lower |
| Many small groups | 2,048 / 128 | 2.982 ms | 3.101 ms | 0.120 ms higher (about 4%) |

Hyperfine 2.0.0 additionally measured seven process runs after two warm-ups:
83.0 ± 0.9 ms before and 84.2 ± 0.9 ms after for the complete two-case harness.
These short local timings do not establish a general speed gain. The small-group
cost and overlapping process samples remain in the result, rather than being
hidden behind the one-group improvement. File I/O and native Access are outside
this benchmark. Initial exploratory samples, superseded before the single-term
fast path, are retained separately and excluded from the final comparison.

[Raw paired samples and complete Hyperfine measurements](results/floating-means-2026-10-22.json)
retain timing/memory fields; only executable labels replace temporary absolute
paths. Reproduce with `CARGO_INCREMENTAL=0 cargo bench -p tablecraft-query --bench averages --locked`.

The numeric library's [Ratio conversion API](https://docs.rs/num-rational/0.4.2/num_rational/struct.Ratio.html)
and reviewed `ratio_to_f64` implementation supply final nearest-even rounding.
This is not an independent audit of all upstream arithmetic. Native Access
comparison and broader mixed-type compatibility remain roadmap work.
