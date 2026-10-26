# Comparer-qualified Short Text equality seeks

Native query execution can prune a qualified single Short Text index for a
single direct equality under the stored database General policy:

```sh
tablecraft query run \
  "SELECT TAG_GRP_ID FROM TAG_GRP_TB WHERE TAG_GRP_NME='Office Objects'" \
  --database docs/examples/Access_Example_VBA.accdb \
  --collation database --auto-index --format tsv
```

The registered example returns ID 9. An explicitly selected matching index can
use the same hint. Automatic selection retains sequential table encounter order;
explicit indexes retain their configured index order. The full original WHERE
always runs over returned candidates. False positives are permitted; skipped
matches and suppressed eager errors are not.

## Agreement and boundaries

The backend-neutral source opts into `Source::text_equality_hints`. The actual
`CollationResolver` can issue an execution-local `TextEqualityKey` for the bound
field policy and literal. Its bounded opaque payload is at most 32 bytes; query
code never interprets it. Constructors validate the envelope, not semantic
provenance. Reusing a policy ID with an unrelated comparer does not qualify a
hint. Default sources and resolvers decline it.

The Access resolver derives `GeneralEqualityPrefix` using the same General model
as residual comparisons. ACE owns prefix encoding, descriptor validation, physical
markers, direction and traversal. The adapter checks policy agreement and maps
only qualified Short Text columns. These are pruning hints, not equality keys
for grouping, DISTINCT, joins, persistence or serialization.

Execution checks current source metadata before invoking the resolver, then
checks it again during retrieval. Preparation remains metadata-only. Supported
operands are one direct field and a Text literal or currently bound Text parameter,
in either order. NULL parameters, oversized literals, computed values, raw String
carriers, global comparison overrides, inequalities, AND/OR combinations, joins,
derived sources, collections and Memo receive no new Text equality hint. Existing
safe numeric and NULL constraints retain their separate behavior. Partial AND
extraction remains forbidden because it can suppress eager failures.

Provider work consumes the same cumulative [comparison pool](../../query/COMPARISON_BUDGETS.md)
as residual comparison. Initial cancellation, exhausted quotas and selected index
failures propagate without retries or sequential fallback. No selected snapshot
becomes a complete-table cache. Later unfiltered consumers load independently.
Empty/ignored primary streams can retain all non-NULL candidates; this operation
makes no selectivity promise.

## Independent evidence

The [query runner](../../testing/compare_text_equality_queries.py) passes 792 controls
against 128 independent encoder/getter oracle cases across eight unchanged
Jet 4, ACE 2007, ACE 2016 and ACE 2019 Text files, covering both supported General
versions. It compares sequential, automatic, explicit ascending/descending and
parameterized results. Controls include table order, TOP/First/Last, NULL and
oversized parameters, eager errors, independent complete caches, quota refusals,
corrupt chosen roots and buffered failure output. The existing engine candidate
runner also passes all 824 controls after the owned-prefix refactor.

Focused Rust tests cover metadata changes before comparer activation, incompatible
resolvers with identical IDs, false positives and ignored hints, parameter reuse,
consumer limits, shared budgets, cancellation, descriptor refusal and selected
index reads. Independent encoder agreement is engineering evidence; Windows Access
SQL comparisons remain unrun. Broader regional policies, SQL inequality bounds,
compound/collection seeks and full read/query acceptance remain unfinished.

## Measured CLI cost

The [benchmark recipe](../../testing/benchmark_text_equality.py) verifies identical
answers before timing. Hyperfine 2.0.0 measured 15 runs after three warmups for
each path over 1,000-row tables, returning 94 rows for `Label='Straße'`.
The build was debug with default features and normal incremental compilation,
on Darwin arm64 with Rust 1.99.0. Explicit comparison allowances were 128 MiB and
100,000,000 steps; these do not change defaults.

| Family / General version | Sequential mean ± SD, ms | Automatic mean ± SD, ms |
| --- | --- | --- |
| Jet 4 / 0 | 45.8 ± 0.8 | 8.7 ± 0.9 |
| Jet 4 / 1 | 50.7 ± 4.7 | 8.8 ± 0.4 |
| ACE 2007 / 0 | 45.6 ± 0.9 | 8.5 ± 0.2 |
| ACE 2007 / 1 | 51.4 ± 4.8 | 8.5 ± 0.7 |
| ACE 2016 / 0 | 46.5 ± 1.7 | 8.3 ± 0.5 |
| ACE 2016 / 1 | 49.1 ± 1.1 | 8.3 ± 0.4 |
| ACE 2019 / 0 | 45.6 ± 1.2 | 9.8 ± 2.8 |
| ACE 2019 / 1 | 49.4 ± 1.1 | 8.8 ± 0.6 |

All samples, including the noisier ACE 2019 / 0 run, remain in the
[raw measurements](../../testing/results/text-equality-queries-2026-10-24.json).
[Metadata](../../testing/results/text-equality-queries-2026-10-24.metadata.json) records
binary/input hashes and answer digests. This is one warm-cache CLI workload,
not release-build throughput, a kernel benchmark or native Access performance.
