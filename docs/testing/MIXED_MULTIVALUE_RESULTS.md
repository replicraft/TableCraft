# Mixed complete-list and displayed-member replay

This replay compares whole stored lists beside scalar displayed children with
an independent parent/child model. It reuses the unchanged V2007/V2016/V2019
originals and 24 getter transcripts registered by the
[bare-list replay](BARE_MULTIVALUE_RESULTS.md). No fixture replacement, lookup
execution or database mutation is involved.

The eighteen cases run under ordinary and declared arithmetic:

- Same-field expansion and filtering, duplicate Text values, NULL children and
  present empty lists, always retaining the complete selected list.
- Independent displayed collections and hidden parent membership, plus an
  independent displayed product.
- Static derived projections, scalar sorting/TOP, LEFT/RIGHT joins and a nested
  join, including explicit absent-parent sidecars.
- Raw IEEE list data with an independent finite displayed member, contrasted
  with refusal when the nonfinite member itself enters scalar expansion.
- Eager filter errors and remaining DISTINCT, UNION and whole-list sort refusals.

There are thirteen successful cases per profile/mode, containing 168 rows and
534 cells. The [committed-build replay](results/mixed-multivalue-originals-2026-10-26.json)
matches all 108 controls with zero disagreements: 78 successful results containing
1,008 rows and 3,204 cells, plus 30 actual errors/refusals. Its frozen debug CLI
was built at `923fc290`; all original/getter, executable and reference pins stayed
unchanged. The earlier frozen outer-join CLI refused the first mixed query with
exit status 2 and empty stdout. Preliminary uncommitted-build results remain
private and are not counted again.

```sh
python3 docs/testing/compare_mixed_multivalue_results.py \
  --cli FROZEN_EXECUTABLE --cli-sha256 SHA256 \
  --compiled-revision REVISION --originals ORIGINAL_FIXTURE_DIRECTORY \
  --report FRESH_REPORT.json --log FRESH_LOG.jsonl
```

The working [shared replay driver](collection_query_replay.py) checks original
and getter hashes, pins the executable and reference helpers before and after,
records exact commands/results, validates complete output and requires exit 2
with empty stdout for expected failures. Cases explicitly select the ordinal
comparison policy; they do not qualify native database collation. Selected
committed source snapshots are not a complete compiler-input manifest.

Microsoft's [query-criteria guide](https://support.microsoft.com/en-us/access/examples-of-query-criteria)
places the whole field and its single-value member in separate grid columns,
with hiding the member optional. This supports the capability, without printing
the exact generated SQL or qualifying TableCraft's duplicate order, empty-parent
placeholder and outer-absence contracts. Getter values establish stored data,
not native Access SQL results. Existing NaN display limits remain; typed API
tests separately retain exact stored representations.
