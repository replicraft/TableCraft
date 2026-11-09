# Attachment decoder session evidence

Measured 23 October 2026 on macOS ARM64, Rust 1.99.0, release profile without
native-CPU or LTO overrides. This is bounded decoder evidence, not native Access
or general engine throughput qualification.

## Correctness and allocation behavior

The ACE 16/ACE 2007 [larger stored-object corpus](FIXTURES.md#larger-attachment-and-stored-history-inputs)
now checks every encoded metadata field, decoded byte and stored history DATE
bit for 20,000 attachments and 20,000 versions per input. Thirty-three independent
controls pass. Four new native tests check shared workspace accounting, corruption/
truncation/trailing-stream resets, lazy allocation and every observed second-stream
cancellation checkpoint with explicit subsequent recovery. A 192 KiB logical pool
admits 512 independent compressed files through one session; recreating codecs
exhausts the same pool. This measures cumulative allowances, not physical peak RAM.

A session borrows the original budget/cancellation policy, charges its selected
Rust inflater workspace once, and resets before every independent compressed
stream. Per-file body/type/copy allocations and work still consume the pool.
Checksums, exact lengths, sentinel, wrapper validation and file ceilings remain.
The free single-file API retains its fresh-workspace behavior. The Access adapter
uses one session per complete decoded-column cache, never publishing partial data.

## Timings

The [raw report](results/attachment-sessions-2026-10-23.json) records ten process
runs and two warmups per contender, full samples, source/binary hashes and locked
dependency versions. Both modes use matched cumulative pools. Each iteration
validates a complete wrapper and destroys its decoded output; reused mode also
drops the session before its inner timer ends. Hyperfine process timings include
input creation/setup. Three-byte cases use 20,000 iterations; 60,000-byte cases
use 1,000. All payloads are authored repeated or deterministic-noise bytes.

| Payload / storage | Fresh, µs/file mean ± SD | Reused, µs/file mean ± SD | Interpretation |
| --- | ---: | ---: | --- |
| 3 bytes / stored repeated | 0.191 ± 0.041 | 0.187 ± 0.030 | Unresolved within variation |
| 3 bytes / zlib repeated | 3.699 ± 0.167 | 2.582 ± 0.061 | 1.43× in this case |
| 3 bytes / zlib noise | 3.513 ± 0.025 | 2.526 ± 0.050 | 1.39× in this case |
| 60,000 bytes / stored repeated | 2.566 ± 0.175 | 2.341 ± 0.118 | Observed difference; no codec is used |
| 60,000 bytes / zlib repeated | 24.668 ± 0.429 | 22.538 ± 0.369 | 1.09× in this case |
| 60,000 bytes / zlib noise | 154.970 ± 6.864 | 151.268 ± 0.567 | Unresolved within variation |

The original [overlapping run](results/attachment-sessions-excluded-2026-10-23.json)
ran alongside Cargo/Clippy and reference/native comparisons. Its complete raw
results are preserved and explicitly excluded. The accepted pass ran after those
agent jobs completed. Stored/noise controls and variation remain visible; no
universal speedup or allocator/RSS conclusion follows from these measurements.

```sh
cargo bench -p tablecraft-ace --bench attachments --no-run --locked --offline
python3 docs/testing/benchmark_attachment_sessions.py \
  --binary target/release/deps/attachments-BUILD_HASH \
  --output /tmp/attachment-sessions.json
```

Use Cargo's printed benchmark path for BUILD_HASH. Build and finish correctness
jobs before timing. The older single-mode runner now describes the matched
cumulative pool; its historical reports retain their original source hashes and
measurement conditions.
