# Adaptive encryption copy measurements

Measured 8 November 2026 on macOS ARM64 with release CLI binaries and Hyperfine
2.0.0. These measurements cover the complete CLI operation: credential checks,
native preflight and SID handling, page transformation or envelope-only copying,
output synchronization and fresh no-clobber publication. They are not isolated
cipher throughput, native Microsoft Access timings or native Access qualification.

[The complete sanitized report](../../results/encryption/adaptive-encryption-2026-11-08.json)
retains all measured samples, wall/user/system/total CPU time, peak resident
memory, input and executable hashes, and full decoded-output verification.
Repository-relative paths identify local evidence without distributing inputs.
The prior serial binary's exact source commit was not frozen; its recorded
SHA-256 identifies the actual comparator. The initial adaptive binary was built
from the working state later committed by `46e193c3`, without a reproducibly
pinned build revision. The final production change is `16c0ff39`; its binary hash
is recorded separately.

## Workloads and controls

The 1, 8 and 64 MiB ACCDB inputs contain deterministic unallocated padding and
real Agile encryption. The 128 MiB input extends the 64 MiB plaintext with
zero-filled unallocated tail pages and is encrypted outside the timed runs.
These are performance-only workloads, not Access-authored application fixtures.
The separate 24.265625 MiB Jet 4 input is the retained encoded-values reference
fixture; the benchmark does not establish its authorship or native Access
compatibility. Its encrypted counterpart was prepared with the baseline CLI
outside timing. No fixture bytes are added to this record.

The initial exploratory cohort contains 60 workloads: five inputs, three
operations and four modes, each with one warmup and five measured runs. New
modes use a 2 MiB batch cap with one worker, four workers or automatic workers.
The final cohort contains 36 workloads: the 64/128 MiB and Jet 4 inputs, three
operations and four modes, each with two warmups and seven measured runs. It
compares the same old serial binary against four workers with a 2 MiB batch cap,
automatic workers with that cap, and automatic workers with automatic memory.
The process CPU count and platform are recorded with the final cohort. No
Cargo builds, tests or rendering ran concurrently. During the final cohort, a
separate packaging metadata audit traversed Cargo dependency metadata, six
platform dependency trees and license files; one generation took approximately
30–40 seconds, followed by a second generation. This filesystem/metadata work
overlapped measurements, so the final cohort is not an idle-machine experiment.
Background OS work, caches and thermals were also uncontrolled. Modes run
sequentially in the recorded order;
these are descriptive repeated observations, not randomized causal estimates.
A post-run disk check found 8.4 GiB available on the 926 GiB volume (reported
as 100% used), compared with an earlier 61 GiB check before the large build
gates. Disk availability was not sampled during benchmark invocations; this
post-run observation is an additional environmental limitation, not evidence
of what caused any individual timing.
Output removal occurs in Hyperfine's untimed preparation; each measured CLI
still creates, synchronizes and publishes its own complete fresh output.

## Final observations

Wall times vary substantially, including large changes in the unchanged baseline
between cohorts. The ratios below describe these observations and are not
reliable estimates of isolated crypto gains or expected application speedups.
CPU and RSS measurements are descriptive process observations under the same
limitations. Complete decoded-byte verification is the stronger correctness
result. All final new modes have lower observed mean CPU time than the
comparator. The explicit 2 MiB batch modes use about 8–9 MiB process peak RSS,
while automatic memory reaches about 135 MiB on the 128 MiB input. These are
measurements of this host and workload, not enforced RSS bounds.

Wall time in milliseconds; parentheses show the ratio of old/new means.

| Input and operation | Old serial | Four workers, 2 MiB | Auto workers, 2 MiB | Auto workers/memory |
| --- | ---: | ---: | ---: | ---: |
| Agile 64 MiB, remove | 1935 ± 727 | 351 ± 97 (5.52×) | 431 ± 179 (4.49×) | 443 ± 432 (4.36×) |
| Agile 64 MiB, add | 2393 ± 978 | 569 ± 140 (4.20×) | 526 ± 153 (4.55×) | 359 ± 170 (6.67×) |
| Agile 64 MiB, change-password | 1049 ± 417 | 211 ± 59 (4.96×) | 467 ± 196 (2.25×) | 501 ± 117 (2.09×) |
| Agile 128 MiB, remove | 1935 ± 511 | 788 ± 118 (2.46×) | 563 ± 104 (3.44×) | 647 ± 426 (2.99×) |
| Agile 128 MiB, add | 9408 ± 7287 | 2837 ± 1381 (3.32×) | 3403 ± 1540 (2.76×) | 1223 ± 663 (7.69×) |
| Agile 128 MiB, change-password | 1545 ± 836 | 591 ± 335 (2.61×) | 370 ± 280 (4.18×) | 238 ± 39 (6.49×) |
| Jet 4, 24.27 MiB, remove | 649 ± 290 | 131 ± 53 (4.95×) | 163 ± 44 (3.98×) | 126 ± 65 (5.15×) |
| Jet 4, 24.27 MiB, add | 451 ± 247 | 170 ± 50 (2.65×) | 178 ± 60 (2.53×) | 70 ± 22 (6.42×) |
| Jet 4, 24.27 MiB, change-password | 427 ± 331 | 207 ± 96 (2.06×) | 169 ± 39 (2.52×) | 98 ± 17 (4.36×) |

