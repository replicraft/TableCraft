# Original-file ignored-NULL fixed UPDATE comparisons

This package replays retained engineering evidence for the committed
`60b33158937b4fe66c6daa050c0d37f11f33ddeb` fixed/batch UPDATE consumers.
Sixteen candidates match independent SOURCE/request images exactly, covering
9,076,736 bytes. Eighteen additional calls return their precise expected errors
without creating output. Sixteen fresh inert readers independently verify the
successful images. This is an independent Jackcess comparison, not Microsoft
Access execution, production save, journal, data-macro or Windows qualification.

## Originals and requests

Two genuine public Jackcess creations supply V2016 and V2019 originals. Each has
six target tables plus Sentinel: Single (4 rows), Tuple (6), UniqueSingle (4),
UniqueTuple (5), RequiredSingle (2), RequiredTuple (2), and Sentinel (1). That is
23 user rows plus one Sentinel per layout, not 24 user rows. Source gates check
all 12 target tables and both Sentinels, complete raw columns, ROWs, two physical
indexes/four logical aliases per target, ownership/maps and catalog LVAL roles.
All selected fields are ordinary fixed Int32; the tuple is Right DESC then Left
ASC. Source fields and declaration flags are not patched to qualify a profile.

The public [plan](plan.json) fixes 17 actions per layout. Eight are admitted by
this engine profile: omitted to new/existing scalar families, represented scalar
to NULL, and five complete/partial tuple transitions. Four ambiguous history-order
actions, two UNIQUE duplicates and three Required controls per layout are engine
refusals. Requests retain actual original logical ROW references, source live-row
ordinals and original expected values. Tuple assignments use the existing batch
example's repeated `--set` argument groups. The single-cell example uses its
existing positional arguments. Neither consumer reads a TSV file or enables
cascades for this study.

## History and contrary public outcomes

The public writer removes and adds keys sequentially. A represented replacement
does not increment history; an omitted row introducing a family absent at that
written step can. The engine applies original-association batch rules and refuses
ambiguous removed/reintroduced or jointly introduced families.

| Original Single action | Public history, both layouts | Engine outcome |
| --- | --- | --- |
| Omitted row introduces 30 | 2 → 3 | Exact candidate |
| Omitted row joins 10 | 2 → 2 | Exact candidate |
| Remove original 10 members, then insert omitted 10 | 2 → 3 | IndexedColumn refusal |
| Insert omitted 10, then remove original members | 2 → 2 | IndexedColumn refusal |
| Represented replacement introduces 30, then omitted row joins | 2 → 2 | IndexedColumn refusal |
| Omitted row introduces 30, then represented replacement joins | 2 → 3 | IndexedColumn refusal |

Both writers refuse represented UNIQUE duplicates, including partial-NULL tuples.
The public writer accepts an entirely NULL Required+IgnoreNULL key because its
omission check precedes Required; the engine intentionally returns NullKey for
both single and tuple controls. Both refuse a Required partial-NULL tuple.
These observations do not change the engine's Required-before-omission contract.

Across the 16 supported comparisons, ten complete images equal the public images.
Six differ in eight bytes total, all behind cleared NULL presence bits. Per
layout the differences are one byte for single-present-null, one for
tuple-null-partial and two for tuple-partial-omitted. The engine retains ignored
payload bytes, including a NULL-to-NULL companion; the public writer clears those
payload bytes. All selected secondary-root bytes match. Complete typed proofs
pass on both sides. No source, expected image or producer was tuned to AFTER bytes.

## Phase barriers and preserved attempts

SOURCE5 is one compile, two public creations and two inert inspections. Capture
leaves qualification false. Separate saved raw gates complete before construction.
The constructor is frozen against archived committed semantics and SOURCE/request
inputs. All 16 complete expected images and 18 refusal classifications are frozen
before public AFTER, producer build or native calls.

REF68 is 34 fresh public actions and 34 inert inspections, containing 46 explicitly
ordered row-update calls. The first two child calls succeeded; a Python checker
then rejected the redundant whole-definition digest because it included the
correct historical counter change. The original failed report, output and streams
remain immutable. A separately frozen correction first proves the complete table
definition equals the original plus the independently predicted four-byte history
patch, then exempts only that redundant digest. Saved replay qualifies the first
transition. The continuation executes exactly the remaining 66 original commands,
without repeating either first call or regenerating sources. All 68 child statuses
are zero; expected public constraint exceptions are captured by the helper.

Before SOURCE5 ran, source review corrected an empty-stderr assumption. Five known
omitted-key warnings per layout are required in exact table order, with fixed
logger/context/alias grammar and literal null oldEntry. The original unexecuted
freeze remains retained. Compile and inert inspect still require empty stderr.
Public actions have their own exact old-entry/key/ROW warning checks.

One fresh isolated release target builds both existing examples from the complete
4,727-file archive of commit 60b33158, verifying every Git blob and SHA-256 before
and after compilation. The copied artifacts and receipt are retained outside the
target. Native34 executes all 16 admitted candidates first, then 18 exact refusal
controls. Each refusal must exit 1, print no stdout, preserve sources and create
no output; it is not an unexpected controller failure. All images and streams are
bound before the separate fresh16 reader phase, with every image rechecked before
and after each reader. Source images remain unchanged throughout.

During maintained-package authoring, an initial copy looked for a local
BoundedFileChannel.java and raised FileNotFoundError before verifier execution.
The actual source compile uses the existing shared
[parent-index fixture helper](../parent_index_split_originals/BoundedFileChannel.java).
No standalone exception log is claimed and no evidence bytes changed. The first
maintained replay and its code freeze remain retained separately from the later
observational NULL-payload comparison check.

## Saved replay

Run from the repository root:

```sh
python3 -B docs/testing/ace16_fixed_ignore_null_updates/verify.py
python3 -B docs/testing/ace16_fixed_ignore_null_updates/verify.py --verify-git
```

Default replay starts no subprocess, executes no cached Python, and imports only
maintained proof owners. It independently reconstructs all 16 SOURCE/request images
before reading AFTER images, verifies all public/native/fresh envelopes and typed
proofs, checks exact nonzero refusal results, and validates the committed archive.
Source, model and reference function bodies retain their executed identities;
only repository-root/import bootstrap changes are permitted. Optional Git uses
one metadata-only batch for all 4,727 source blobs and the receipt at commit
`6496fac1cb7c7d73d07d4f6502eea592456d76d5`.

Private retained evidence is required. Missing evidence returns status 2;
contradictory evidence returns 1. The default does not create fixtures, build a
producer, invoke Java/native code, install dependencies or write databases.
The [result](../results/ace16-fixed-ignore-null-update-originals-2026-10-29.json)
records the phase pins and saved replay checks. The locally retained Jackcess
5.0.4 artifact follows existing [fixture provenance](../FIXTURES.md); no JAR or
original database is bundled here.
