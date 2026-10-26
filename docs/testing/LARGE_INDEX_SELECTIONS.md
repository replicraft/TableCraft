# Indexed reads above 4,096 table data pages

Qualified partial indexed reads and explicit complete `IndexReadRequest` snapshots
use fully validated bitmap membership instead of a complete materialized list of
table data-page addresses. The engine retains its existing owner, live-slot,
relocation, schema and row/key checks. Explicit complete reads retain full index
count validation and a caller-selected positive row ceiling. Legacy complete
entry points, full checks, mutation consumers and index-owned maps keep their old
4,096-address ceiling. There is no new index codec,
coercion, implicit fallback or shared-file coherence claim.

The initial partial-read [native verification record](results/large-index-selections-native-2026-10-26.json)
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

## Explicit complete-request extension

Explicit `read_table_index_with_request` and table-order requests now use the
same fully preflighted bitmap membership even for `All`. Selection completeness
still controls full entry-count validation; an independent private ownership
strategy controls map storage. Complete snapshots validate the declared count
against the caller's cap, validate all index entries and fetch/check all returned
roots before exposing a snapshot. The whole index reference list and completed
result remain retained under cumulative budgets.

The [complete-request record](results/large-complete-index-native-2026-10-26.json)
records 165 unique passing tests, nine new: two implementation controls and seven
independent public-API contract tests. Six existing selection tests and 150
index, constraint, preview and page regressions pass. These checks cover explicit
key/table order and hidden key projection; declared-count disagreement; invalid
and insufficient caps; missing ownership, owner/key corruption and a late invalid
bitmap; cancellation after actual bitmap I/O; all three cumulative budget pools;
and the unchanged separate index-owned map ceiling. The sparse fixture contains
three actual rows and 5,000 owned addresses; it does not itself establish a
5,000-live-row complete snapshot. The original-file complete replay is separate.

Legacy complete reads/projections, full checks and mutation consumers keep their
materialized table-map gates. Index-owned maps retain their separate ceiling.
The first strict lint found only unused shared-test helpers in the original
selection target; the shared fixture module now explicitly permits unused helpers.
Its original log is retained; final strict lint, minimal compilation and formatting
pass. No runtime code or expected result changed in response to a failing test.

## Complete original-file replay

The [complete-snapshot runner](compare_large_complete_indexes.py) and
[record](results/large-complete-index-originals-2026-10-26.json) use a frozen debug
CLI built at `5b0ca3eb`. All eighteen controls match, with zero disagreements:
50,004 result rows and 130,000 Binary-field comparisons across both unchanged
originals. These counts include repeated operations and the extra hidden-key
projection; they are not counts of distinct database rows.

Complete IDs, reverse query ordering, Count, and a projection omitting the key
all agree. Two payload projections cover every one of the twelve 240-byte Binary
fields on all 5,000 rows per profile. Before output comparison, the producer's
per-field formula is checked against each independent getter's full-row digest.
Payload output is split to fit the existing 16 MiB per-command output ceiling.
The runner explicitly chooses cumulative native/query pools; defaults are unchanged.

A 4,999-row allowance refuses a complete read while a point selection under that
same allowance succeeds. A one-byte output allowance returns its expected error
with empty stdout. Legacy/check/write and index-owned ceilings are independently
covered by the engine tests; the CLI uses the explicit request APIs.

Full payload transcripts remain private. The compact public record contains
per-command output hashes and counts, with the raw transcript pinned by SHA-256.
Original/getter, executable and reference hashes stay unchanged. No native
Microsoft Access execution, save/reopen or shared-file evidence is claimed.

```sh
python3 docs/testing/compare_large_complete_indexes.py \
  --cli FROZEN_EXECUTABLE --cli-sha256 SHA256 \
  --compiled-revision REVISION --inputs ORIGINAL_DIRECTORY \
  --report FRESH_REPORT.json --log FRESH_LOG.jsonl
```

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
fresh generation is not promised byte-identical.

## Committed-build original replay

The [read-only runner](compare_large_index_selections.py) and
[record](results/large-index-selections-originals-2026-10-26.json) compare 22 CLI
controls from the frozen debug executable built at `66041fd7`. All match, with
zero disagreements and 10,028 result rows compared. Required point/range and
automatic selections now succeed over both 5,000-owned-page tables, including
empty matches. Two selected payload cases per profile hash all twelve decoded
Binary fields against the getter digest. Sequential controls still return all
5,000 IDs; at that historical revision, complete indexed reads retain their expected structural refusals.

Original/getter, writer recipe, executable and reference hashes stay unchanged.
This is independent public-writer/getter conformance, not native Microsoft
Access execution, save/reopen or shared-file qualification.

```sh
python3 docs/testing/compare_large_index_selections.py \
  --cli FROZEN_EXECUTABLE --cli-sha256 SHA256 \
  --compiled-revision REVISION --inputs ORIGINAL_DIRECTORY \
  --report FRESH_REPORT.json --log FRESH_LOG.jsonl
```
