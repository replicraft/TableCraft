# Enlarged ordinary-hint source observations

Four enlarged copies of the two accepted
[variable DATA/index pressure originals](VARIABLE_ENCODED_SPLIT_SOURCE_OBSERVATIONS.md)
are qualified as sources for later experiments. They retain the original target
records and indexes, add exactly 384 or 700 passive wide padding rows, and expose
ordinary candidate-map pressure beyond the old inline window. No public UPDATE,
native preview or changed target key belongs to this phase. The transition
classifications below are source-policy predictions, not observed mutation
results or Microsoft Access qualification.

## Actual copied sources

Each copy retains the original 2,701 user rows, including the three 900-row
targets and the one-row Memo/OLE sentinel. `PaddingHint` adds a manual LONG ID
and seven uncompressed TEXT255 fields, each storing U+0100 repeated 255 times.
It has no index or AutoNumber. Its records have eight columns, seven variables,
one mask byte and exactly 3,595 physical bytes. The independently decoded
variable offsets run from 6 through 3,576 in 510-byte steps. Each padding DATA
page contains one record at offset 501 and has 485 free bytes.

| Source | Bytes | Pages | User rows | SHA-256 |
| --- | ---: | ---: | ---: | --- |
| V2016 / 384 | 2,199,552 | 537 | 3,085 | `f827703094960ea70f6c03c03c1fcd936b049b50b8488ca04f5cb49a7fe5baac` |
| V2016 / 700 | 3,493,888 | 853 | 3,401 | `239c6987531a878f2c0ed6ddc5f50403070c24ee5a297197407e7d792ce662a6` |
| V2019 / 384 | 2,273,280 | 555 | 3,085 | `c2c05bdd1918690df58fa29a861e8c41269c02e280ee7cf65e410354d9b54eeb` |
| V2019 / 700 | 3,567,616 | 871 | 3,401 | `457870f9f628fb6a5ef5c305f6d50298d7dfe2276d73f6999b7f02b12e0285a1` |

The original target schema, fixed GUID/BYTE companions, General descriptors,
selected field flags and genuine Ascending/Twin physical aliases remain exact.
All 2,700 target records per copy remain 43 bytes, distributed across 30 DATA
pages with 90 roots and 32 free bytes per page. Logical roots and final physical
locations are captured independently and agree; these copies introduce no
forwarding or hidden target records. BYTE getters use unsigned `Number`
normalization, corroborated by raw bytes and metadata; they do not establish a
specific concrete Java carrier class.

Every copy has five user tables, 28 captured user/global map roles, 13 logical
indexes and ten physical indexes. The padding table contributes two map roles
and no index. These 28 roles are not the complete catalog/current/historical map
authority census required for a native engineering operation. Source inspection
does not establish that a future native request is admitted.

## Natural and deliberately controlled hints

`TextLegacy` retains its natural highest-only candidate hint. For `TextModern`
and `BinaryOne`, copied-source preparation explicitly invokes public
`UsageMap.addPageNumber` under the page channel's write scope to add the lowest
already-owned DATA page. It retains the existing highest hint. These eight
metadata additions across four copies are controlled low-level preparations,
not effects of ordinary row updates or observations of Microsoft Access policy.
No flags, headers, payloads or existing map extents are forced or normalized.

| Profile | TextLegacy hint | TextModern hint | BinaryOne hint |
| --- | --- | --- | --- |
| V2016, either padding count | `[106]` | `[112,124]` | `[130,142]` |
| V2019, either padding count | `[123]` | `[129,141]` | `[148,160]` |

All these source hints remain inline with base zero. Their ordinary owned maps
are indirect. The getter and raw bitmap decoder agree on every membership;
the only permitted changes to original non-global map rows are the two explicit
lower-hint bits per copy.

The saved proof compares complete original user ordinary/LVAL-owned pages and
all old non-global indirect bitmap pages. Their deduplicated inventory is 36
pages, or 147,456 bytes, per copy. This protects opaque DATA/LVAL gaps and owned
bitmap future bits as well as live payloads. Original table definitions and
index pages are also compared in full. Map containers use exact per-row checks
because new padding metadata may occupy distinct rows in an existing container;
the global free map may change for actual padding/table allocation. This is not
a byte oracle for every new system or padding metadata page.

## Predicted pressure, without target writes

The unchanged max255 requests would require 547-byte Text or 297-byte Binary
records plus a two-byte slot. Their highest source hints have only 32 free
bytes, so the capacity failure is decisive even under a hypothetical reclaim of
the 39 bytes following a root pointer. No such failure or hint clear was executed
in this source phase.

