# Typed outer-join list replay

The runner compares explicit LEFT/RIGHT parent/list results with an independent
model over the unchanged V2007/V2016/V2019 originals and their 24 registered
getter transcripts. Fixture provenance is retained by the
[bare-list replay](BARE_MULTIVALUE_RESULTS.md). No replacement databases are
generated, and no active database code runs.

Fourteen cases run under both ordinary and declared arithmetic for each profile:

- LEFT/RIGHT inequality matching, wholly unmatched parents and empty opposite
  inputs, preserving each join's explicit encounter order.
- A nested missing subtree and repeated absent outputs through a derived source.
- Hidden membership and NOT membership, retaining complete stored lists while
  distinguishing an absent parent from a present empty list.
- Refusals for mixed displayed members, list comparisons and bare scalar-list
  presence, plus an eager ON error before a false WHERE filter.

The ten successful cases contain 44 rows and 176 cells per profile/mode. The
complete expected model is 84 controls: 60 successful outputs containing 264
rows and 1,056 cells, plus 24 errors/refusals. The
[committed-build replay](results/typed-multivalue-outer-originals-2026-10-26.json)
matches all 84 controls with zero disagreements. Its frozen debug executable was
built at `fbe0e840`; all original/getter, reference-helper and executable pins
stayed unchanged. The earlier frozen INNER/CROSS-only CLI refused the first LEFT
query with exit status 2 and empty stdout. Preliminary uncommitted-build results
remain private and are not counted again.

```sh
python3 docs/testing/compare_typed_multivalue_outer_joins.py \
  --cli FROZEN_EXECUTABLE --cli-sha256 SHA256 \
  --compiled-revision REVISION --originals ORIGINAL_FIXTURE_DIRECTORY \
  --report FRESH_REPORT.json --log FRESH_LOG.jsonl
```

The runner verifies registered original/getter hashes, records exact commands and
outputs, checks exit status 2 and empty stdout for errors, and pins reference
helpers and executable before and after execution. Selected source objects at
the compiled revision include the explicit missing-parent implementation; this
is not a complete compiler-input manifest.

CLI `NULL` for absence is distinct from present `[]` and `[NULL]`. Output text
does not establish the API variant itself; separate typed query tests check
`QueryValue::AbsentMultivalue` and independent parent/element nullability.
Getter models establish stored values, rather than Microsoft Access SQL results.
Native outer-join query equivalence, display spelling, child order and existing
NaN payload/sign text limits remain separate qualification work.
