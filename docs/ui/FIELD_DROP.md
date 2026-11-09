# Reviewed saved-field deletion

The shared Datasheet field menu offers **Delete Field**. Saved Table Design
uses **Delete Rows** or the Delete key for the selected native field. The normal
desktop host supplies actual platform support; a readonly source inspection must
succeed before **Yes** is enabled. **No** and Escape dismiss the confirmation.
**Yes** marks a local pending deletion. The stored schema and file stay unchanged
until an explicit Save or Ctrl+S completes the reviewed native operation and
checked reopening.

This follows Microsoft's documented Datasheet sequence of Delete Field, Yes and
Save, and its Design sequence of selecting the field, Delete Rows/Delete, then
Save. Removing a stored table field is distinct from removing a query projection.
See [Add or delete a column in a datasheet](https://support.microsoft.com/en-us/access/add-or-delete-a-column-in-a-datasheet)
and [Delete a field](https://support.microsoft.com/en-gb/access/delete-a-field).

## Identity, eligibility and confirmation

The target binds the exact open tab, document revision, true stored field name
and actual native column number. Caption, visible position and the surviving
field vector are not identities. Inspection creates no AFTER image or write
lease. Its source eligibility is metadata only; named review freshly repeats the
native proof and fitted-property preparation.

Current support is the [bounded native DROP profile](../access/writes/FIELD_DROP.md):
an empty canonical eleven-page ACE16 field-history source with explicit stored
Name AutoCorrect OFF, and a supported nonprimary, nonindexed Number Long or
compressed Short Text255 field. Missing, absent or unknown settings refuse;
this command never disables settings automatically. Queries, relationships,
required programs, unknown metadata, populated tables and older formats remain
outside this profile. These are current TableCraft limits, not general Access
field-deletion restrictions.

The confirmation and pending state own the exact target and local draft
revision. Confirmation supplies no native authority. Other dirty stored-object
owners block the operation rather than being discarded. Save/Discard/Cancel
binds the current deletion and deferred tab/file/window action; a stale choice
cannot remove newer work. Discard clears only an unstarted matching local draft.
Save resumes a deferred transition only after checked publication.

## Persistence and failure ownership

The runtime issues a consuming review and opaque native capability. Desktop
compares the complete target before starting the nonreplaceable save actor; it
contains no native writer or source qualification logic. The operation has no
absent-target or unchanged success route. Cancellation preserves pending intent
and genuine retained recovery/release owners. A cancelled readonly review cannot
publish a later capability into a different owner.

Terminal native persistence and ACK do not publish a fresh UI schema. A distinct
full-image `FieldDroppedReopen` witness must succeed first. Atomic publication
preserves tab identity/order, preferences and source-free scratch SQL bytes while
invalidating native result handles. Scratch retention is an application-tool
boundary, not evidence that Access retains an unsaved native Query object. Saved
Design selection resolves the next surviving native number, or the previous one
when the removed field was last, only against fresh matching schema metadata.

A failed or cancelled AFTER read retains the original UI and actual saved
outcome. Retry uses only the accepted witness; it does not automatically reopen
the original source. Return retains the exact deletion status, cleanup notice
and stale-source mutation guard until an explicit fresh document reopen. Actor
loss is uncertain, never a fabricated no-write or rollback result.

Number deletion can leave native numbers 0/2 with high-water 3. The separate
qualified sparse generated INSERT path uses those actual numbers and omitted
field defaults; a DROP receipt itself grants no insertion authority. Surviving
Text variable-slot gaps remain outside ordinary INSERT support.

## Verification boundary

The [coordinated headless macOS record](../../tests/results/schema/saved-field-drop-gui-2026-11-09.json)
reports 681 shared UI tests passed and 201 desktop tests passed with one existing
ignored desktop test, including all seven focused DROP test functions. They
exercise actual Datasheet and Design entry points, No/Yes/pending Save, complete independent raw AFTER
images, retained scratch/tab ownership, Number insertion after witnessed
deletion, review cancellation, source refusal, retained release, failed AFTER
reads and exact close choices. The focused tests are included in those suite
totals, not added to them.
The subsequent ownership/presentation batch passed 201 desktop tests and 112
CLI unit tests. Its shared UI run passed 681 cases and caught a new retry test
checking the action before the next-frame collector ran. After correcting only
that test's timing, all 32 DROP UI cases passed. Repeated cases are not added to
the suite totals. The fixtures are independently generated first-party byte
models, not original Access-authored databases.

All 16 final offscreen frames were directly inspected: light/dark confirmation,
inspection, refusal, pending Save, close and review states, plus compact Russian,
Japanese, Chinese and German layouts. Pixel dimensions/scales and image hashes
are in the same record. Ordinary eligible confirmation has Yes/No only; retry is
shown after refused, interrupted or failed inspection. These frames open no file
and contain no native capability, save worker or completed deletion.

Native graphical checks resumed on macOS. Startup and File/Open appearance were
observed in a task-owned app, but path-entry automation did not establish an
opened database; that incomplete attempt does not qualify native DROP interaction.
The generated review inputs remained unchanged and the task app was closed.

Retained Access Datasheet and Design screenshots guide the base surfaces. No
expanded Delete Field menu, deletion confirmation, pending deletion or failure
capture exists; those surfaces are explicit adaptations of documented workflow.
Headless interaction and offscreen rendering do not establish Microsoft Access
execution, Windows rendering parity, SDK-authored deletion or Access 97–2019
write completion.
