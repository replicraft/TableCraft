# Normal Short Binary index deletion

Normal DELETE maintains qualified nullable ascending and descending Short Binary
secondary root leaves beside a manual Int32 primary key. Native planning,
retained offline sessions and `query journal-delete` use the existing TCND intent
and checked journal recovery. Native Access save/reopen and Windows durability
remain unrun.

## Admission and preservation

Fresh native catalog, required-program absence, complete checked constraints,
all-local ownership and preserved source checks remain mandatory. The selected
Short Binary declarations, flags and widths must be admitted by the existing
native profile. Logical aliases share one physical change. NULL, empty values,
segmented values and duplicate secondary keys retain distinct key semantics.

Removal matches original physical ROW references, including after earlier
commits have shifted visible row ordinals. The producer retains original key
wires. Independent proof reconstructs original values and survivor streams,
mask and free-space bytes, then compares the complete candidate. It does not
consume producer patches or staged key facts.

The operation preserves historical counters and inactive index tails. Each
commit retains its own source's tail: two successive deletions can produce a
different inactive tail from one atomic deletion of the same rows. DATA payloads,
slot count, maps, properties and EOF remain unchanged; selected DATA directory
words gain the deletion flag and the table's live count decreases.

The qualified path requires root leaves with zero prefix compression. Binary
NODE maintenance, splits, allocation and page retirement remain work. This does
not widen indexed Binary UPDATE or grant permission from header recognition.

## Transactions and host behavior

Recovery freshly qualifies the BEFORE source, rebinds the original request,
replans and compares complete images before effects. It uses the existing
checked journal session, including interrupted rollback and mixed-target
refusal. No new serialized permission or preview conversion is introduced.

Typed and SQL operations retain the session's exclusive lease and completion
receipt. Handles from an earlier snapshot refuse after a commit; callers obtain
fresh handles from the reloaded snapshot. Empty requests preserve the file and
return a completed zero-row receipt. The CLI buffers its completion report and
uses the same service.

## Evidence

The [independent source qualification](../../testing/ACE16_BINARY_DELETE_SOURCES.md)
passed fourteen native views and 112 exact SDK observations before matching
DELETE producers ran. Focused native tests cover complete literal commits,
independent proof, forged candidates, resource/cancellation controls and checked
recovery. Actual host tests cover typed/SQL saves and retained-session handle
refresh. The two-commit control derives its expected inactive tails from the
frozen intermediate source; it does not equate sequential and atomic images.

These are local engineering checks, separate from native Microsoft Access
qualification. The combined batch passes 1,160 ACE units, 79 adapter units, 102 CLI units
and 90 default recovery functions, plus query, CLI and prior Binary INSERT
regressions. Strict feature checks pass. The [execution record](../../testing/results/ace16-primary-creation-and-binary-delete-writes-2026-11-01.json)
retains failed checks and corrections alongside successful observations.
