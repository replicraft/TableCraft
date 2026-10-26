# Ordinary visible-Memo history flag family

Fresh original reference-builder/getter evidence identifies a second visible-Memo
history admission defect after the Unicode-compression fix: Hyperlink Memos already
read as ordinary stored Text, but their valid history association remained unavailable
because the visible-Memo gate required ordinary flags 0x02 exactly.

History selection now accepts the complete observed ordinary visible family:
flags 0x02 or 0x82 (updatable with optional Hyperlink), combined with extended
flags 0 or 1 (optional Unicode compression). It still requires a variable Memo
and its existing ordinary scalar codec. Unknown, calculated and fixed variants
remain refused. History templates and flat child fields retain their original
ordinary/extended flag gates; their codecs and association checks do not change.
See the [stored-history contract](../access/collections/STORED_HISTORY_QUERIES.md).

## Independent original corpus

The [producer](MemoHistoryFamilyFixture.java) creates separate plain, Hyperlink and
RichText families in fresh ACE 2007/2016/2019 files, with uncompressed Notes and
compressed Details in separate tables. All four ordinary flag combinations occur;
RichText properties exercise the same physical Memo family without inferring a
new carrier. Expression evaluation remains disabled throughout.

The [metadata oracle](MemoHistoryFamilyOracle.java) opens originals read-only,
disables evaluation and combines public Column flags/association/property getters
with pinned VersionHistoryColumnInfoImpl metadata getters. It records actual
hidden/member/flat names, compression booleans, Hyperlink booleans and original
BYTE TextFormat declarations. Existing original current-value/flat-history oracles
retain exact stored Text, physical child order and DATE carriers.

Hyperlink component strings and RichText markup remain raw Text. No URL is
opened, provider activated, HTML rendered, components parsed or content normalized.
Current values remain separate from older versions. The corpus retains Unicode,
empty/NULL cells, larger text, duplicate values/timestamps and empty histories.
Each file has ten parent rows and twelve stored versions; all originals remain
preserved under ignored local fixture directories.

## Query and refusal scope

The [runner](compare_memo_history_family.py) checks actual native current/hidden/
child column flags against reference declarations and typed feature properties.
It reuses the original stored-history query model in ordinal and stored-database
comparison modes for every family. That model compares projection order,
current/history alignment, nullable members, exact time displays, grouping/counts,
collection presence, member criteria and explicit mode/resource refusals against
independent inert getters. It does not infer native Access SQL member syntax.

Focused engine controls test all four visible combinations and reject unknown
ordinary flags, calculated extended flags, fixed layouts and unqualified child
Hyperlink flags. An adapter control checks metadata-only association binding and
existing member restrictions. Existing compressed-history tests retain complete
cache reuse, late-orphan failure, budget/cancellation and strict template/flat
compression refusal. No mutation or native application behavior is qualified.

## Accepted results

On 24 October 2026, the [accepted report](results/memo-history-family-2026-10-24.json)
records 594 passing controls: eighteen original metadata/property comparisons
plus 96 query controls per family in each of ordinal and stored-database modes.
The nine original files retain 90 total parent records and 108 stored versions.
All 32 pinned producer, oracle, helper, implementation, original input, reference
JAR and executable hashes remain unchanged after replay.

The immutable CLI SHA-256 is
`15e9822bbac91a3039d29c6b9bfb0465ac2a6d8d0caf7686a75e6fe1a42afd07`.
The complete engine/adapter history targets pass 22 tests, and scoped strict Clippy
passes for both integration targets. The reused query replay explicitly permits
a 64 MiB cumulative native byte pool; other controls and defaults do not change.
These are bounded offline engineering results, not native Access qualification.
[Fixture provenance](FIXTURES.md#original-ordinary-memo-history-family-inputs)
records all original hashes and excluded capability probes.

```sh
python3 docs/testing/compare_memo_history_family.py \
  --cli .cache/tablecraft-bin/memo-history-family-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/memo-history-family-20261024 \
  --report /tmp/memo-history-family.json
```
