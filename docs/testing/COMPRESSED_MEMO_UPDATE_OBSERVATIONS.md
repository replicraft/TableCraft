# Ordinary compressed Memo UPDATE observations

Two finite public Jackcess 5.0.4 experiments completed **148 updates in 456
bounded processes** on fresh V2016/V2019 copies. All public update calls completed;
132 requested values reopened exactly and **16 leading-FEFF inputs reopened with
changed text**. These are independent-writer observations, not TableCraft candidate
acceptance or Microsoft Access save/reopen qualification. They do not expand the
current [raw Memo/OLE engineering preview](ACE16_LONG_VALUE_UPDATES.md).

The [compact receipt](results/compressed-memo-update-observations-2026-10-27.json)
pins executed sources, frozen selectors, originals, raw captures, later public
helpers and read-only verification results. No accepted raw 60-case input,
selector, model or result was changed or replaced by this series.

## Inputs and prerequisites

The original [producer/getter](CompressedMemoUpdateProbe.java) creates an inert
`CompressedMemos` table: ID LONG, CompactMemo MEMO with Unicode compression
requested, RawMemo MEMO without it, and an untouched OLE field. Actual persisted
Memo metadata is native type 12, flag 2, zero declared length, with extended flag
1 for CompactMemo and 0 for RawMemo; OLE is type 11, flag 2, extended flag 0.
The ID is fixed LONG, flag 3. No field/table properties are stored. No Memo index,
AppendOnly/history, calculated field, relationship or active content is added.

ID 91 initially holds `seed` repeated 1,200 times in both Memo fields and OLE
bytes 0 through 255. ID 92 holds `untouched-Ā中` in both Memo fields and OLE
`03 01 04 01 05`. The ID primary and two ascending ID declarations remain present;
the actual getters establish that IDAsc and IDTwin share one physical index/root.
Raw column blocks, sort descriptors, full row values/locations, index cursor
orders, historical counters, column map references and all map memberships are
captured. Metadata labels alone are not qualification.

Both original metadata/getter captures had to pass before the first update in
each experiment. Expressions are disabled, linked opens throw, updates use one
entry in `Cursor.updateCurrentRowFromMap`, and getters open read-only. Every
action copies its frozen source into a fresh output. Source parents and originals
are rehashed afterward. The threshold experiment reuses hash-equal copies of the
first experiment's originals; it creates no new database.

## First 92 actions

Twenty inputs are applied to each Memo field in each profile, followed by six
CompactMemo history actions per profile: 80 direct and 12 history updates, using
283 processes. Values include NULL/empty, ASCII ties, Latin-1, embedded NUL, wide
and nonBMP units, leading FEFF/FFFE, short inline/external boundaries and long
inputs. Boundary names were frozen hypotheses; actual encoded bytes determine
the recorded storage form.

| Input | CompactMemo encoded bytes / form | RawMemo encoded bytes / form |
| --- | --- | --- |
| `A`, `AB` | 2, 4 / inline, raw | 2, 4 / inline, raw |
| `ABC`, `éÿß` | 5 / inline, marker plus narrow bytes | 6 / inline, raw |
| 62 / 63 ASCII units | 64 inline / 65 single | 124 / 126 single |
| 4,074 / 4,075 ASCII units | 8,148 / 8,150 chained, raw | same |
| 8,143 ASCII units | 16,286 chained, raw | same |
| 4,075 Latin-1 `é` units | 8,150 chained, raw | same |

Mixed wide/nonBMP and embedded-NUL inputs remained raw UTF-16. No emitted
narrow/wide mode-switch stream was observed. In particular, the existing Short
Text preference cannot be applied to arbitrary-length Memo merely because all
units fit Latin-1.

Both field policies reopen leading FEFF alone as empty and FEFF followed by AB
as AB: eight actual mismatches across both profiles. The exact requested, stored
and getter bytes are retained. Leading FFFE followed by AB remains exact. These
observations do not authorize silently dropping a requested character in an
engineering encoder.

## Exact eligibility boundary

A separate frozen follow-on applies 12 values to both fields/profiles and four
CompactMemo history actions per profile: **56 updates in 173 processes**. It
covers ASCII/Latin-1 at 1,023, 1,024 and 1,025 UTF-16 units; 1,024-unit values with
NUL, wide or nonBMP units; marker-prefixed values; and compression-eligible →
ineligible → eligible → NULL histories.

