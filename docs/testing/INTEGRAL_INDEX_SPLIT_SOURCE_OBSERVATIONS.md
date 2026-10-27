# ACE 16 integral and Currency split sources

The first bounded source-preparation run completed all 13 planned processes:
one Java compilation, two fresh public-builder generations, two read-only
metadata/getter captures and eight existing typed fixed-preview no-ops. Both
originals contain 2,729 user rows. Every no-op returned a byte-identical whole
image. No non-noop update, public UPDATE or split candidate ran in this phase.
The [compact receipt](results/ace16-integral-index-split-sources-2026-10-27.json)
pins the exact first report, commands, sources, outputs and separate postcheck.

These are new repository-owned Jackcess 5.0.4 comparison inputs, not restored
historical files or Microsoft Access qualification. Original files and accepted
helpers remain unchanged. The optional engine preview remains separate from
normal saves and journalable edits; see the [fixed-preview contract](ACE16_WRITE_PREVIEWS.md).

## Actual source profile

Each ACE 16 V2016/V2019 database contains four target tables, four small unique
control tables and one separate preserved Memo/OLE table. Target columns are ID,
KeyValue and an independent marker. ID and KeyValue store the actual Boolean
`Required=true` property. KeyValue indexes are required ordinary nonunique
indexes, with ascending and descending physical storage. The logical Alias
actually shares Ascending storage; it is not a third physical split target.

All eight target table/profile combinations passed the existing typed native
before/after gates through exact no-ops. Fixed column flags are 3, extended flags
are zero, the raw header decodes to encoding key zero and subversion zero.
Actual BIG_INT native type `0x13` works in both generated formats. No headers,
flags or unsupported-format gates were patched to obtain admission.

| Target | Rows | Payload width | Original leaf prefix / free bytes | Updated best bytes |
|---|---:|---:|---:|---:|
| Byte | 900 | 1 | 2 / 14 | 4,501 |
| Int16 | 900 | 2 | 3 / 13 | 5,401 |
| Int64 | 400 | 8 | 9 / 2,007 | 4,801 |
| Currency | 400 | 8 | 9 / 2,007 | 4,801 |

The measurements hold in both directions and both formats. Every selected
KeyValue root is a leaf. The “updated” sizes are independent capacity calculations
for the fixed, still-unexecuted ID1 zero-to-typed-maximum action; all exceed the
3,616-byte capacity and both calculated halves fit. They are not actual update
outcomes. Actual ID1 native ordinal is zero in each target; raw physical ROW
references, complete key/ROW streams and public index orders are captured.
Existing represented owner/global cells cover the required future allocations.

Byte is unsigned even though a Java Byte may print negatively. Currency values
use exact BigDecimal/scaled i64 units, without floating conversion. Int64 and
Currency remain distinct carriers. Duplicate zero populations provide genuine
compression-loss witnesses without inventing an impossible 900-key unique Byte
index. Unique controls have 32 rows each; no mutation of those controls is claimed.
The separate preserved table supplies one Memo/OLE row.

## Bounds and evidence

The producer enforces at most 4,096 explicitly inserted rows and 64 MiB per
original through a bounded channel. Actual files are 757,760 bytes for V2016 and
827,392 bytes for V2019. Java and javac use 512 MiB heaps. Native no-ops use the
existing frozen non-splitting `preview_ace16_fixed` artifact, compiled from
`452f2bde94a2580bd0a64e166cc11384f1ff8a4a`; its
[build receipt](results/ace16-integral-split-baseline-build-2026-10-27.json)
records the artifact and historical source binding. No source or allocation cap
was increased, and there were no retries, selector changes or fallback generation.

The original executed private helper/model/runner/freeze and precall archives
remain pinned. Before any fixture processes, peer review corrected a raw-zero
header assertion: the ACE encoding field is XOR-obfuscated, and the observed
`fb8abc4e` bytes decode to zero. The original assertion/freeze are retained.
Explicit heap arguments and a nontruncating BigDecimal guard were also fixed
before execution. These were static preparation corrections, not failures or
changes to database headers after generation.

A separate read-only postcheck rehashed all 20 frozen source/artifact pins and
verified the raw report hash was unchanged. It did not rewrite the report or
repeat fixture processes. Full raw getter/metadata, ROW spans, index streams,
maps and command outputs remain ignored and hashed by the compact receipt.

## Read-only verification and fresh reproduction

The public Java producer and model are byte-identical promotions of the executed
sources. The public capture/selfcheck wrappers adjust repository discovery and
use the separate fresh reproduction plan; they were maintained after execution.
They are not claimed as the historical command paths. JSON dictionary-key
normalization in the verifier only compares saved facts and does not alter them.

```sh
python3 docs/testing/integral_split_originals/verify_sources.py \
  --report .cache/ace16-integral-split-sources-first-20261027/report.json
python3 docs/testing/integral_split_originals/selfcheck.py
python3 docs/testing/integral_split_originals/capture_sources.py
```

The first command replays saved source facts, hashes, command associations and
whole-file no-op identity without JVM/native calls. The last command only checks
the current reproduction source/artifact/Git inventory. Explicit `--execute`
generates new identities in the plan's separate fresh directory; it never
regenerates or overwrites the accepted originals. That public reproduction has
not been executed as part of this evidence.

A separate action phase must retain actual old-service error classes and fresh
public UPDATE outcomes, including both index directions and actual historical
counter behavior. Source preparation and capacity arithmetic do not establish
split output compatibility or a new typed split API.

The subsequent [MAX-action reference phase](INTEGRAL_INDEX_SPLIT_OBSERVATIONS.md)
uses these exact unchanged sources. Its public updates and old-consumer refusals
are separate from this source/no-op phase and from new typed candidate acceptance.
