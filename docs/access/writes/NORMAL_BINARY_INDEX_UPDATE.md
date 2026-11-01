# Fitting Short Binary index updates

Normal fitting UPDATE now maintains qualified nullable ascending and descending
Short Binary secondary root leaves beside a manual Int32 primary key. The
existing fitting planner, retained offline sessions and `query journal-update`
share this path. Native Access save/reopen remains unrun.

## Admission and placement

Fresh native catalog, required-program absence, constraints, complete source
preservation and physical index ownership remain mandatory. This branch requires
its qualified dense native declarations and exact Short Binary target profile.
Metadata selection grants no permission. Every actual plan and recovery rechecks
the source, including empty requests.

The record keeps its original capacity, directory, slot count and inactive bytes.
Nullable Binary changes rebuild the qualified root-leaf population in place;
logical aliases share one physical update. NULL and present-empty remain distinct.
History stays unchanged, including when a new distinct key increases the current
population beyond the historical counter. Unique primary keys and unrelated
fields remain protected.

All changed records and index streams must fit existing owned capacity. This
branch does not grow records, allocate pages or split Binary indexes. Automatic
selection chooses the fitting branch from original metadata and does not retry
another physical planner after a failure. Fixed-only UPDATE admission retains
its prior boundaries.

## Independent proof and recovery

The physical kernel retains original wires and checks expected key values before
replacement. Payload-sized comparison and encoding work is prepaid, including
before early conflict errors. Separate proof replays original/requested DATA
values and Binary component/ROW streams, masks, free bytes, history and protected
complements, then compares the whole candidate and freshly checks constraints.
It consumes no producer patches or staged key arrays.

The existing TCNI0005 intent retains original names, rows, expected/replacement
carriers and BEFORE binding. Checked V1 recovery reloads, replans and compares both
complete images before effects. No physical permission or new wire family is
introduced. Host snapshots refresh after commits and keep lease/receipt ownership.

Typed calls carry exact Binary values. SQL/CLI acceptance uses representable NULL
assignments; no Binary literal syntax or text/hex coercion is invented.

## Observed acceptance

The [source phase](../../testing/ACE16_BINARY_UPDATE_SOURCES.md) passed sixteen
native views and 128 exact independent SDK observations before these producers.
Six focused unit functions pass literal plans, independent proofs, protected-byte
forgeries, resource/cancellation controls, noops and stale-key checks. Two native
functions pass actual commits and interrupted/forged recovery. Actual retained
sessions and CLI saves/readbacks match complete independent images. Prior Binary
INSERT host tests pass with the newly qualified update capability.

The [combined execution record](../../testing/results/ace16-index-updates-splits-and-primary-creation-2026-11-01.json)
records 1,165 ACE units, 79 adapter units, 104 CLI units and 90 default recovery
functions, plus strict feature checks and the focused tranches. These checks do
not qualify native Access saves.
