# ACE 16 ignored-NULL index UPDATE previews

Status: implemented engineering continuation. The final 16 focused checks and
108 shared regressions across 16 targets pass. Strict all-target/all-feature and
minimal-feature Clippy checks pass. The earlier full engine library passed 657
tests with two existing ignored tests before the final state-only Population
refactor and stricter primary-group guard. Independent literal source/expected
read gates precede the preview checks; they are not Microsoft Access comparisons
or evidence of a native save.

These results describe the committed `60b3315` continuation. The separate
[selected forwarded-record continuation](ACE16_SELECTED_FORWARDED_UPDATES.md)
has separately passed its ownership, preview and regression verification; the
earlier counts do not qualify that additional storage path. The
[original-file proof](ace16_fixed_ignore_null_updates/README.md), committed in
`1196b4dc`, separately verifies the older `60b3315` producer.

## Entry points and scope

With `tablecraft-ace/ace16-write-preview`,
`NativeImage::preview_ace16_fixed_updates` and
`NativeImage::preview_ace16_batch_updates` can maintain a qualified secondary index
whose physical declaration omits entirely NULL keys. They return the existing
non-journalable `Ace16WritePreview`. Normal Jet 4 editing, ordinary ACE save
refusals and slot updates retain their previous admission rules. The
[selected-forwarded continuation](ACE16_SELECTED_FORWARDED_UPDATES.md#admission-and-bounded-failure)
also corrects a transitive UPDATE-cascade admission bug: the `60b3315` wrapper
called the public batch service and therefore inherited this extended policy.
It now uses a separate strict batch consumer. The earlier verification did not
exercise or establish that isolation; its passing counts are retained as history.

The fixed APIs keep their existing carrier and layout checks: Boolean,
Byte, Int16/32/64, Currency, Single, Double, legacy DateTime, GUID and qualified
Decimal assignments. Decimal uses its existing 17-byte variable slot without
record resizing. Index maintenance uses the existing exact fixed scalar and
qualified composite profiles, with each component's declared direction and type.
Text/Binary, digest keys, Memo/OLE replacement and Extended DateTime replacement
are outside these fixed-update entry points. Unrelated fields remain subject to
ordinary source and constraint qualification.

Complete source read, ownership, index and constraint qualification still applies
to empty or unrelated assignments. A known ordinary ignored-NULL secondary can
therefore produce a byte-identical empty preview after those checks; an unknown
flag does not become admissible because no key changes. The mutation key profile
is bound only when that physical owner needs maintenance.

Logical aliases are grouped by physical data number. An ignored-NULL group is
rejected if any logical declaration makes it a primary index, regardless of which
alias is visited first, including empty or unrelated assignments. Ordinary
non-ignored primary owners keep their existing update path. Distinct physical
owners cannot silently share an index root. Unknown flags and unqualified source
owners refuse; an ordinary alias cannot supply a fallback for an unqualified
declaration.

## Original associations and complete membership

The index leaf stream alone cannot describe omitted rows. The planner reads the
selected key columns for every original logical table ROW, including rows whose
complete key is NULL. It constructs each typed key from those fields, filters the
eligible population, sorts it by the actual index profile and ROW identity, and
compares the complete result with the original physical index stream. Missing,
extra or changed associations refuse before rewriting pages. Projection does not
decode unrelated Memo/OLE payloads merely to build a key.

Request ordinals and expected values refer to the original table. Each row's
complete final tuple incorporates all its column assignments before constraints
are checked. Duplicate assignments to the same row and column refuse, including
conflicts split across batch groups. Changing distinct columns of the same row
uses their original expectations and does not validate an intermediate tuple as
the final result.

Source certification follows logical roots through forwarding when reading the
original key population. It retains the logical root as identity, rather than
substituting a hidden target ROW. At the `60b3315` checkpoint, this certified only
unselected forwarded source rows; selecting a forwarded record still refused.
The separate [resolved-record path](ACE16_SELECTED_FORWARDED_UPDATES.md) now stages
eligible selected payloads under full original owned/exclusive-target checks.
Its validation is recorded separately from this earlier index continuation.

Required is checked before omission. Only an entirely NULL tuple is omitted;
a partial-NULL tuple remains represented and participates in uniqueness. The
all-NULL uniqueness exemption must not become an exemption for partial NULLs.
An ordinary non-ignored physical index on the same columns retains its existing
representation and replacement rules.

## Historical counters and batch order

A represented original key is a replacement, so moving it to another family does
not increment the historical counter. Removing it through an all-NULL result does
not decrement that counter. An originally omitted row becoming represented can
introduce a new historical family. New families are compared with the original
eligible stream and deduplicated across incoming rows; logical aliases never
multiply a physical counter increment. Joining a family that remains represented
from the original source adds no increment. Counter overflow refuses.

Two combinations remain deliberately unsupported because their history depends
on an unqualified update order: an omitted row fills an original family after all
its original members leave, or an omitted row and a represented replacement both
introduce the same previously absent family. The planner returns a typed
unsupported result rather than selecting an arbitrary order or claiming Access
SQL transaction semantics.

## Preservation and bounded rewriting

The complete final key population is repacked into the existing tree. Page
identities, links, height, ownership, allocation sets and file length are retained.
A required page split, retirement or empty-child transition refuses. Existing
node trees use the original counts for earlier leaves and place the remaining
keys in the final leaf. The complete result must fit that retained partition; the
planner does not search arbitrary redistributions for another possible fit. This
is not an allocation or compaction service.

The established masked-update law also remains intact. A non-NULL replacement
writes its exact payload and presence bit. Clearing presence retains the old
ignored payload bytes. NULL-to-NULL leaves those bytes untouched. Decimal NULLs
retain their original slot framing. Changed index spans and an independently
qualified historical counter span are the additional permitted changes; unrelated
source bytes and map metadata remain protected.

Source reads, projected keys, sorting, history comparisons and patch allocations
share the supplied resource budget and cancellation callback. Stale expectations,
constraint failures, unsupported topology, resource exhaustion and cancellation
return no candidate. The API buffers the complete result; it performs no file
publication, recovery journal operation or native Access save.

## Evidence boundary

The current fixtures are independent literal engineering models with source and
expected-image read/check gates. Their opaque payload expectations follow the
pre-existing masked-update contract, including preservation behind a cleared
presence bit. The first new oracle that zeroed such bytes was corrected from that
existing contract; it is not authority to change production NULL storage.

The first five-function preview batch had three passes and two failures caused
by the new oracle zeroing ignored NULL payloads. The expected-image constructor was
corrected from the existing preservation law, with the first attempt retained.
A separate source-only tuple compile attempt stopped on an integer type
annotation before running its gates; that compile failure also remains part of
the preparation history. Neither correction changed the production payload law.

The 657-pass full-library result predates the final Population refactor and the
primary-group guard moving before affected-column filtering. After those changes,
the final focused batch passed all 16 functions: four source-gate functions and
twelve preview functions covering 44 complete images, comprising 16 single-key,
16 tuple, eight existing-node and four forwarded-source cases. The subsequent
shared batch passed 108 regressions across 16 targets. Strict all-target/all-feature
and minimal-feature Clippy checks passed. A final formatting-only line wrap did
not change the byte policy.

One older integration assertion grouped known marker-plus-IgnoreNULL flags
`0x82` with genuinely unknown `0x90`. It now verifies that the known `0x82` source
passes complete source qualification and returns a byte-identical empty preview.
The unknown `0x90` constraint-coverage refusal remains unchanged. The first shared
batch failure is retained; this correction did not relax production admission.

On 29 October 2026, the separate
[original-file proof](ace16_fixed_ignore_null_updates/README.md) qualified 12
target tables and two Sentinels from its SOURCE5 capture, then derived all 16
complete expected images from original sources and immutable requests before
public reference actions or producer calls. The isolated `60b3315` producer
matched all 16 images exactly, totaling 9,076,736 bytes, returned 18 precise
refusals with no candidate output, and passed 16 fresh independent reader checks.
The [saved result](results/ace16-fixed-ignore-null-update-originals-2026-10-29.json)
and verifier preserve the public reference's original two completed calls and
66-call continuation, including the intervening proof correction.

Public writer observations retain differences rather than redefining the engine
oracle: six public images differ by eight bytes behind cleared NULL bits, and
the public writer accepts Required all-NULL cases that this preview refuses.
This proof was committed in `1196b4dc`; it covers the older `60b3315` fixed/batch
producer, not selected forwarding or strict cascade isolation. It is an
independent public-writer comparison, not native Microsoft Access execution or
save/reopen qualification. Those boundaries remain separate work.
