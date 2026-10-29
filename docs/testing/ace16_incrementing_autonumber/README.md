# Original-file incrementing AutoNumber INSERT proof

Eight immutable public-builder originals pass independent engineering INSERT and
fresh inert reader checks. Both ACE 16 layouts cover an empty table, removal of
the highest generated ID, removal of all rows, and a fixed index-pressure source.
All eight native candidates match the earlier SOURCE/request-derived images byte
for byte: 3,571,712 bytes in total, with 20 generated rows.

This is explicit caller-selected signed Int32 increment behavior. It does not
detect a stored Increment/Random mode, qualify Random generation or native Access
save/reopen, or authorize normal writes, journal application or data macros.
The public originals use Jackcess's default positive sequence; negative/MIN,
exhaustion and stale-counter engineering controls are separate literal tests.

## Source and counter authority

Items declares Earlier first, AutoNumber ID second and nullable Score third.
The genuine required primary and its ordinary alias share physical owner 0;
ScoreOrder and ScoreAlias share owner 1. Every raw field flag, fixed offset,
property, key/ROW stream, alias, historical counter, map and catalog role is
qualified from the immutable original. ID is native column 1 rather than the
first Int32 field. Requests supply only Earlier and Score in declaration order.

The stored signed counter at table-definition offset 20 is the last issued ID.
Read-only getLast getters must agree with it; inspection never calls a generator.
The complex counter at offset 28 and opaque bytes 24..28 remain unchanged. The
stored New Values mode stays unclassified even when properties are absent.

The fixed recipes produce these actual transitions in each layout:

| State | Original live IDs | Last issued | New IDs | Final Items rows |
| --- | --- | --- | --- | --- |
| Fresh | None | 0 | 1, 2 | 2 |
| Highest deleted | 1, 2 | 3 | 4, 5 | 4 |
| All deleted | None | 3 | 4, 5 | 2 |
| Pressure | 1..599 | 600 | 601..604 | 603 |

Thus an empty or highest-deleted population does not reset generation to the
maximum surviving ID. Primary history grows by all 20 new distinct IDs across
the corpus. Score history grows from 12 to 26: per layout, fresh adds 20/30,
highest-deleted adds 30, all-deleted adds 20/30, and pressure adds 30/NULL to its
original 10/20 families. Logical aliases do not duplicate these increments.

Fresh tables allocate one DATA page followed by their first ordinary-owned bitmap
(V2016 pages 97/98; V2019 pages 114/115). Highest/all-deleted cases fit existing
DATA and index storage. Pressure sources have 600 physical records, including
the deleted ID 600, and a primary leaf with prefix 3/free 19. Four requests split
both physical roots, allocating four INDEX pages in owner order. The independent
engineering image uses new RIGHT then LEFT pages, floor halves, fresh prefix zero
and zero fresh unused bytes. All original live and deleted bodies, references,
properties, allocation roles, historical storage and Sentinel remain protected
outside the independently derived patch ranges.

## Public observations remain separate

Six public images are byte-exact with the prefrozen engineering images. Each
pressure image differs in 3,152 bytes. The public writer uses left/right leaf
populations 302/301 while the engineering policy uses 301/302, with corresponding
bound/framing differences and nonzero unused bytes in fresh leaves. Five differing
bytes are in the two root bounds. One byte is in new ID 604's inactive Score
payload: public bytes hold 30 from the preceding record while its presence bit
is clear; the engineering image stores zero. No source tombstone reference is
reused. Maps, counters, complete typed rows and full key/ROW membership agree.

These comparisons are observations after successful public typed proofs. They
never feed the constructor, tune expected bytes or establish a general native
split/scratch policy. All eight native candidates retain the earlier exact
engineering policy, followed by eight separately bound fresh typed-row/map reads.

## Phases and retained preparation history

SOURCE17 is one compile, eight public creations and eight inert inspections.
Capture leaves qualification false. Eight separate saved raw gates complete
before any expected construction, build or public append. Eight complete images
are frozen before REF16, the producer build and native calls. REF16 comprises
eight fresh-copy public batches and eight inert inspections. Native8 must finish
all whole-image and generated-result checks before fresh8 can be bound; every
image is rechecked before and after each fresh reader. Every actual source,
public-reference, native and fresh-reader phase passes on its first execution.

The first pure constructor wrote three images before a bytearray-versus-bytes raw
reader boundary failed in the pressure case. A separately frozen derivative only
converts that decoder argument to bytes; its completed eight-image run preserves
the first three files exactly. No source, record, key or allocation decision was
changed from producer or public output. An overbroad freeze-authoring filename
guard also rejected a legacy raw-decoder module named model.py, followed by a
static missing-freeze check; both happened before reference calls.

Before any fresh reader ran, peer review caught an unchanged public GENERATED
stream validator receiving CLI output. The original native protocol and all eight
successful candidates remain immutable. A separately frozen reader derivative
validates the actual CLI changed-row count, column 1 and request-order generated
values; it ran eight readers once, without native reruns. A later saved facts
comparison needed ordinary JSON normalization of integer suffix-role keys; no
image or proof changes followed.

The retained producer was compiled from committed
`9e77ffcf5612734ec89e342aa304e17cb60be8fd` in an initially absent isolated target.
The archive's 4,702 file contents match its source manifest and Git blob IDs;
actual engine compilation and the copied executable are pinned by the
[build receipt](../results/ace16-incrementing-autonumber-example-build-2026-10-28.json).
Disposable target contents are not evidence dependencies. The
[result](../results/ace16-incrementing-autonumber-originals-2026-10-28.json)
keeps each phase, preparation correction and public byte difference separate.

## Saved verification

```sh
python3 -B docs/testing/ace16_incrementing_autonumber/verify.py
python3 -B docs/testing/ace16_incrementing_autonumber/verify.py --verify-git
```

Default replay starts no subprocess, loads no cached Python implementation and
runs no Java, Cargo or producer. Maintained source/model/reference bodies match
the executed owners with bootstrap-only changes. It reconstructs all eight
expected images before reading any AFTER, checks the archive/build/artifact,
then replays public and fresh raw proofs, native CLI results and complete bytes.
Optional Git mode starts one metadata-only batch checking all 4,702 source blobs
and the committed receipt. Missing private evidence returns available:false and
status 2; contradictory evidence returns status 1. Database binaries, classes,
full transcripts, archives and failed attempts stay ignored and must be restored
at their recorded relative paths for replay. Newly generated originals need not
be byte-reproducible with these immutable captured files.

The first saved default replay verifies 2,860 pin occurrences over 384 distinct
paths. Independent peer replay also passes with subprocess.run and Popen blocked;
optional Git mode passes with exactly one metadata child and checks all 4,702
source blobs plus the committed receipt. Explicit missing-producer and missing-
getter controls each return unavailable/status 2. These saved checks execute no
Java, Cargo or native candidate, and do not change any expected image.
