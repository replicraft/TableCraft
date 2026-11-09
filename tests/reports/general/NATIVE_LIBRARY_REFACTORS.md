# Native library refactor measurements

Measured 8 November 2026 on macOS ARM64 with Rust/Cargo 1.99.0 and Hyperfine
2.0.0. Release binaries use the normal release profile, without custom CPU/LTO
flags. Each command has three warmups and nine measured runs. No Cargo builds or
other agent work ran during the final cohort; background OS work and thermals
were not controlled. No native app windows were opened.

[The complete report](../../results/general/native-library-refactors-2026-11-08.json) retains
input/source/binary hashes, every final sample, observed peak RSS and earlier
exploratory samples. The earlier cohort whose ignored filename said `quiet`
actually overlapped compilation; it is retained as noisy evidence. Do not use
Hyperfine's cross-command summary ratios to compare different file sizes.

## Shared snapshot originals

Each process loads the original once, then performs **five** native captures and
catalog inspections, comparing every payload byte on every iteration. Shared
mode additionally verifies that the original pointer is retained. These process
wall times include startup, input loading, inspection, equality checks and drops;
they are not isolated allocation timings or per-review latency estimates.

| Source | Copying, mean ± SD | Shared, mean ± SD | Mean peak RSS, copying / shared |
| --- | --- | --- | --- |
| Public example, 3,387,392 bytes | 4.86 ± 0.47 ms | 3.04 ± 0.12 ms | 9.67 / 5.00 MiB |
| Synthetic 1 MiB | 2.12 ± 0.30 ms | 1.83 ± 0.16 ms | 3.88 / 2.64 MiB |
| Synthetic 8 MiB | 9.88 ± 0.69 ms | 4.64 ± 0.25 ms | 26.63 / 9.66 MiB |
| Synthetic 64 MiB | 60.30 ± 2.92 ms | 24.84 ± 1.39 ms | 129.78 / 65.72 MiB |

For five 64 MiB captures, the copying path requests 320 MiB of cumulative payload
copies; the shared path requests none. This is derived from the API path, not
allocator instrumentation. The benchmark reports the original retained capacity
and additional requested payload bytes separately: allocator capacity can exceed
a reservation request. Catalog/control allocations and the original read still
exist. Writable AFTER images and some internal candidate recaptures still copy.
Do not infer complete runtime memory usage from this isolated comparison.

The runtime now consumes a whole-owned `NativeImageSource` for immutable reviews.
Every review admits complete retained capacity and control storage under its own
budget. Arbitrary sliced `Bytes` cannot establish that witness. Native save and
recovery still re-read/requalify the real source; cached bytes grant no authority.
[The ownership contract](../../../docs/access/NATIVE_READING.md#shared-immutable-native-image-ownership)
retains these distinctions.

## Optional parallel page decoding

Each process unlocks once and constructs a four-thread pool, then performs
**five** prepaid clone/decode/ordered-status/zeroize iterations. Serial and Rayon
modes use the same workload, including setup. The process times below include
that setup, while the benchmark also prints separately timed inner iterations.
They do not measure complete encryption removal, native SID/header changes,
filesystem publication or general query execution.

| Encrypted input | Serial, mean ± SD | Four workers, mean ± SD | Mean peak RSS, serial / parallel |
| --- | --- | --- | --- |
| Synthetic 8 MiB | 88.10 ± 11.21 ms | 58.87 ± 1.35 ms | 49.67 / 50.59 MiB |
| Synthetic 64 MiB | 550.48 ± 15.02 ms | 316.25 ± 8.51 ms | 210.58 / 211.56 MiB |

The page transform benefits from parallelism; it does not demonstrate a process
memory reduction. Generated inputs contain deterministic unallocated padding and
real Agile encryption. They are crypto workloads, not qualified native database
corpus entries. Verification on 1, 8 and 64 MiB inputs compares complete decoded
bytes/hashes, deterministic ordered errors, cancellation and exact/one-below
worker/coordinator quotas. Production tests separately compare entire rewritten
serial/parallel candidates and check credentials, cancellation and fresh-output
refusal.

The measured consumer is the optional native `parallel-encryption` feature,
selected explicitly by CLI `encryption remove --jobs 2..16`. Default `--jobs 1`
retains serial streaming. Parallel removal captures at most 64 MiB and refuses
other operations/ciphers without fallback. Native validation/publication remain
serial. The default desktop dependency tree contains no Rayon; browser targets
do not inherit this feature's native dependency. See the
[parallel removal contract](../../../docs/access/storage/NATIVE_ENCRYPTION.md#explicit-parallel-agile-removal).

## Reproduction

Build the two real public-API benchmark executables:

```sh
cargo build -p tablecraft-ace --release --features ace16-create \
  --bench image_capture --bench encryption_pages --locked
```

Use the executable paths emitted under `target/release/deps/`, with their target
hash suffixes. `encryption_pages --generate FILE --mib 8` creates a fresh encrypted
workload using the documented public benchmark password. Use the CLI's serial
`encryption remove` into a fresh path for the corresponding plaintext snapshot
input. `--generate` and removal refuse existing destinations.

```sh
image_capture stream --database plain.accdb --iterations 5
image_capture shared --database plain.accdb --iterations 5
encryption_pages --input encrypted.accdb --mode verify --threads 4
encryption_pages --input encrypted.accdb --mode serial --threads 4 --iterations 5
encryption_pages --input encrypted.accdb --mode rayon --threads 4 --iterations 5
```

Run each timing pair with `hyperfine --warmup 3 --runs 9 --export-json REPORT`.
Use the same input and iteration count for both sides; keep compilation and other
work outside timing. Equality checks, resource limits and cancellation tests are
separate correctness evidence; timing does not establish native Access parity.
