# Old variable writer on combined-pressure sources

The frozen existing variable-record consumer now passes twelve controls on the
unchanged [maximum-width sources](VARIABLE_ENCODED_SPLIT_SOURCE_OBSERVATIONS.md).
All six no-ops produce exact source copies. All six requested growth operations
return `Unsupported(IndexedColumn)` without publishing an output. This is native
legacy-service evidence, separate from source construction, public writer actions
or a new combined DATA/index candidate.

## Actual consumer and inputs

The [historical build receipt](results/ace16-variable-example-build-2026-10-26.json)
identifies committed revision `87c35d435450251d69f305cb247741ccbf7be7e2` and the
17,512,496-byte frozen artifact, SHA-256
`2d2b173617e115a3bca3b10018086dab1c5d8f3d72b536fe3825d21e19cb4a5f`.
The artifact and all 107 selected historical source objects were checked before
execution. That selection is not an exhaustive compiler-input closure.

Every call explicitly supplies `--variable-updates`. Omitting that flag would
exercise the exact-slot service and could substitute a width refusal for the
index-topology limit. The real headerless five-field TSV parser receives original
row ordinal zero, column `KeyValue`, expected `aaa` or Binary `00`, and either the
same value or the maximum-width replacement. Text transports 255 UTF-8 `z` bytes;
Binary transports 510 hex characters for 255 `ff` bytes. Both fit the historical
1,024-byte encoded-value ceiling. Original physical row ordering independently
binds ordinal zero to ID 1 in all six targets.

| Profile | Target | No-op | Maximum-width replacement |
| --- | --- | --- | --- |
| V2016 | TextLegacy | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2016 | TextModern | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2016 | BinaryOne | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | TextLegacy | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | TextModern | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | BinaryOne | Byte-identical, zero changed rows | IndexedColumn, no output |

## First execution and preservation

The first actual execution completed all twelve calls, with no retries or altered
expectations: six no-ops, followed by six growth requests. No-op stdout reported
exactly zero changed rows. Growth stdout remained empty; exit status one and the
exact `IndexedColumn` error were required. All six growth output paths stayed
absent. Both originals, 29 frozen inputs and the freeze remained unchanged.

The native allowance remained 256 MiB, 100,000 rows and 100,000,000 steps, with
16 MiB TSV input and 4,096-assignment ceilings. Each child had a 30-second deadline
and live 16 MiB capture limit per stream. Every output name was fresh; capture,
partial-file inventories and per-input hash checks were retained in the report.
No JVM, new build, source generation, public UPDATE or new combined API ran.

The [compact receipt](results/variable-encoded-split-old-variable-native-2026-10-27.json)
pins actual arguments, streams, source identities and the historical artifact.
The [later saved verifier](variable_encoded_split_sources/verify_baseline.py)
checks 61 identities, exact TSV carriers and flag, physical ordinal bindings,
no-op bytes and refusal publication without starting a child process:

```sh
PYTHONPATH=docs/testing python3 -B docs/testing/variable_encoded_split_sources/verify_baseline.py
```

Its first saved-only invocation used an incorrect copied default directory and
failed before reading the report. The source and diagnostic are archived; only
the verifier defaults were corrected. No native call, source, request or recorded
outcome was repeated or changed.

These controls establish old-service scalar/table admission and its topology
refusal on these sources. They do not establish the stronger complete metadata
census, combined allocator, persisted new-key behavior, native Access save/reopen,
macros, journaling or production saves.
