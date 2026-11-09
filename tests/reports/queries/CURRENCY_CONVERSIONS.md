# Currency conversion measurements

Measured locally on Darwin arm64 with rustc 1.99.0 (b940084d7), release defaults,
on 19 October 2026. Hyperfine used two warmups and seven measured runs per command;
each process performed 1,000,000 conversions. Every run exited successfully.

| Mode | Batch mean ± sample deviation | Amortized time per conversion |
| --- | --- | --- |
| Existing integer-kernel reference, Decimal inputs | 14.212 ± 0.341 ms | 14.2 ns |
| rust_decimal-backed CCur, same Decimal inputs | 53.037 ± 0.320 ms | 53.0 ns |
| rust_decimal-backed CCur, Double inputs | 511.100 ± 3.826 ms | 511.1 ns |

These are whole-process elapsed times divided by the iteration count, including
startup, argument parsing, setup assertions, loop/checksum and final output. They
are not isolated instruction timings or end-to-end query throughput. Decimal and
Double modes use different inputs, so their numbers do not establish a relative
cost for interchangeable workloads. The Decimal reference is faster for this
workload; the production library supplies the selected checked decimal boundary.
No own Cargo build was running alongside the measured commands.

The clap Parser example lives at
crates/query/tablecraft-expressions/examples/currency_conversion.rs. Both Decimal
modes cycle through coefficient/scale pairs (123455,5), (-123445,5),
(90071992547409931200,8) and (1,28), covering midpoint rounding, sign, values beyond
Double's exact-integer range and tiny scale. The benchmark-only reference shifts
scale by four then uses the pre-existing half-even integer coercion kernel.
Setup asserts both implementations agree on all four inputs. Double mode uses
1.25, -0.03125, 549755813888 and the smallest positive subnormal, with explicit
scaled-result assertions. Borrowed inputs/results use black_box and contribute
to a wrapping i128 checksum. One loop iteration performs one conversion.

```sh
CARGO_INCREMENTAL=0 cargo build --release --locked --offline -p tablecraft-expressions --example currency_conversion
hyperfine --warmup 2 --runs 7 --export-json /tmp/tablecraft-currency-hyperfine.json \
  'target/release/examples/currency_conversion --mode reference --iterations 1000000' \
  'target/release/examples/currency_conversion --mode library --iterations 1000000' \
  'target/release/examples/currency_conversion --mode floating --iterations 1000000'
```

Independent exact binary-rational property tests, Decimal bounds/ties and Currency
identity tests accompany the kernel. Native scalar input regressions are recorded
in [FIXTURES.md](FIXTURES.md#explicit-currency-conversion-read-regression).
Neither this benchmark nor those independent models qualify Microsoft Access/OleAut
rounding, locale conversions or native query behavior. See the
[expression contract](../expressions/EXPRESSIONS.md#explicit-currency-conversion).