Times are mean ± sample standard deviation. Ratios compare the old serial mean
to the corresponding new mean for the same input and operation. CPU time sums
user and system time across threads; it may exceed wall time. Peak RSS is a
process measurement, separately retained for every run.

Mean CPU time (ms) / mean peak RSS (MiB), separately observed:

| Input and operation | Old serial | Four workers, 2 MiB | Auto workers, 2 MiB | Auto workers/memory |
| --- | ---: | ---: | ---: | ---: |
| Agile 64 MiB, remove | 624 / 4.38 | 149 / 8.26 | 158 / 8.51 | 184 / 70.84 |
| Agile 64 MiB, add | 716 / 4.19 | 263 / 8.23 | 276 / 8.46 | 269 / 70.90 |
| Agile 64 MiB, change-password | 515 / 3.93 | 99 / 8.05 | 109 / 8.25 | 142 / 70.50 |
| Agile 128 MiB, remove | 1074 / 4.31 | 248 / 8.33 | 300 / 8.50 | 326 / 135.33 |
| Agile 128 MiB, add | 1726 / 4.29 | 781 / 8.29 | 971 / 8.42 | 501 / 135.34 |
| Agile 128 MiB, change-password | 656 / 3.94 | 117 / 8.07 | 113 / 8.28 | 170 / 134.94 |
| Jet 4, 24.27 MiB, remove | 200 / 4.03 | 72 / 8.14 | 80 / 8.39 | 83 / 30.79 |
| Jet 4, 24.27 MiB, add | 185 / 4.06 | 71 / 8.24 | 72 / 8.30 | 76 / 30.77 |
| Jet 4, 24.27 MiB, change-password | 209 / 4.07 | 137 / 8.17 | 139 / 8.31 | 123 / 30.80 |

The batch cap accounts for owned page/status capacities, headers and active
codec scratch. Thread stacks, allocator overhead and unrelated process memory
are outside it, so a 2 MiB batch cap is not a 2 MiB RSS promise. The native
streaming source ceiling remains **2 GiB**, separate from this working-set cap,
the **64 MiB image API** limit and cumulative work quotas. The 128 MiB workload
exercises streaming beyond the image API limit; it does not test the 2 GiB
boundary. Automatic memory can retain more than the explicit 2 MiB cap.

## Verification and retained failures

For every measured mode, a fresh untimed execution checks the complete output.
Removal hashes its plaintext directly; add/change-password outputs are removed
with the retained baseline executable before their full plaintext hashes are
compared with the corresponding sequential result. Agile password changes also
retain all post-header ciphertext bytes. Input hashes match before and after
both cohorts; final checks also confirm both executable hashes remain unchanged.
These checks compare implementations using a shared decoder, so they supplement
independent raw-byte tests rather than establish independent native compatibility.

The initial harness incorrectly required unchanged Jet 4 ciphertext on a
password change. Native SID remasking legitimately changes those bytes. The
assertion stopped verification after all timing samples had been written. The
existing baseline output/decoded file and fresh remaining outputs were then
checked against the complete plaintext digest without repeating or discarding
any timings. The report retains the correction and all 60 verified workloads.
The final cohort uses the corrected cipher-specific assertion.

The initial exploratory samples are retained in full, including high-variance
and slower observations. They measure the earlier envelope-copy implementation
and must not be pooled with the final source state. No sample or operation was
excluded because its timing was unfavorable. The earlier captured-image
[page-decoding experiment](../general/NATIVE_LIBRARY_REFACTORS.md#optional-parallel-page-decoding)
is a different workload and remains separate evidence.

## Reproduction and limits

Build the native CLI in its ordinary release profile with the locked toolchain;
retain a separately identified previous serial executable for the comparator.
Prepare inputs outside timing. For each mode and operation, use a fresh output:

```sh
tablecraft encryption remove encrypted.accdb --output plain-copy.accdb -p PASSWORD
tablecraft encryption add plain.accdb --output encrypted-copy.accdb --new-password PASSWORD
tablecraft encryption change-password encrypted.accdb --output changed.accdb \
  -p PASSWORD --new-password REPLACEMENT
```

Append `--jobs 4 --memory-budget-mib 2`, `--memory-budget-mib 2`, or no flags
for the final new modes. Use `hyperfine --warmup 2 --runs 7 --export-json REPORT`
with an untimed prepare that removes only the named benchmark outputs. Verify
complete decoded bytes and unchanged input hashes separately. Actual sources,
paths, executable hashes, commands and all observations are retained in the
sanitized report; ignored local runners are not a portable fixture package.

Only Agile and the retained Jet 4 route are timed here. Other implemented
ciphers, constrained machines, Windows/Linux execution, cold-cache behavior,
concurrent access and native Access saves need their own evidence. No native
app window was launched. See the
[encryption contract](../../../docs/access/storage/NATIVE_ENCRYPTION.md#adaptive-parallel-streaming)
for current eligibility, failure/publication behavior and integrity limitations.
