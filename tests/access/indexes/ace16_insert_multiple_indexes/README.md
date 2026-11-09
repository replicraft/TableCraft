# Saved multi-index INSERT reference observations

This package verifies four original synthetic Jackcess source/reference cases:
ACE V2016 and V2019, each with an ordinary three-row source and a 400-row source
that pressures both physical root leaves. These are independent public-writer
observations. They do not establish native Access behavior, ordinary TableCraft
writes or byte identity with a Rust engineering candidate.

Each table has explicit manual `ID` Int32, nullable `Group` Int32 and nullable
Short Binary `Score`. Four logical indexes resolve to exactly two physical
owners: required unique ascending `PrimaryID` and its alias, plus ordinary
ascending `GroupOrder` and its alias. Both physical indexes retain complete
key/ROW membership. A NULL secondary key is represented and participates as one
key family; it is not an omitted entry or a NULL row reference.

The ordinary batch inserts four rows, including an existing secondary key, a new
key repeated within the batch and another NULL. Historical primary counters
increase by four; secondary counters increase by one. The pressure batch inserts
52 rows into each 400-row source. Both physical roots grow from leaves to
height-one trees, each with two leaves. Historical counters increase by 52 for
both owners. All 806 original logical ROWs and raw bodies across the four cases
remain unchanged; 112 rows are added. Historical counters are never normalized
to current live row counts by this proof.

The V2016 pressure reference allocates DATA page 96 and INDEX pages 97–100; V2019
allocates DATA 114 and INDEX 115–118. Ordinary cases fit existing DATA and index
pages. Exact source-derived records, schema, declarations, true alias mappings,
complete key/ROW streams, per-owner map membership, aggregate global free bits
and protected original bytes are rechecked. Only independently validated old
INDEX pages may have public-writer scratch rewritten; their unused bytes and
prefix choices are observations, not engineering image policy. No index or
bitmap page may become ordinary DATA ownership.

## Preserved first failure and continuation

The first controller ran seven children successfully: compilation and the first
two source generation/getter/map triplets. Its Python gate then rejected the
pressure primary root's prefix 0, having incorrectly required prefix 1. The
400-entry root actually used 3,600 active bytes and fit without compression;
its full key/ROW common prefix 1 permits a minimum 3,201 bytes. Final 452-entry
minimum 3,617 bytes still exceeds 3,616 and preserves the intended overflow case.
The secondary source has one five-byte NULL entry plus 399 nine-byte entries,
using 3,596 bytes. No database bytes, keys or source recipe were changed.

A separately frozen gate derives actual active/free bytes and checks the stored
prefix against the complete wire common prefix. It first admitted both saved
V2016 sources without starting any child. A new controller then executed only
the eighteen unattempted calls. All four source gates passed before any reference
append. All eighteen children and all four reference transition proofs passed.
Together, the original seven and the continuation eighteen are the original
25 distinct calls, each attempted once. The original controller remains failed;
there is no fabricated combined successful raw report.

An earlier unexecuted gate draft also assumed zero bytes behind a NULL fixed
field. It was archived before the first child call. The executed gate instead
uses the mask and preserves opaque NULL payload bytes, while checking every
present field, variable footer and getter projection.

## Saved replay

From the repository root:

```sh
python3 -B docs/testing/ace16_insert_multiple_indexes/verify.py
```

The default verifier starts no child process. It uses `scripts.compat_artifacts`
for fresh root-scoped artifact reads, checks fixed report/freeze hashes and every
recorded input/output pin, validates all 25 exact argument vectors, statuses and
bounds, and replays all four BEFORE gates before examining any AFTER transition.
Missing ignored-cache evidence returns JSON with `available:false` and exit 2;
there is no source generation, retry or hash fallback.

