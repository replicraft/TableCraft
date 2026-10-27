# Repeated numeric index split observations

Two new ACE 16 public-builder originals demonstrate a concrete remaining split
limitation: a first balanced split can leave a half too large. The source phase
passed its first nine processes. The separate action phase passed its first
thirteen-process capture: four current typed-service attempts refused atomically,
and four fresh public updates succeeded by splitting again. No repeated-split
implementation or candidate replay is established by these observations.

The [source receipt](results/ace16-repeated-index-split-sources-2026-10-27.json)
and [reference receipt](results/ace16-repeated-index-split-reference-2026-10-27.json)
retain distinct phases, exact commands, original/copy/tool hashes and raw reports.
Jackcess 5.0.4 is the reviewed independent public writer, not Microsoft Access.
No original database was patched, regenerated or replaced after its first run.

## Source phase: nine processes

The repository-owned [producer](repeated_split_originals/RepeatedSplitFixture.java)
uses public builders with UTC, expression evaluation disabled, foreign-key
enforcement enabled and a throwing link resolver. Each V2016/V2019 database has
900 nonunique zero keys in each of `Int64Keys` and `CurrencyKeys`, plus one separate
Memo/OLE preservation row: **1,801 live user rows**. ID1..900 and Marker=ID*17 are
fixed. ID and KeyValue have actual Boolean Required properties. Int64 uses native
BIG_INT and Currency exact scaled 64-bit units through scale-four BigDecimal.
Both formats actually supported these declarations; no format fallback occurred.

Each target has an independent ID primary index, required nonunique ASC/DESC
KeyValue indexes and an actual ASC logical alias. Each affected physical index
is one root leaf with 900 complete key-plus-row entries, prefix nine and seven
free bytes. ID1 is measured live ordinal zero in every target. All selected native
field spans, carriers, properties, masks, physical row references, complete index
orders and represented future map cells passed the source gates.

| Profile | Original bytes | EOF page | SHA-256 |
| --- | ---: | ---: | --- |
| V2016 | 573,440 | 140 | `c8b1feaa3acc119dd3d867398785217ae62735088b46d5cb9f309ee3d6e6ac4d` |
| V2019 | 647,168 | 158 | `aeb73b84f68288eb763523948273a82e1e0ff0112b5815fa61c041ee2359652f` |

The fixed inventory was one javac, two generations, two complete getter/map
descriptions and four committed typed-facade zero-to-zero calls. All four
candidates were byte-identical fresh outputs. The process ceilings were 4,096
rows, 64 MiB files, 512 MiB Java/javac heaps, 120 seconds per process and 16 MiB
per output stream. Existing native allowances remained 256 MiB, 100,000 rows and
100 million steps. There were no MAX/public UPDATE actions in this phase.

The executed source runner inherited an inaccurate API-count field,
`native_split_calls: 0`. Its exact commands are authoritative: **four real typed
split-facade no-op invocations**, zero split allocations and zero non-noop updates.
The raw report and executed source are unchanged. The maintained public wrapper
uses an actual `native_noop_attempts` counter instead of that inherited label.

## Action phase: thirteen processes

The separate frozen phase used the same unchanged originals and measured ID1
ordinals, with exact zero-to-MAX requests. It ran one javac, four prior-service
attempts, four fresh public UPDATE copies and four complete descriptions. The
prior artifact was the [committed typed consumer](results/ace16-fixed-split-example-build-2026-10-27.json),
compiled from `809c5e8d72ed1afe698f565f8447005292739b05`, SHA-256
`ff4adf2f0a3699bdeefdc64cd95daed54dda183f0f871861a14fce6f9a324418`.
All four attempts returned `Unsupported(IndexedColumn)`, with empty stdout,
no candidate file and unchanged input bytes.

The zero source stream occupies 3,609 bytes under key-prefix compression. MAX
loses that shared prefix: the whole stream needs 10,801 bytes, and one first
450-entry half still needs 5,401 bytes in a 3,616-byte region. A second split is
therefore necessary. This conservative capacity calculation establishes overflow,
not exact future candidate prefix bytes: full key/ROW sharing and original-versus-
fresh prefix seeds must be measured independently for an engineering golden.

The public updates actually allocate **six EOF pages per copy**, three for each
distinct affected physical index. Alias sharing causes no extra allocation.
Every original root ID is retained at height one with two full key-plus-row
bounds and a right tail; its prefix is zero and free space 3,582 bytes.
Let E be the original EOF page (140 or 158):

