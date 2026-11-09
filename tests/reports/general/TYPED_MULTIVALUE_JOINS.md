# Typed multivalue join replay

The join replay compares explicit complete-list INNER/CROSS query output with
an independently assembled parent-pair model. It reuses the three unchanged
V2007/V2016/V2019 databases and 24 getter transcripts registered by the
[bare-list replay](BARE_MULTIVALUE_RESULTS.md). It does not generate replacement
fixtures or execute database code.

Each profile runs twelve cases under both ordinary and declared arithmetic:

- Equality and inequality INNER joins, filtered CROSS joins and swapped lists.
- Nested right joins, scalar derived leaves and typed derived leaves.
- Hidden member criteria that select parent pairs while retaining whole lists.
- Explicit refusals for outer joins, displayed-member/list combinations and list
  comparisons, plus an eager ON division error before a false WHERE filter.

The eight successful cases per profile/mode contain 33 rows and 130 cells.
Across all profiles and modes, the expected comparison is 72 controls, including
48 successful results containing 198 rows and 780 cells, and 24 errors/refusals.
The [committed-build replay](results/typed-multivalue-joins-originals-2026-10-26.json)
matches all 72 controls with zero disagreements: 198 successful result rows and
780 cells, plus 24 actual errors/refusals. The frozen debug executable was built
at `0f0fe5ac`; all registered database/getter and executable hashes stayed
unchanged. The earlier frozen CLI refused the first original INNER query with
exit status 2 and empty stdout. Preliminary uncommitted-build observations remain
private and are not counted again.

Run the replay with a frozen executable, its SHA-256 and the committed source
revision used to build it:

```sh
python3 docs/testing/compare_typed_multivalue_joins.py \
  --cli FROZEN_EXECUTABLE --cli-sha256 SHA256 \
  --compiled-revision REVISION --originals ORIGINAL_FIXTURE_DIRECTORY \
  --report FRESH_REPORT.json --log FRESH_LOG.jsonl
```

The runner verifies registered database/getter hashes, records exact commands and
outputs, requires an empty stdout and exit status 2 for errors, and pins inputs,
reference helpers and the executable before and after execution. It records
selected committed source objects, including all joined-list implementation
modules. These selected objects are not a complete compiler-input manifest.

The getter supplies stored values rather than native Access SQL results. This
replay therefore checks TableCraft's explicit parent-pair/list transport and
display contracts. Native Microsoft Access query cardinality, ordering, lookup
labels and display equivalence remain separate qualification work. Existing
NaN text-display limits remain: typed API checks establish stored IEEE bit
preservation, while a displayed NaN cannot identify its payload or sign.
