# Original conditional calculation preview replay

The [accepted report](results/calculated-iif-2026-10-24.json) passes 86 controls
and 160 independently modeled cells across fresh V2016/V2019 originals, with
34 successful commands and 52 intended buffered refusals. All 1,906 pins
remain unchanged. The 1,892-file Rust inventory exactly matches commit `11ccd44d`.

## Scope and producer

Microsoft's [table-calculation example](https://support.microsoft.com/en-us/office/add-functions-to-access-expressions-72a471a0-4f0e-4d49-923b-be97b0fa4bb0)
supports narrow `IIf` admission. This verifies the explicit
[preview profile](../CALCULATION_PREVIEWS.md#deterministic-iif-calls), not native
ACE conversion, freshness or a general calculated-function whitelist.

Original MIT [CalculatedIIfFixture.java](CalculatedIIfFixture.java) authors two
fresh databases through the reviewed Jackcess 5.0.4 public builders. Expression
evaluation stays disabled; read-only ordinary getters and a throwing link resolver
capture declarations and stale caches without running formulas or linked content.
No upstream fixture or implementation is copied. Originals stay ignored under
`.cache/tablecraft-fixtures/calculated-iif-20261024/`.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| V2016.accdb | 393,216 | `0918d2bcf67eb8ab12a9f1439149262ba6bb8ab165f0d303b16f28950ea9be3d` |
| V2019.accdb | 466,944 | `ffd7835b2498bfe670b8c87c4eeee2823c3f80ca622a8d6228599c6ad91e0a9f` |

[compare_calculated_iif.py](compare_calculated_iif.py) independently models branches,
NULL conditions, numeric/Text/Boolean outputs and transitive dependencies from
ordinary Input/Label getters. Seven successful formulas run on populated and empty
tables. Sixty-six recomputed values differ from raw caches; separate cached queries
prove their preservation. Unsupported declarations include unknown/clock/IsNull
calls, wrong arity, omitted arguments and hidden cycles. Public-builder persistence
of these negative controls does not qualify them as valid native formulas.
An inactive division error fails populated previews eagerly but permits empty EOF.
Quotas require buffered refusals; ordinary queries still refuse automatic recomputation.

## Reproduction and verification

Use the runner's `--generate --directory PATH --jar PATH` to author a new corpus.
Replay requires `--directory`, `--jar`, `--cli`, `--cli-sha256`, `--source-manifest`,
`--manifest-sha256` and a fresh `--report` path. It refuses report overwrite and
verifies original/generator/class/JAR/getter/source/executable hashes before and
after execution. The report retains exact commands and observations; generation
commands and hashes remain in the pinned ignored `generation.json`.

[The scoped verification record](results/read-query-calculated-iif-2026-10-24.json)
records 76 unique Rust tests, strict Clippy, formatting, minimal Access features,
CLI build and archive verification. No Windows comparison, complete workspace
suite or performance claim is supplied.
