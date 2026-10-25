# Native integral join CLI benchmark

The 25 October 2026 local benchmark measures complete debug CLI commands on
the two original 1,100-row ACE 16-family databases. Results are exact before
and after timing. These sparse unique-key joins improve from roughly
0.87–1.01 seconds to 15 milliseconds; this is not a release-build, isolated
kernel or general workload claim.

| Profile | Before mean ± SD | After mean ± SD | Mean ratio |
| --- | ---: | ---: | ---: |
| V2016 | 1010.2 ± 102.5 ms | 15.41 ± 0.57 ms | 65.6× |
| V2019 | 867.0 ± 16.8 ms | 15.30 ± 0.61 ms | 56.7× |

All seven measured runs per case are retained: 28 successful measurements,
with two warmups per case. No samples, slower runs or outliers were excluded.
The V2016 baseline has visibly greater spread; the raw data preserves it.
This warm local developer-machine result does not establish cold-cache,
concurrent-client or arbitrary-size performance.

## Exact scope

Both commands select `L.Id,R.Score` from the same raw native table self-joined
on `L.Id=R.Id`. Each expected row is `(id, id * 17 - 9)` for IDs 1 through
1,100, in original encounter order. The repository-owned producer and reopened
getter/model evidence are in [the original corpus](LARGE_DEPLOYMENT.md).

Timing uses identical explicit query allowances: 256 MiB of query bytes,
10,000 query rows and 100,000,000 query steps. The higher work allowance lets
the old algorithm finish with the same exact result. The separate
[76-command replay](DIRECTORY_JOINS.md) proves old default-step refusals and
new default-step success; that distinction is not hidden in the timing flags.

The before executable is frozen from `ef6d7f50acf9d6068c7ea14bc26f8270a2d8c446`;
the after executable is frozen from `35b6ce8dca59590d316e62bf59ec9511a16d14ca`.
Both were built with the ordinary development Cargo profile. The source inputs
and binaries have exact hashes in the [qualified summary](results/directory-joins-benchmark-2026-10-25.json).
The environment is Darwin arm64 with Hyperfine 2.0.0. All native images and
binaries retained their hashes after measurement.

## Correctness and raw evidence

The old high-work commands matched the independently constructed 1,100-row
Id/Score TSV model before timing. Candidate commands matched before timing;
all four commands matched after timing. The expected output hash is recorded
in the summary. Hashes and complete commands identify exactly what was measured.

[Raw Hyperfine schema-2 data](results/directory-joins-hyperfine-2026-10-25.json)
retains every observation, exit code, command, summary and additional measured
metric. Its SHA-256 is `d2b5686a5589db811453a77255e501f8a1265072928d88f667b3e649de27e7d9`.
No memory improvement claim is derived from the incidental RSS observations.

The initial summary printer assumed Hyperfine 1 top-level mean fields and
failed after successful measurement and pin checks. Reading the actual
schema-2 summaries corrected reporting; no command measurement was rerun,
discarded or rewritten. The retained raw log is
`.cache/directory-join-hyperfine-20261025.log`.

To reproduce, use the exact four commands in the raw report with
`hyperfine --warmup 2 --runs 7 --export-json FRESH_REPORT.json`, adding a
`--command-name` for each case. First independently validate each command’s
complete output and pin the two source images and frozen binaries. Do not
substitute the mutable `target/debug/tablecraft` executable.
