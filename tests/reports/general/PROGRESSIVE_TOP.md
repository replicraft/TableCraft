# Original unordered TOP streaming replay

The [accepted replay](results/progressive-top-2026-10-24.json) passes 156 commands
and 1,220,688 independently modeled cells: 128 candidate and 28 frozen buffered
baseline controls. All 1,900 pins match the accepted artifacts and committed
source revision `88380c9b`. This covers TableCraft's explicit engineering contract,
not native Access execution order/error precedence or a performance benchmark.

## Original inputs and independent model

[compare_progressive_top.py](compare_progressive_top.py) reuses the four original
20,000-row databases and inert public getter captures registered in
[the original corpus](PROGRESSIVE_QUERIES.md). It imports only the original Python
recipe's getter/TSV display helpers, not TableCraft execution code. No reference
SQL runs, no database is mutated and no additional upstream material is copied.
The Java producer, getters, generation metadata, baseline/candidate executables,
runner sources and frozen Rust inventory are pinned. Existing MIT recipe and
reviewed Jackcess provenance remain in [FIXTURES.md](FIXTURES.md).

Expected TOP output is computed by filtering getter records and taking their
first n qualifying rows. Counts 1, 129 and 20,001 cover a singleton, a batch
boundary and a count exceeding input cardinality. Successful shapes also cover
filtered/empty inputs and omitted unpaired UTF16 values constructed by ChrW.
All six successful shapes run at batches 1/128/1,000, candidate buffered execution
and the frozen pre-extension buffered baseline. All complete TSV cells match.

The discarded raw-String control deliberately keeps only 'ok' while later rows
construct unpaired surrogate intermediates. It must succeed like buffered TOP:
those cells are evaluated but do not undergo final strict Unicode publication.
This is not permission to suppress errors raised during their computation.

## Full-source validation after the cap

`TOP 129 ID, ID Mod (ID-15000)` still fails at ID 15,000. The independent integer
model gives remainder ID for each of the first 129 rows. Streaming publishes
129, 128 or zero rows at batches 1, 128 or 1,000 respectively; buffered execution
publishes nothing. Pending batches are dropped on error, even after TOP has
selected all its output rows.

Separate native/query row allowances of 1,000 still cause runtime resource
refusals after a complete 128-row output batch. Reaching TOP never refills a pool
or skips validation. PERCENT and ordered TOP remain refused before output.
Across all profiles the report records 120 successes, twenty division errors,
eight resource refusals and eight unsupported-shape refusals. Full stdout is
compared before recording its byte count and hash.

## Exact artifacts and reproduction

Candidate `.cache/tablecraft-bin/progressive-top-cli-20261024`:
`ee7a40c313daf7f5e874235857e103bd937df5ef240dcf2b0883f9d062d077d4`.
Baseline `.cache/tablecraft-bin/progressive-query-cli-20261024`:
`771116caa0b27854de4d2737ef9962e7c3d696020a98b309cd2277cab58fe3b0`.
The 1,881-file candidate Rust manifest
`.cache/progressive-top-rust-source-20261024.manifest` has SHA-256
`2f14fd22dcfca0ee53819261d1d02e2aa76da877a1658a27c5110cd47eba771e`.
The accepted report has SHA-256
`fe7053913ed99b8ac6e031eed636b729c60db60c5fa8fffe28062ffd011ceaa0`.

```sh
python3 docs/testing/compare_progressive_top.py \
  --cli .cache/tablecraft-bin/progressive-top-cli-20261024 \
  --cli-sha256 ee7a40c313daf7f5e874235857e103bd937df5ef240dcf2b0883f9d062d077d4 \
  --baseline .cache/tablecraft-bin/progressive-query-cli-20261024 \
  --baseline-sha256 771116caa0b27854de4d2737ef9962e7c3d696020a98b309cd2277cab58fe3b0 \
  --directory .cache/tablecraft-fixtures/progressive-query-20261024 \
  --source-manifest .cache/progressive-top-rust-source-20261024.manifest \
  --manifest-sha256 2f14fd22dcfca0ee53819261d1d02e2aa76da877a1658a27c5110cd47eba771e \
  --report .cache/progressive-top-replay.json
```

The runner refuses existing reports and checks sources before and after replay.
Restore the accepted production revision for reproduction; later source changes
are not silently accepted. Parent verification separately checked all source
pins against the immutable committed archive after later work resumed.

[The verification record](results/read-query-progressive-top-2026-10-24.json)
retains 88 unique passing Rust tests, strict Clippy and final CLI build logs.
The [TOP contract](../query/PROGRESSIVE_QUERIES.md#unordered-top-counts) cites the primary
Microsoft syntax reference and distinguishes the native qualification boundary.