| Physical direction | Allocation order | Final leaf order and populations |
| --- | --- | --- |
| ASC | right E, left E+1, repeated-left E+2 | E+1:450, E+2:225, E:225 |
| DESC | right E+3, left E+4, repeated-left E+5 | E+5:225, E+4:225, E+3:450 |

Every 450-entry public leaf has prefix nine/free 1,807; every 225-entry public
leaf has prefix zero/free 691. Complete row-inclusive separators and reciprocal
leaf links were decoded independently. All 900 keys/ROWs per affected tree agree
with actual typed getters and public index orders. Original ROW references and
all other typed values, markers and preservation payloads remain unchanged.
Only the selected record's exact eight-byte field differs. All observed physical
historical counter bytes and getter entry counts remain unchanged; no unaffected
index page was rewritten.

Each physical owned map gains its three exact pages; global future-free bits
clear for E..E+5. No index retirement, table-owned/hint change or map representation
change was observed. Full map membership changes remain in the raw report.

## Framing and evidence limits

A separate read-only saved-copy classifier ran after the capture without repeating
any JVM/native action. All eight physical roots preserve the original suffix and
bytes outside the engineering header/bitmap/new-entry spans. **All 24 fresh public
leaf pages contain nonzero unused scratch bytes.** The public writer's scratch
is not the stronger candidate contract: an appended page split again still has
no original-image residue authority, and its final unused bytes must be zero.
Retained original pages must preserve their original residue.

The inspected public source repeats modified-page preparation until no split
occurs. The actual allocation order above is observed, not inferred from that
loop. This witness uses even 900/450 partitions. It does not qualify odd
ceil-versus-floor partition behavior, repeated internal-node splitting, deeper
child-tail conversion or peer-chain changes across new internal parents. The
[existing service](ACE16_FIXED_INDEX_SPLITS.md) retains explicit atomic refusal
when one of its balanced halves cannot fit. Native Access, macros, locking,
recovery and normal saves remain unqualified.

## Saved verification and fresh reproduction

The [read-only verifier](repeated_split_originals/verify.py) recomputes complete
source facts, typed/reference streams, maps and framing from saved descriptions
and database bytes. Its accepted run checked 269 recursive pin occurrences with
zero fixture processes. The multi-megabyte reports, getter stdout, class files,
originals and reference copies remain ignored and hash-pinned.

```sh
python3 docs/testing/repeated_split_originals/verify.py \
  --source-report .cache/ace16-repeated-split-sources-first-20261027/report.json \
  --action-report .cache/ace16-repeated-split-actions-first-20261027/report.json \
  --framing .cache/ace16-repeated-split-actions-first-20261027/framing.json \
  --source-postcheck .cache/ace16-repeated-split-sources-first-20261027/post-execution-pins.json \
  --action-postcheck .cache/ace16-repeated-split-actions-first-20261027/post-execution-pins.json
```

The maintained wrappers are distinct from the exact executed private sources,
which remain archived and pinned. The public producer, public updater and source
model are byte-identical promotions; path/import handling and count labels in
the public capture/proof tools are maintained derivatives. Their saved-fact
verification is not a claim that those wrappers performed the historical actions.

Fresh reproduction creates new identities and outputs; it never overwrites the
accepted corpus. The explicit source plan contains JAR, artifact/revision, helper
pins, limits and a fresh output directory. Review its inventory before execution:

```sh
python3 docs/testing/repeated_split_originals/capture_sources.py \
  --freeze docs/testing/repeated_split_originals/source-plan.json
python3 docs/testing/repeated_split_originals/capture_sources.py \
  --freeze docs/testing/repeated_split_originals/source-plan.json --execute
python3 docs/testing/repeated_split_originals/postcheck.py \
  --freeze docs/testing/repeated_split_originals/source-plan.json
python3 docs/testing/repeated_split_originals/bind_actions.py \
  --source-plan docs/testing/repeated_split_originals/source-plan.json \
  --source-report .cache/ace16-repeated-split-sources-reproduction-20261027/report.json \
  --source-postcheck .cache/ace16-repeated-split-sources-reproduction-20261027/post-execution-pins.json \
  --work .cache/ace16-repeated-split-actions-reproduction-20261027 \
  --output .cache/ace16-repeated-split-actions-reproduction-plan-20261027.json
python3 docs/testing/repeated_split_originals/capture_actions.py \
  --freeze .cache/ace16-repeated-split-actions-reproduction-plan-20261027.json
```

Only an explicit additional `--execute` on the final action command starts its
thirteen processes. Source/model/limit or geometry failures remain first outcomes;
there is no regeneration, fallback, selector change or automatic retry.
