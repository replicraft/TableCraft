# Original native/CAB stateful streaming replay

The [96-control report](results/progressive-stateful-stages-2026-10-25.json)
checks static nested aggregate stages against independent Python integer/Fraction
models over the six unchanged [large deployment inputs](LARGE_DEPLOYMENT.md).
Only TableCraft executes SQL. Inert getters establish source rows and separate
CAB checks establish packaged bytes; neither is a native Access SQL oracle.

All 78 successful commands match, comparing 96 result rows and 480 cells. Eighteen
intended failures retain the expected diagnostics and publication boundaries;
six of those retain exactly one earlier row, six cells in total. These prefix
rows are separate from successful results. All 96 controls matched on the first
complete replay; SQL, models and resource settings were not revised afterward.

Coverage includes inner global Count/Sum/Avg/Min/Max, exact and Text grouping,
outer regrouping/aggregation, HAVING suppression, intermediate filters and TOP,
three stateful barriers, empty-global versus empty-keyed populations, raw String
and Error observers, Currency/Decimal carrier observers, late inner errors and
work exhaustion. A late inner final-expression failure can retain completed
outer row batches but cannot finalize an outer Count result. The native sources
and their stored/MSZIP CAB copies exercise the same query models.

The frozen CLI was built from committed revision
`57140e145eaef59b3de6f8b84eca6852a8526ea8`, with SHA256
`93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9`.
Fifteen immutable inputs/tools and fifty selected committed source files were
independently rehashed after the run, including the new stage driver. The selected
source hashes are evidence pointers, not a complete executable compilation manifest.
All six original database/package hashes remain unchanged.

Reproduce using the registered original inputs and fresh report path:

```sh
python3 docs/testing/compare_progressive_stateful_stages.py \
  --cli .cache/tablecraft-bin/progressive-stateful-cli-20261025 \
  --cli-sha256 93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9 \
  --revision 57140e145eaef59b3de6f8b84eca6852a8526ea8 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --report FRESH_REPORT.json
```

The runner caps commands before execution and refuses to overwrite a report.
Native reads explicitly allow 256 MiB and ten million steps for these 77 MiB
files. Query defaults remain except the deliberate one-step refusal. Ordinal
comparison and invariant numeric formatting are explicit. This does not qualify
native Access behavior, other streaming shapes or arbitrary-size performance.
