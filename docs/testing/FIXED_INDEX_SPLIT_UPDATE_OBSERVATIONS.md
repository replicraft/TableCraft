# Exact fixed numeric split candidate acceptance

The first committed-consumer replay now passes all eight Byte, Int16, Int64 and
Currency MAX updates on the two unchanged ACE 16 originals. All complete candidate
images match the independently frozen expectations, and all fresh read-only
getters, key/ROW streams, physical indexes and allocation maps agree. This is
bounded engineering candidate evidence, not normal-save or Microsoft Access
qualification.

The [receipt](results/ace16-fixed-index-split-originals-2026-10-27.json) separates
the actual first replay from the earlier
[source/no-op phase](INTEGRAL_INDEX_SPLIT_SOURCE_OBSERVATIONS.md) and
[public-writer MAX observations](INTEGRAL_INDEX_SPLIT_OBSERVATIONS.md).

## Immutable inputs and actual execution

The consumer was built from committed revision
`809c5e8d72ed1afe698f565f8447005292739b05` in an isolated Git export. Its
[build receipt](results/ace16-fixed-split-example-build-2026-10-27.json) pins the
executable, build driver/log, 215 selected Git objects and the complete 2,556-file
export inventory. All exported sources remained unchanged through the build.
Debug symbols were disabled explicitly; this was an unoptimized development
build with ordinary debug assertions, not a release benchmark.

The independent complete-image plan was frozen before this consumer existed.
It derives field offsets, exact unsigned/signed/scaled widths, original ROW
references, physical alias deduplication, root/leaf framing, EOF ownership and
global free-map edits from the original files. It does not use production
serializers, candidate outputs or public unused-page scratch as goldens.
The published [candidate model](integral_split_originals/candidate_model.py) is
byte-identical to that executed proof source.

The actual first replay executes sixteen fixture commands: eight native candidate
attempts and eight fresh read-only descriptions. All exit zero; all eight cases
meet both complete-image and fresh-getter checks. There are no retries, source
regeneration, public UPDATE calls, changed selectors or changed allowances. The
source files, model, runner, compiled artifact and other frozen inputs remain
unchanged. Compiled source pins are checked against Git objects at the recorded
revision, independently of concurrent working-tree changes.

The consumer retains its 256 MiB/100,000-row/100-million-step allowance. Fresh
getters use the previously pinned Java classes, an explicit 512 MiB heap and the
existing 120-second/16 MiB stream controls.

## Verified candidates

Each format has these four cases, always ID 1/original ordinal zero, KeyValue
zero to its exact typed maximum:

| Field | Replacement | Rows in target | Entries in each new leaf |
| --- | --- | --- | --- |
| Byte | 255 | 900 | 450 |
| Int16 | 32,767 | 900 | 450 |
| Int64 | 9,223,372,036,854,775,807 | 400 | 200 |
| Currency | 922,337,203,685,477.5807 | 400 | 200 |

All sixteen physical indexes retain their original roots, now at height one with
a complete left maximum and right tail. Each physical index allocates right then
left at EOF, in physical data-number order. Alias shares the ascending physical
tree. Each candidate grows by four pages (16,384 bytes); no original page retires.
Fresh leaves have prefix zero and zero unused padding. Original root residual
bytes remain unchanged. Public reference leaves' nonzero unused scratch remains
a separate observation.

Only the selected fixed field bytes change in ordinary records. Original row
references, markers, other typed/opaque values, historical/getter counters,
unaffected index pages and all bytes outside the independently derived spans
remain intact. Exact owned/global map bytes compose across the four allocations.
The independent model checks 5,200 selected-table rows across these eight cases;
fresh getters and raw decoders also check the other tables.

## Retained proof check

From the repository root, while the recorded ignored artifacts are available:

```sh
python3 docs/testing/integral_split_originals/verify_candidates.py \
  --report .cache/ace16-integral-split-acceptance-first-20261027/report.json
```

This later verifier rehashes live artifacts, checks compiled Git objects, rebuilds
all eight independent images and reanalyzes saved getter output. It invokes no
native consumer, JVM, generation or database UPDATE. Its first run passes;
separate independent saved-copy verification also passes without repeating any
fixture command. The original raw first report remains unchanged.

These cases qualify root-leaf typed growth only. Recursive typed growth has
separate literal tests under the [implementation contract](ACE16_FIXED_INDEX_SPLITS.md).
Oversized balanced halves still refuse; repeated splitting, broader codecs,
map growth, general allocation, required data macros and native Access save/reopen
qualification remain separate work. This non-journalable preview does not enable
ordinary backend or CLI editing.
