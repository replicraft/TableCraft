# Advanced-field storage evolution

Reviewed 24 October 2026. The existing reader passes a fresh original ACE
2016/2019 corpus that combines advanced fields with historical row layouts,
updates, deleted roots and relocated records. This expands independent storage
and explicit query-model evidence; it does not qualify Microsoft Access SQL,
recalculation, concurrent files or native application saves.

## Original workload

The [inert producer](AdvancedEvolutionFixture.java) starts with 48 rows containing
ordinary scalar fields. It then adds multivalues, attachments, a cached Int32
calculation and an append-only Memo/history field, followed by 48 more rows.
A later stage adds cached Memo/Decimal columns and eight Boolean fields before
adding another 64 rows. Expression evaluation remains disabled throughout.

A fixed subset remains untouched after the later additions. Other roots receive
stale cached values, scaled Decimals, larger ordinary/cached Memo text, duplicate
and NULL multivalue children, original attachment contents through 20,000 bytes,
empty files and explicit stored history. Selected child lists are deleted and
repopulated. Six parents with empty collections are deleted through a fresh cursor.
No formula, macro, provider or application code runs.

The [read-only oracle](AdvancedEvolutionOracle.java) emits exact stored scalar
getters and flat child rows in their physical table order. It checks attachment
identity and encoded-byte agreement against the separate public attachment API,
and retains original decoded bytes. History values keep their actual hidden field
and member names, paired with DATE bits from public in-memory serialization.
Public RowId locations let the runner independently inspect the original slot
headers and historical field counts without using TableCraft's format constants.
Reachable overflow targets may carry the hidden/deleted bit; they are not counted
as separate live or deleted parent roots.

| Observed structure | ACE 2016 | ACE 2019 |
| --- | ---: | ---: |
| Live parent roots | 154 | 154 |
| Untouched historical 8-field roots | 7 | 7 |
| Current 18-field roots | 147 | 147 |
| Relocation links | 120 | 120 |
| Live multivalue children | 303 | 303 |
| Stored attachments | 130 | 130 |
| Stored versions | 30 | 30 |

The historical layouts cross one-byte and three-byte NULL masks. Original values,
missing later caches and Boolean defaults are compared with the independent
reader rather than inferred from the current schema alone.

## Read/query checks

The [runner](compare_advanced_evolution.py) compares ordinary and cached scalar
projections, cached predicates with/without automatic hints, multivalue expansion
and counts, mixed cache/collection criteria, exact encoded and explicitly decoded
FileData, and stored history members/dates alongside the current Memo. Required
indexes still refuse cached selections. Ordinary queries do not gain implicit
calculation access.

Disposable copies test an orphaned multivalue child and a late unknown calculation
wrapper. Explicit collection/cache queries fail without stdout even under TOP;
unrelated ordinary projections and attachment metadata remain readable. Originals
are never changed. The original getter oracle's `--orphan` operation is restricted
to an explicitly named disposable `orphan.accdb` copy and is not an application
write capability.

The [accepted report](results/advanced-evolution-2026-10-24.json) records 60 passing
controls and 18,584 compared query cells across the two files, with unchanged
producer, oracle, implementation, helper, JAR, input and immutable CLI hashes.
The executable is the previously fixed cached-query binary, SHA-256
`d0666d2d733e746b5cdbda776d55aca0b263e55aa566cff93e715a2fd7e9de15`.

Larger Memo reads exceed the default one-million-step native work pool. Accepted
replay explicitly supplies 100-million-step native and query pools; existing byte,
row, value and output ceilings remain unchanged. This is bounded correctness
coverage, not throughput or peak-memory measurement. No production Rust change
was needed for these cases. Full native collection/history semantics, calculated
freshness, other historical variants and Windows comparisons remain separate.

```sh
python3 docs/testing/compare_advanced_evolution.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli .cache/tablecraft-bin/cached-query-20261024 \
  --ace2016 .cache/tablecraft-fixtures/advanced-evolution-deleted-20261024/V2016.accdb \
  --ace2019 .cache/tablecraft-fixtures/advanced-evolution-deleted-20261024/V2019.accdb \
  --report /tmp/advanced-evolution.json
```

[Fixture provenance](FIXTURES.md#advanced-field-storage-evolution-inputs) records
accepted original hashes and excluded investigation inputs. Generated databases,
compiled fixture classes and the executable remain ignored local artifacts.