| Input length in UTF-16 units | CompactMemo ASCII or Latin-1 bytes | RawMemo bytes |
| --- | --- | --- |
| 1,023 | 1,025, marker plus narrow bytes | 2,046 |
| 1,024 | 1,026, marker plus narrow bytes | 2,048 |
| 1,025 | 2,050, raw UTF-16 | 2,050 |

The 1,024-unit NUL/wide/nonBMP controls stay raw at 2,048 bytes. All non-NULL
threshold values use a single external row. The eight leading-FEFF values at
total lengths 1,024/1,025 lose that initial character on reopen; the leading-FFFE
controls remain exact. These are additional observations, not failed operations
or requested-value equality passes.

The exact Maven Central
[Jackcess 5.0.4 source artifact](https://repo.maven.apache.org/maven2/com/healthmarketscience/jackcess/jackcess/5.0.4/jackcess-5.0.4-sources.jar)
explains this independent writer's policy. Its binary POM identifies SVN tag
`jackcess-5.0.4`; archive/source hashes are in the receipt. LongValueColumnImpl.java
line 134 calls the text encoder. ColumnImpl.java lines 1780–1829 require the
compression flag, more than two units, every unit in 1..255, and no more than the
format limit. JetFormat.java line 1012 defines 1,024; the ACE inheritance appears
at lines 1112, 1151, 1181 and 1207. This reference and the measured boundary are
kept separate from native Access authority. Apache-2.0 source headers remain in
ignored review material; no upstream implementation or tables are copied into
production.

## Storage and preservation

Across both experiments the outputs comprise eight NULL, 44 inline, 78 single
and 18 chained values. There are 128 unchanged-length files, two one-page,
14 three-page and four four-page growths. The 60 new column-owned EOF pages match
exactly 60 global free-bit clears; no owned removal is observed. Existing LVAL
pages participate in 76 update placements. Hint bits are recorded as candidate
state, without inferring a fixed free-space threshold.

All **996 immediate and 1,302 cumulative ancestor chunk body/slot comparisons**
remain byte-exact, including unreachable old chains. All 1,332 map-wire checks
retain their representation and change only the complete observed membership-bit
deltas, including represented beyond-EOF bits. Unrelated pages, protected table
definition/index pages, other row bodies, other field descriptors/streams,
logical index order and historical counters remain unchanged. Old-chain
retirement, indirect LVAL map transitions and mode-switch authoring are not
established by this series.

The read-only [saved-result verifier](compressed_memo_observations/verify.py)
rehashes all recursive artifact pins, checks the saved assertions and derives
counts without opening a database or invoking Java/native tools. Its first two
runs verified 694 and 435 live pins respectively; those counts overlap and are
not a unique combined artifact count. The independent direct `ColumnImpl.read`
wrappers match 140 stored streams; wrapping a full stream for that oracle does
not claim its inline wrapper was the persisted storage form.

## Source identity and reproduction

The tracked Java helper is byte-identical to the compiled executed helper.
The two exact executed Python drivers and all frozen plans remain separately
pinned. The later [parameterized capture helper](compressed_memo_observations/capture.py)
combines original creation/reuse and count handling and adjusts the repository
root for its public location. It has passed both frozen plans in verify-only
mode; it has not rerun either public update experiment. The saved-result verifier
has executed against both unchanged reports.

For example, verify a retained report into a fresh output:

```sh
python3 docs/testing/compressed_memo_observations/verify.py \
  --report .cache/ace16-compressed-memo-observations-first-20261027/report.json \
  --output .cache/compressed-memo-verify-fresh.json
```

The capture helper requires explicit root-relative plan/input hashes, fresh
outputs and explicit `--execute`; no inspection activates content. The first
static asset preparation incorrectly listed a nonexistent namespace-package
`__init__.py`; that failure/source is retained, with zero Java/database actions.
A pre-execution correction explicitly included FFFE among marker observations.
Both actual compilations and both action experiments passed on their first
attempt. No source regeneration, metadata masking, selector adaptation or
candidate retries were used.
