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
