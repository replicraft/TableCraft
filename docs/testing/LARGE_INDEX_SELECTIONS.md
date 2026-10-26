# Selected index reads above 4,096 table data pages

Qualified partial indexed reads now use fully validated bitmap membership instead
of a complete materialized list of table data-page addresses. The engine retains
its existing owner, live-slot, relocation, schema and row/key checks. Complete
indexed reads, full checks and index-owned maps keep their old 4,096-address
ceiling, preserving the existing mutation gates. There is no new index codec,
coercion, implicit fallback or shared-file coherence claim.

The [native verification record](results/large-index-selections-native-2026-10-26.json)
records 170 unique passing engine tests, ten new: six public API tests and four
private membership checks. Ten existing allocation tests and 150 existing
index/constraint/write-preview/page regressions also pass. Selected strict Clippy,
minimal library compilation and formatting pass. One regression command initially
named the nonexistent `large_maps` target; it ran no tests and is excluded. Its
helper belongs to the actual `table_pages` target, whose 35 tests passed.

New controls cover inline and indirect ownership above the materialized ceiling,
first/middle/last/no-match keys, hidden key projection, sequential table order,
whole-map preflight on empty matches, a corrupt later bitmap before any record,
missing membership, wrong owners/keys, cancellation and resource exhaustion.
The sparse source refuses unrelated data reads, so a sequential fallback cannot
satisfy selected-read expectations. Private tests also check arbitrary membership
order, cache reload validation and bounded retained memory. Synthetic fixtures
are not native Access qualification.

## Original public-writer baseline

[LargeUsageMapProbe.java](LargeUsageMapProbe.java) is repository-owned code under
the root license. It retains the existing `UsageMapFixture.java` geometry: ID,
twelve Short Binary fields of 240 bytes, an ordinary primary Int32 index, and one
row per owned data page. It selects ACE16 V2016/V2019 and 5,000 rows. Public builders
write the new files with expression evaluation disabled; linked opens are refused.
No imported fixture, reference SQL or active content executes.

The reviewed Jackcess 5.0.4 JAR has SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Public row and index-cursor getters establish all IDs and stored Binary bytes.
Explicitly identified pinned `TableImpl`, `IndexImpl` and `RowIdImpl` getters
supply physical page/identity metadata. The Python baseline independently models
every concatenated twelve-field Binary payload and checks its getter SHA-256.
The index cursor returns all 5,000 exact IDs. Each table owns 5,000 data pages;
each index owns only seventeen pages.

The public Java source is byte-identical to the actual cached generator/getter,
SHA-256 `bb207c72e9324f166a160929e9596da09d4b15b45de7d2414d44ec20eed88b13`.
The immutable originals remain under
`.cache/large-indexed-table-probe-20261026/`:

| Profile | Bytes | SHA-256 |
| --- | ---: | --- |
| V2016 | 20,934,656 | `a92022ed6a1a63335cf82901da00d9d9b7d6013792042f90fce328304c0319f4` |
| V2019 | 21,004,288 | `0dc0bf89240ab499427e546502e983547715b3a27698dd4c1eb6a86d96cf558b` |

Generation/getter stdout, stderr and exact commands are retained beside them.
The raw read-only getter stdout SHA-256 values are
`09018306f3c8b51cbb3e513504e746b7d2edd0333e02bea30387e29aa25feb33`
(V2016) and
`46022af4c5f7719abdb0906ede3212124c1936589e53990ac25703361a8559ea`
(V2019); both stderr files are empty.
`commands.json` records all four successful calls. `baseline-reproduction.json`
(SHA-256 `6ef8d7453b754c1fc4864c4fcf182ee066dff9b8ea2f3aa2fc8e270d90f413ee`)
records twelve baseline CLI controls: both sequential ID projections succeed;
first/middle/last/absent point selections and automatic range selection each refuse
with a structural-limit error and empty stdout. The baseline frozen executable is
`typed-multivalue-outer-20261026`, SHA-256
`44679046d3b5320f928b0967612dc9b79a1fb8c6de2df4b868e6f32f3c7f41c4`.
The first private probe used `--auto-indexes` instead of the actual `--auto-index`
flag; its source/log remain excluded, and no expected values changed.

Fresh reproduction commands use a reviewed JAR and paths that do not exist:

```sh
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar \
  docs/testing/LargeUsageMapProbe.java generate path/to/new.accdb V2016
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar \
  docs/testing/LargeUsageMapProbe.java oracle path/to/new.accdb
```

Use V2019 for the other profile. Exact hashes identify the accepted originals;
fresh generation is not promised byte-identical. A post-change original-file CLI
replay is separate from these baseline observations and focused engine tests.
