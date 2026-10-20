# Decimal arithmetic measurements

Measured on Darwin arm64, rustc 1.99.0, release defaults, on 19 October 2026.
Hyperfine used two warmups and seven measured runs per mode, with 10,000,000
operations per process. Every run exited successfully.

| Workload | Batch mean ± sample deviation | Amortized time per operation |
| --- | --- | --- |
| Int64 add/subtract/multiply mix | 50.798 ± 0.652 ms | 5.08 ns |
| Decimal add/subtract/multiply mix | 125.650 ± 1.402 ms | 12.57 ns |
| Decimal division | 388.908 ± 1.889 ms | 38.89 ns |

These whole-process elapsed times include startup, argument parsing, setup
assertions, loop dispatch, checksum and output. They are divided by operation
count, not isolated instruction timings or measured query throughput. Initial
one-million-operation runs had a larger startup contribution; the recorded runs
use ten million to reduce it. No own Cargo build or comparison runner was active
alongside the recorded measurements. Results are synthetic and do not establish
an end-to-end engine speedup or native Access compatibility.

The clap Parser example at
crates/query/tablecraft-expressions/examples/decimal_arithmetic.rs uses fixed
operands 12345 and 6789. Integer and Decimal modes cycle Add/Subtract/Multiply
with the same numeric operands and public checked arithmetic API, differing in
result carrier/range contract. Decimal mode uses scale-zero inputs; these numbers
do not characterize every 96-bit magnitude or scale combination. Division repeats
12345/6789; its independent rational golden coefficient is
18183826778612461334511710119 at scale 28. Setup checks exact integer/Decimal
results for all mixed operations and the division golden before timing loops.
Inputs/operators/results use black_box, and each result contributes to a wrapping
i128 coefficient checksum. One loop iteration performs one operation.

```sh
CARGO_INCREMENTAL=0 cargo build --release --locked --offline -p tablecraft-expressions --example decimal_arithmetic
hyperfine --warmup 2 --runs 7 --export-json /tmp/tablecraft-decimal-arithmetic-hyperfine.json \
  'target/release/examples/decimal_arithmetic --mode integer --iterations 10000000' \
  'target/release/examples/decimal_arithmetic --mode decimal --iterations 10000000' \
  'target/release/examples/decimal_arithmetic --mode division --iterations 10000000'
```

The example adds no dependency: it reuses existing development-only clap and
normal expression/core APIs. Independent i128/rational properties, midpoint/range
and error goldens, query/CLI integration and original native-file regressions
provide separate correctness evidence. See [contracts](../EXPRESSIONS.md#checked-decimal-arithmetic)
and [fixture evidence](../FIXTURES.md#checked-decimal-arithmetic-read-regression).
