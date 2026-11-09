# Complete ignore-NULL physical checks

The [original comparison](results/ignored-index-coverage-replay-2026-10-25.json)
passes 172 commands: 86 current complete checks and 86 baseline checks over the
same original ACE16 V2016/V2019 files. Every command succeeds with its independently
modeled coverage. Current checks qualify 318 of 342 logical index profiles;
the baseline qualifies 116. The 24 unique Memo/digest profiles remain explicitly
unsupported. This compares complete physical coverage, not query results or native
Microsoft Access integrity decisions.

The current executable is
`.cache/tablecraft-bin/read-query-p0-cli-20261025`, SHA-256
`9ef8530041ddefff1b888b6b58757a35221297044497fa9a32360570c1ed259f`.
It was built at base revision `612bd74a` with eight pending native production
snapshots, all identified in the report and build record. Those exact native
snapshots were subsequently committed as `9c4fad8c`. The frozen executable and
snapshot hashes identify this run; the base revision alone does not describe
the entire build. The baseline is the preserved `read-query-distinct-wide-cli-20261025`
executable, SHA-256
`121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c`.

## Originals and independent model

[IgnoredIndexCoverageFixture.java](IgnoredIndexCoverageFixture.java) is original
repository-owned Apache-2.0 code. It uses the reviewed Jackcess 5.0.4 JAR,
SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Public builders author fresh inert tables; public row/index-cursor getters read
back every cell and membership. Expression evaluation stays disabled and the link
resolver rejects target opens. No reference SQL or imported active content runs.
The pinned `ColumnImpl` General sort constants and `TableImpl`, `IndexImpl` and
`RowIdImpl` metadata getters are identified implementation interfaces, not claimed
as public APIs. No upstream source or fixture bytes are copied.

Each profile contains 43 tables, 10,393 actual getter rows and 171 logical indexes,
including 128 ignored indexes and 89 indexes owning multiple pages. Matched ordinary,
ignored, shared-alias and descending definitions cover Boolean, narrow/wide integers,
exact Currency/Decimal, IEEE bits, DATE, GUID, General0/1 Text/Memo and Short Binary.
Mixed tuples reach ten key components; unique partial-NULL tuples and intentionally
unqualified unique Memo/digest definitions retain separate coverage.

The Python model resolves the actual indexed column names and includes each root
unless every indexed getter value is NULL. It validates all 342 physical cursor
populations independently, including duplicate-free root identities. It never uses
index counters as the live population oracle or normalizes/hashes production keys.
Actual Boolean false, empty Text/Binary and partially NULL keys remain values.
Coverage status models preserve the established codec/uniqueness boundaries.

`EvolvedPair` begins with 17 ID-only rows before adding two fields. Four new rows
then exercise partial and all-NULL tuples. Read-only getter locations plus direct
original-byte row-header reads establish stored column counts of one for all 17
historical roots and three for all four later roots, without rewriting old rows.
Ordinary indexes contain 21 roots; ignored indexes contain three. This tests whole
row history together with index omission, rather than only adding NULL to modern
rows.

The two immutable originals under
`.cache/tablecraft-fixtures/ignored-index-coverage-20261025/` are:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| IgnoredProfiles-V2016.accdb | 4,157,440 | `90c5fc95e47be4996924ac6b0fe232a5209a12689527c5c05be593ec0b7f08df` |
| IgnoredProfiles-V2019.accdb | 4,227,072 | `2db1bd079e3f9df28dc82c2d3eb9dc82753a8a75273b67213eefa5e2ce872b05` |

Generation and original getter commands are preserved in
`producer-getter-commands.json`; location getter commands are in
`location-getter-commands.json`. The generation source snapshot is retained as
`source-at-generation.java`. The final helper adds only getter LOCATION output and
its import; producer methods are unchanged. A fresh reproduction uses:

```sh
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar \
  docs/testing/IgnoredIndexCoverageFixture.java generate path/to/fresh.accdb V2016
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar \
  docs/testing/IgnoredIndexCoverageFixture.java oracle path/to/fresh.accdb
```

Use V2019 for the second profile. Exact original hashes identify the accepted
inputs; a new generation is not promised byte-identical.

## Accepted replay and limits

```sh
python3 docs/testing/compare_ignored_index_coverage.py \
  --cli .cache/tablecraft-bin/read-query-p0-cli-20261025 \
  --baseline-cli .cache/tablecraft-bin/read-query-distinct-wide-cli-20261025 \
  --originals .cache/tablecraft-fixtures/ignored-index-coverage-20261025 \
  --build-record .cache/read-query-p0-cli-20261025.json \
  --report docs/testing/results/ignored-index-coverage-replay-2026-10-25.json \
  --log .cache/ignored-index-coverage-replay-accepted-20261025.jsonl
```

The runner requires fresh output paths. It pins seventeen artifacts and eight
selected native source snapshots before/after, compares complete status/count
output, and retains every command and disagreement. Both versions use the same
explicit 128 MiB native byte and 20 million step allowances; defaults are unchanged.
The initial 86-command candidate sweep remains private diagnostic evidence and is
not added to the accepted command count. Writer duplicate-null-entry warnings are
preserved verbatim; actual cursor memberships agree with all independently computed
populations. They are not silently discarded or treated as corruption.

The [implementation record](results/read-query-ignored-index-coverage-2026-10-25.json)
records 213 unique passing checks, including 15 new tests, strict linting and minimal
build. Synthetic corruption tests cover missing roots, injected all-NULL entries,
wrong keys, required/unique violations, cancellation and budgets. Public full/NULL
retrieval, additional automatic selection profiles and all mutation eligibility
retain their existing refusals. The originals remain unchanged; Windows/native
Access qualification remains separate.