`reference.py` is the exact executed continuation proof with only the repository
root depth changed for its maintained location. `verify.py` checks that textual
identity on every replay. `MultiIndexInsertReference.java` is byte-identical to
the executed original helper; it calls the pinned Jackcess API and contains no
copied third-party implementation. Its bounded-channel dependency is the already
maintained [BoundedFileChannel](../parent_index_split_originals/BoundedFileChannel.java),
whose bytes equal the executed helper dependency. Existing
[SlotUpdateFixture](../SlotUpdateFixture.java) and
[SlotMapProbe](../SlotMapProbe.java) supplied read-only observations from pinned
compiled classes. The verifier does not invoke these Java tools.

Recorded child bounds were 25 then 18 maximum processes, 30 seconds per child,
512 MiB Java heap, 16 MiB per captured stream and 64 MiB per database channel.
A later proof replay starts zero Java/native processes and does not grant
permission to rerun the historical protocol.

See the [compact reference receipt](../results/ace16-insert-multiple-index-references-2026-10-28.json)
for source/reference hashes, exact lineage and maintained-tool pins. Private
source files and captured streams remain ignored artifacts; absence does not
become passing evidence. Rust candidate acceptance and native Access save/reopen
qualification remain separate.

## Separate engineering candidate qualification

Four candidates now also pass complete byte comparisons against an independent
SOURCE+REQUEST model, followed by eight fresh read-only Jackcess getter/map calls.
This second phase is separate from the public-writer references above. The model
never reads a public AFTER image to construct expected bytes.

All four source gates completed before construction. The model derives ordered
records, logical ROW placement, DATA free words/directories, physical key/ROW
streams, distinct historical counter deltas and aggregate map-byte changes from
the unchanged originals and explicit requests. It preserves existing root suffix
bytes and uses zero-filled fresh index pages. DATA precedes physical indexes in
ascending data-number order; a split allocates RIGHT before LEFT. Source native
prefixes remain captured source facts. Four exact images and their input closure
were frozen before the isolated committed-source producer build.

Each candidate matched its complete expected image before any fresh getter ran.
Only then did all eight read-only getter/map calls execute. Actual typed values,
all original ROWs and raw records, inserted ROWs, both index graphs, alias
metadata, counters, maps and protected bytes passed independent reinspection.
There are 918 final target rows and 1,836 physical key/ROW pairs across the four
cases. Logical aliases do not duplicate physical maintenance or counter deltas.
All originals, public references, model inputs and candidate bytes stayed unchanged.
These observations do not qualify ordinary saves, Access application execution
or arbitrary multi-index profiles.

Run the complete saved candidate proof with:

```sh
python3 -B docs/testing/ace16_insert_multiple_indexes/verify_candidates.py
```

The default starts no child process. It reconstructs all four engineering images
before replaying public references or fresh candidate getter facts. The public
`model.py` is exactly the frozen private model except for repository-root depth;
text and function-AST identity checks enforce that transformation. The existing
`verify.py` remains the actual owner of reference-only verification and is called
directly, rather than copying its proof into another model.

Optional `--verify-git` starts exactly one bounded Git metadata batch to verify
all 4,433 regular files from the committed source archive and the separate build
receipt blob. It starts no Java/native fixture process. Default verification
checks the pinned archive, artifact, receipt and recorded binding without a Git
child. Missing private evidence returns `available:false` and exit 2.

Two prospective pure preparation failures are preserved separately: an omitted
repository-root Python import path, then a bytearray passed to a set-based model
self-check. The first produced no images; the second left one complete ordinary
image. After correcting only the import and immutable-buffer handoff, a fresh
namespace produced all four images; the earlier ordinary image and facts remain
byte-identical. Neither failure involved a producer or fixture process, and no
expected bytes were adapted to a candidate. Earlier unexecuted controller drafts
and the adaptation to the actual committed-archive build receipt are also retained.

The first four producer calls and first eight fresh read-only calls all succeeded;
there were no native/getter retries. Their input closure and first reports are
immutable. The older reference receipt's tool hashes describe its initial
promotion; its prior README is archived before this candidate section was added.
The [candidate receipt](../results/ace16-insert-multiple-index-native-2026-10-28.json)
records this later scope without rewriting the original reference history.
