# Original full-check projection replay

The [64-command report](results/native-check-projection-replay-2026-10-25.json)
compares frozen full-table consistency checks over the same sixteen original
[deletion-history inputs](DELETE_STORAGE_OBSERVATIONS.md). It generates no
replacement files and performs no writes. The registered independent getter/byte
report remains the source population and provenance reference.

All commands match their expected outcomes. For each input, the old and new
implementations produce byte-identical completed TSV reports under an explicit
ten-million-step allowance. The optimized implementation also produces that same
complete report under the unchanged one-million-step default in all sixteen cases.
The old implementation exhausts default work in twelve nonempty states; its four
empty-table states already complete. Those twelve failures remain expected controls,
not failed correctness comparisons. The total is 52 successful reports and twelve
expected default-work refusals.

This proves a concrete default-budget improvement on these long-payload/index
histories while retaining every reported checked/unsupported status and row count.
The known ignore-NULL profile stays explicitly unchecked. It does not qualify
native Access integrity results, arbitrary table sizes or elapsed-time performance.
The [focused engine tests](results/native-check-projection-2026-10-25.json) separately
prove payload-page visit reduction and corrupt unselected payload rejection.

The old CLI is SHA256
`1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0`
from committed revision `32b5d157fea3512cd9f624bdff5a3f3a4b3d2e2b`.
The new CLI is SHA256
`93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9`
from committed revision `57140e145eaef59b3de6f8b84eca6852a8526ea8`, which includes
engine optimization `c24eb11a`. Twenty input/tool/report hashes were checked before
and after execution and independently rehashed afterward. Every original is unchanged.

Reproduce with the frozen executables and a fresh report path:

```sh
python3 docs/testing/compare_native_check_projection.py \
  --baseline .cache/tablecraft-bin/inspect-native-work-cli-20261025 \
  --current .cache/tablecraft-bin/progressive-stateful-cli-20261025 \
  --corpus-report docs/testing/results/delete-storage-final-2026-10-25.json \
  --report FRESH_REPORT.json
```

The runner validates exact registered hashes, uses bounded subprocess timeouts,
and refuses to overwrite an existing report. Full checks still require stable,
offline sources; unchanged-source evidence here does not establish live-file isolation.
