# Bounded raw-byte search experiment

The `tablecraft-expressions` `byte_search` example compares three borrowed-byte
search kernels. This is a development experiment: production InStrB and its
public `find_binary` callback/work contract remain unchanged.

| Contender | Operation | Work policy |
| --- | --- | --- |
| scalar | Existing per-window, per-compared-byte search | One entry unit, one per candidate and one per compared byte; tests check counts against production |
| bounded | Scalar first candidate; memmem spans up to 4096 bytes for needles up to 32 bytes | Precharge span bytes plus needle bytes before each bounded search; longer needles retain the exact scalar fallback |
| reference | One whole-haystack memmem call | One entry unit only; no bounded cancellation or search work metering; unsuitable for production policy |

All return the first raw byte offset. Odd buffers, unaligned matches and unpaired
UTF-16 units remain bytes throughout; no string decoding, normalization or scalar
publication occurs. Empty-haystack and empty-needle cases retain the existing
start-at-one behavior. A span overlaps the previous span by needle length minus
one, so boundary matches are neither omitted nor reordered. The 32-byte needle
cap bounds overlap; long-needle fallback can remain quadratic. This is not a
universally linear replacement.

The bounded experiment checks cancellation and work before each span. Its first
candidate retains exact existing scalar admission, but later early matches can
require more prepaid work. A two-byte-prefix control finds a one-byte needle at
zero-based offset 1: scalar uses 5 units, while bounded prepays 4100. With a
five-unit allowance, scalar succeeds and bounded refuses. Tests retain this
incompatibility explicitly; a timing improvement would not justify silently
changing production query budgets. The whole-buffer reference offers no solution
to that contract mismatch.

The direct memchr consumer is development-only. Upstream
[memmem::find](https://docs.rs/memchr/2.8.3/memchr/memmem/fn.find.html) supplies raw
substring search; TableCraft owns span bounds and work/cancellation policy.
No production expression, query, native storage or error behavior is changed.

## Correctness checks

```sh
cargo test -p tablecraft-expressions --example byte_search --locked --offline
```

Six tests cover independent slice positions, comparison with the production
kernel/counts, block edges, needle lengths 1/2/3/16/31/32/33/512, arbitrary raw
bytes, early-prefix admission, the later precharge incompatibility, exact scalar
fallback, every observed bounded cancellation checkpoint and byte-work refusals.
The seeded property test checks 256 generated arbitrary byte inputs against a
separate slice-position oracle. These are algorithm/resource checks, not native
Access qualification.

## Measurement recipe

Build once with normal incremental compilation, then keep the binary fixed while
running contenders:

```sh
cargo build -p tablecraft-expressions --example byte_search --release --locked --offline
hyperfine --warmup 3 --runs 15 --parameter-list algorithm scalar,bounded,reference \
  --export-json byte-search-absent.json \
  'target/release/examples/byte_search --algorithm {algorithm} --scenario absent --bytes 65536 --needle-bytes 32 --iterations 100'
```

Use fresh result paths. Retain all samples and report nanoseconds/milliseconds
with absolute units. Also measure front, late, random and raw scenarios, one-byte
needles and lengths 31/32/33 to expose the cap boundary and scalar fallback.
Each example invocation performs one reported probe followed by the requested
iterations. Output fields are algorithm, scenario, haystack bytes, needle bytes,
iterations, first offset, probe work units, probe checkpoints and loop checksum.
Setup and one probe are included in process timings for every contender; sufficient
iterations are required to avoid presenting process startup as kernel cost.

No timing results or production adoption are established by this recipe alone.
Any adoption needs an explicit decision about the incompatible work policy,
continued cancellation checks and an end-to-end InStrB measurement after a real
integration. Library microbenchmarks do not establish query/engine speedups.

The original capture runner checks all 21 contender/scenario combinations against
Python's raw-byte `find`, records exact source/input/binary hashes and probe work
counts, then captures the seven-case matrix with all hyperfine samples:

```sh
python3 docs/testing/benchmark_byte_search.py --check-only
python3 docs/testing/benchmark_byte_search.py \
  --output docs/testing/results/byte-search-EXPERIMENT-DATE.json
```

Keep Cargo and other timing workloads stopped throughout capture. Original raw
captures are retained under ignored `.cache`; public results are byte-identical
copies after checking private path markers and stable source/binary hashes.
The sidecar records host OS/architecture and tool versions without machine names
or absolute paths. The runner is a recipe until actual results are recorded.

## Recorded comparison, 24 October 2026

**Decision: keep this prototype out of production.** The bounded contender improves
long scans substantially in this run, but its prepaid units reject some queries
that the existing callback budget admits. First-position admission is identical;
that does not resolve the second-position counterexample. No end-to-end InStrB
speedup is claimed because no production integration was made.

Six Rust tests and scoped Clippy pass. The capture runner passes 21 independent
Python byte-position probes. The fixed release binary, input bytes and source
hashes are recorded in the [metadata](results/byte-search-experiment-2026-10-24.metadata.json);
[all 315 timed samples](results/byte-search-experiment-2026-10-24.json) are retained.
The host was Darwin arm64 with rustc 1.99.0 and hyperfine 2.0.0. Each contender had
three warmups and fifteen measured runs. Cargo and the other coordinated timing
workloads were stopped during this capture. Binary/source hashes were unchanged
before/after, and public artifacts passed the private-path scan.

The table reports **whole-process milliseconds, mean ± sample standard deviation**.
Each process includes input setup, one probe and the stated number of searches.
These are not per-search nanoseconds; fast cases include material process overhead.
All haystacks are 65,536 bytes.

| Scenario | Needle bytes | Searches/process | Scalar ms | Bounded ms | Whole reference ms |
| --- | --- | --- | --- | --- | --- |
| Front match | 32 | 1,000,000 | 19.09 ± 0.45 | 25.03 ± 9.79 | 87.54 ± 1.08 |
| Absent | 1 | 1,000 | 73.20 ± 17.65 | 2.36 ± 0.18 | 2.11 ± 0.17 |
| Absent, repeated prefix | 32 | 100 | 145.56 ± 73.33 | 1.82 ± 0.11 | 1.65 ± 0.22 |
| Late, repeated prefix | 32 | 100 | 130.59 ± 31.95 | 1.85 ± 0.15 | 7.51 ± 14.24 |
| Seeded random, midpoint needle | 32 | 1,000 | 47.32 ± 16.02 | 3.79 ± 0.22 | 2.94 ± 0.12 |
| Raw surrogate/odd-byte pattern | 3 | 1,000 | 37.91 ± 1.58 | 3.49 ± 1.77 | 7.18 ± 9.98 |
| Absent, scalar fallback | 33 | 100 | 123.53 ± 31.12 | 109.31 ± 0.96 | 1.83 ± 0.14 |

Several distributions are noisy: absent-32 scalar reaches 365.64 ms, front bounded
60.14 ms, late reference 42.65 ms and raw reference 36.65 ms. Those samples remain
included; no rerun or exclusion was used to improve the result. The 33-byte bounded
case calls the same scalar routine and charges the same 2,227,137 units; its lower
observed mean does not establish an algorithmic improvement. Front-match overhead
also deserves attention before any later adoption.

Resource units differ deliberately. The absent-32 probe charges 2,161,666 units
and callbacks in scalar mode, versus 66,609 units across 51 checkpoints in bounded
mode. The whole reference charges only its entry unit and cannot substitute for
metered production behavior. Results show an optimization opportunity, not a
completed search-policy design or an Access compatibility result.
