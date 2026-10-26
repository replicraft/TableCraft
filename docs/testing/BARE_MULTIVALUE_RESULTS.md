# Native bare multivalue results

Explicit collection-aware retrieval returns one parent row with a typed list for
each selected bare scalar multivalue field. Ordered children, duplicates, NULL
members and empty lists remain distinct. Hidden `.Value` criteria select parents
without shortening the selected list; explicit displayed `.Value` still expands
rows. Row-source lookup labels and imported expressions never run.

The CLI cell format is bracketed comma-and-space-separated existing scalar display
inside the ordinary CSV-quoted TSV transport. It is neither JSON nor a native
Access collection representation. Text retains quotes/escapes. `[]` and `[NULL]`
are different cells. Finite IEEE display round trips to the stored width including
negative zero; NaN text cannot identify its payload or sign. Separate typed native
API tests retain exact Single bits, including two signed NaN payloads.

## Original inputs

The unchanged repository-owned [MultivalueFixture.java](MultivalueFixture.java) recipe
and read-only [MultivalueOracle.java](MultivalueOracle.java) were run with reviewed Jackcess 5.0.4, JAR SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Fresh outputs were created under
`.cache/tablecraft-fixtures/bare-multivalue-results-20261025/` after the historical
registered originals could not be located among 453 current local native inputs.
Old records were preserved; new bytes have their own identities.

| Profile | SHA-256 |
| --- | --- |
| V2007 | `583d4b99abf504cd8b1bae72423cffc02390bdbd2e46fb8ba1667abf7aaab0da` |
| V2016 | `ef784999fcb08b6e50a875e05ba702259d900a2ff9d1c79f28a64dec64457c58` |
| V2019 | `3b53417c687f0e4096f5a584bc134b74ece7ae2569fcd847b520231f2e45e295` |

Each profile has eight element types, four parent rows, 32 lists and 137 stored
members. Samples include signed extrema, exact large Decimal/GUID values, raw IEEE
bits, Unicode/control Text, duplicates, one-NULL lists, empty parents and a
100-item list. The separate 101-item table retains the typed-reader ceiling.
Expression evaluation is explicitly disabled. No reference SQL or imported active
content executes. The oracle uses public collection reads with an explicitly
identified pinned implementation metadata getter for the element type.

Exact producer/getter commands and per-call stdout/stderr remain next to the
originals. All 24 getter calls complete without changing original bytes. A new
regeneration is not promised byte-identical. No fixture binaries are tracked.

## Focused native verification

All 50 focused checks pass, including eight new native tests plus 35 existing
multivalue/collection/linked/external regressions and seven recomputed-view
consumer checks. Strict scoped Clippy and formatting also pass. The existing
scalar-only recomputed metadata helper explicitly rejects unexpected collection
metadata after the OutputType extension. The [native conformance record](results/bare-multivalue-native-conformance-2026-10-25.json)
preserves the first authored failures: an execution quota test initially exhausted
catalog inventory, and a scalar-multivalue `IS NULL` assumption confused it with
the existing named attachment/history presence contract. Neither required a
production change; bare scalar-list operations remain refused.

## Accepted original-data replay

The [first accepted report](results/bare-multivalue-results-2026-10-25.json) has
48 controls and zero disagreements: 27 successful commands compare 87 rows and
258 output cells; 21 expected refusals require empty stdout and a matching error
class. Three profiles each exercise the complete eight-carrier matrix, aliases,
static derived projection, scalar ordering/TOP, hidden `.Value` predicates,
scalar-ID-selected empty/NULL-member parents, and existing scalar `.Value` output.
Refusals preserve unsupported collection DISTINCT/order/mixed `.Value`/UNION,
the per-parent member ceiling, scalar nonfinite publication and output limits.

The stdlib Python model reads captured public getter records. Integer membership
predicates independently select parent IDs; returned lists remain the original
complete getter lists. Expected Text/GUID/Decimal/integer spellings reuse the
pinned repository scalar display helper. Finite float text parses back to the
getter IEEE width, including negative zero; NaN checks only its documented class.
The CLI never serves as its own value oracle. List delimiters, per-member spelling
and outer CSV-quoted TSV transport are checked separately.

The frozen executable is
`.cache/tablecraft-bin/bare-multivalue-cli-20261025`, SHA-256
`07224f3fc964afd9c500c3141d592625205504ff664e9b6716bbec522d9b90fe`,
built from committed revision `ff7d63c12184957e0e22ab711cf7b8eb7a051190` with
clean production Rust/manifests. The host build record is
`.cache/bare-multivalue-cli-build-20261025.json`. The replay pins 36 artifacts
before/after and forty selected immutable git objects from that compiled revision;
it does not depend on a moving live-source inventory. Every command and output
is retained in `.cache/bare-multivalue-replay-20261025.jsonl`. No failed original
CLI replay was discarded or replaced.

```sh
python3 docs/testing/compare_bare_multivalue_results.py \
  --cli .cache/tablecraft-bin/bare-multivalue-cli-20261025 \
  --cli-sha256 07224f3fc964afd9c500c3141d592625205504ff664e9b6716bbec522d9b90fe \
  --compiled-revision ff7d63c12184957e0e22ab711cf7b8eb7a051190 \
  --originals .cache/tablecraft-fixtures/bare-multivalue-results-20261025 \
  --report docs/testing/results/bare-multivalue-results-2026-10-25.json \
  --log .cache/bare-multivalue-replay-20261025.jsonl
```

The runner requires fresh report/log paths. This evidence checks TableCraft's
explicit typed output and presentation contract against independently retrieved
stored values. It does not establish Access SQL execution, lookup display-label
semantics, mutation, application save/reopen, or native Microsoft Access collection
result representation.