The source model classifies each target independently from its own immutable
copy. It removes only the hypothetical failed highest hint, places a hypothetical
DATA page at EOF, and records three hypothetical child reservations for each
affected physical index. It does not assert an actual public allocation order,
split partition or future engineering output image.

An addition inside the current inline window retains its current base. Only an
out-of-window addition tests a new base aligned down to eight pages, and it
remains inline only when the aligned inclusive span is **strictly less than
512**. These rules follow the pinned independent writer's `UsageMap` source;
they are not inferred from an unaligned span or a failed cell lookup alone.

| Padding | Natural Legacy hint prediction | Controlled Modern/Binary hint prediction | Affected index-owned map predictions |
| --- | --- | --- | --- |
| 384, both profiles | Empty hint rebases around EOF | Rebase while retaining the lower hint | All considered child additions already represented |
| 700, both profiles | Empty hint rebases around EOF | Indirect promotion retains the lower hint | Both physical maps per target require promotion |

The 384 cases therefore do not provide index-rebase observations. The 700 cases
predict candidate-map and index-map promotion together, but no UPDATE has yet
established those outcomes on these enlarged sources. Existing actual encoder
wires, digest predictions and old persisted keys are rechecked unchanged; the
capacity calculation remains the public key-only prefix model, separate from
engineering full-key/ROW packing.

## Execution and preserved correction history

The original nine-command phase stopped after its first three fixture commands:
javac, V2016/384 preparation and its inert inspection. All three returned zero.
Python proof assembly then failed because current table dictionaries contained
integer row keys while saved JSON facts contained string row keys. The original
phase exited unsuccessfully, and its raw report, inputs and completed database
were retained. This was not an observed database mutation failure.

A separate `measure_json.py` derivative changes only that equality boundary by
JSON-normalizing the current table dictionary. The original already imported
`json`; no source values, raw field flags, parser, framing or expected memberships
changed. A saved-only check qualified the completed V2016/384 database and its
cached getter: all 3,085 rows, 28 roles and 36 protected pages passed. That source
was never regenerated or inspected by another fixture call.

A separately frozen continuation then ran exactly the remaining six original
argv: prepare/inspect V2016/700, V2019/384 and V2019/700. All six succeeded on
their first execution. It reused the compiled classes and unchanged helper/model;
there was no additional javac, retry or adjusted geometry. The complete history
is **three successful initial fixture commands, a failed Python proof assembly,
a saved-only correction, then six successful unique continuation commands**.
The nine unique fixture commands must not be described as an uninterrupted
first-attempt phase pass.

Before any fixture process, independent review strengthened the original source
proof to include full original DATA/LVAL/bitmap-page comparisons. The unexecuted
earlier freeze and source bytes remain archived. Only the measurement/protocol
inputs changed; Java, populations, values, limits and all nine argv stayed exact.
Pure checks on the two accepted originals verify literal 36-page inventories and
reject in-memory mutations of ordinary gaps, LVAL gaps and owned bitmap future
bits. No database operation belongs to those checks.

All children retained 512 MiB heaps, 30-second deadlines, an 8 MiB writable file
ceiling and live 16 MiB limits per stream. Expressions were disabled, foreign
keys enabled, free-page reuse disabled, UTC selected and linked opens refused.
The continuation's 76 input/freeze checks and three created-source pins stayed
unchanged; its separate post-terminal check rehashed 173 saved identities with
zero mismatches. The original 26 input paths and first report remain unchanged.

## Maintained saved verification

The [helper](ordinary_hint_growth_sources/OrdinaryHintProbe.java) is byte-identical
to compiled source. The [source model](ordinary_hint_growth_sources/model.py)
and corrected [measurement](ordinary_hint_growth_sources/measure.py) change only
repository-root composition from their frozen private counterparts. The
[verifier](ordinary_hint_growth_sources/verify.py) preserves the exact one-line
transport correction, both phase histories and preparation archives, then
rederives all four sources from saved database/getter bytes without a child
process. Its first run passed 450 pin occurrences representing 182 unique saved
paths, 12,972 typed rows, 112 map-role instances, 10,800 target slots, 43,204 logical
index entries including aliases and 144 complete original-page comparisons
(589,824 bytes).

```sh
python3 -B docs/testing/ordinary_hint_growth_sources/verify.py
```

The [compact receipt](results/ordinary-hint-growth-source-observations-2026-10-27.json)
pins raw reports, frozen sources, corrected proof and saved checks. Jackcess
5.0.4 binary/source provenance follows the [fixture ledger](FIXTURES.md); the
helper is repository-authored. Later public UPDATE references, native no-op/
refusal controls and committed-consumer candidates belong to separate phases
and records. This evidence does not qualify map growth, native saves, macros,
foreign-key updates, arbitrary page reuse or complete ACE 16 compatibility.
