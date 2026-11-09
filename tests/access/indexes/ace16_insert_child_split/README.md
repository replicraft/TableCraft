# Saved ACE 16 INSERT child-split evidence

```sh
python3 -B docs/testing/ace16_insert_child_split/verify.py
python3 -B docs/testing/ace16_insert_child_split/verify.py --verify-git
```

The [result receipt](../results/ace16-insert-child-splits-native-2026-10-27.json)
records the actual reference25, first producer4 and separate first fresh getter8
phases. All four native candidates matched their pre-frozen complete images;
all eight fresh read-only calls matched the saved semantic getter/map streams.
No fixture process is repeated by the maintained verifier.

`reference.py` maintains the raw proof used in the first public Jackcess
child-split experiment. It provides saved-byte inspection and transition checks;
it starts no compiler, database process, native candidate or JVM. It imports the
existing owning `ace16_insert_root_split.reference` module directly. Private
artifacts remain necessary to replay the observations; this package does not
supply or regenerate databases.

The actual reference phase completed 25 commands: one compilation, four fresh
source preparations, eight before getter/map calls, four final INSERT calls and
eight after getter/map calls. Every prepared source passed before the first
final INSERT. Preparation copied the preserved public 452-row sources and added
225 explicit far-positive IDs through the public builder. No original was
rewritten or replaced by a later ID227 output.

Each admitted source had 677 rows, one height-one root, one complete separator
and two leaves. The left leaf contained 226 entries; the right contained 451,
with prefix one, 3,609 active bytes and seven free bytes. Inserting ID227 made
the right partition require 3,617 bytes. All four actual references retained the
root and old right leaf, inserted one fresh left leaf, and produced three
226-entry leaves under two complete bounds and the original right tail.

The final row fitted the existing highest hinted DATA page in every case.
Only one INDEX page was appended; there was no new DATA or bitmap page. The
ordinary owned and candidate memberships stayed unchanged. One global free bit
cleared and one index-owned bit set. Both the table count and historical physical
primary counter increased from 677 to 678. The proof preserves all 677 old rows
and logical ROW references, exact typed/raw record contents, sibling ordering,
map-byte unions and the original bytes outside explicitly qualified effects.

These public writer bytes are observations, not engineering goldens. All public
retained right leaves reset their prefix from one to zero. Both rewritten right
leaves and fresh left leaves had 2,034 active bytes and 1,582 free bytes. Their
unused suffixes contained nonzero reused scratch, and the parent scratch also
changed. The engineering policy retains a valid original prefix seed and old
residue, and initializes fresh padding to zero; it must be checked independently
against original requests and sources.

The maintained proof differs from the executed private proof only in its
repository bootstrap and direct owning-module import. Calls formerly named
`H.*` now name `root_reference.*`; their arguments and all proof logic are unchanged. The contextual import name
avoids shadowing the existing local ROW variable named `reference`.
The promotion check verifies this exact textual transformation and all function
ASTs, then replays the four saved source and reference proofs and compares their
transition records. No public UPDATE, INSERT, getter or fixture is repeated.

The subsequent engineering phase uses `model.py`, the independent source/request
constructor frozen before the committed consumer build and actual candidate
calls. Its maintained copy changes only the repository-root bootstrap. Complete
models are rederived before any reference-after or candidate getter content is
interpreted. Integrity pin checks may read stream bytes earlier; those bytes are
never constructor inputs. Actual candidate equality is required before its fresh
getter proof, including every compression, residue and zero-padding byte.

The native retained RIGHT leaves keep prefix one: 226 entries occupy 1,809 bytes,
with 1,807 bytes of exact original residue retained after active framing. New LEFT
leaves use prefix zero and 1,582 zero unused bytes each. The root keeps its original
unused residue after two full bounds. The peer changes only its next link. These
bytes differ from public writer framing/scratch by 22,030 bytes across the four
images, while all complete key/ROW entries, records, maps and counters agree.
The public bytes do not replace the engineering goldens.

All four candidates contain 678 rows, preserving all 2,708 original ROWs and
2,712 final leaf key/ROW pairs in total. Each adds one INDEX page and no DATA or
bitmap page; ordinary owned and hint memberships are unchanged. Eight map-byte
effects and eight counter-field increments are checked. Actual requests cover
right-child overflow with two empty Binary and two 84-byte Binary values. Left
child, physical aliases, peer-fit bulk and DATA-before-INDEX EOF cases are separate
literal tests, not additional authentic-source outcomes.

The tool uses `ArtifactStore` directly and rehashes every checked identity at
completion. Its private corpus includes original/prepared files, plans, classes,
JARs, streams and reports under `.cache/`. Missing evidence returns
`available: false` and exits 2, using portable labels. Contradictions exit 1;
complete verification exits 0. A checkout without that corpus is not passing
compatibility evidence. Default verification starts no child. `--verify-git`
uses one local `git cat-file --batch` process for 753 source blobs and one receipt
blob. Neither mode starts Java, Cargo, generators, public INSERT or native
candidate calls.

Compiled revision is `f5168aa65085fb76cfbb979f2d7c1b8f7a8a52d7`, with receipt
revision `b683fb1efacb73729bfbffba070d34b9e6cbb0e4`. Exact archived dependency-info
bytes are pinned by the build receipt; a disposable target directory is not
required. Actual source/report/controller/model bytes remain unchanged. An initial
private verifier preflight expected a per-case request field absent from this
source protocol; that lookup alone was corrected before actual candidate proof.
The private actual proof and maintained default/Git proof passed on their first
actual runs; no database, request or golden was adjusted.

The separate reference-helper promotion preserved two earlier authoring
failures: a module-name collision and a checker bootstrap-literal mismatch.
The corrected helper replay passed without changing executed inputs. These
outcomes remain distinct from the first successful full-verifier runs.

This qualifies four engineering candidates and independent Jackcess observations.
Microsoft Access save/reopen, broader trees, two overflowing children, repeated
splits, generated values, required data macros and normal native saves remain
unqualified by these cases.
