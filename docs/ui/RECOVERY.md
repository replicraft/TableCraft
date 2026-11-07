# Unfinished TableCraft saves

Opening a database with its reserved `.FILENAME.tablecraft-journal` sibling
shows an **Unfinished TableCraft save** prompt. Opening and rendering the prompt
never repairs a file, replays a query, removes an entry or acquires an offline
write session. Cancel has initial keyboard focus. The captured native path stays
with that Open attempt; editing the visible path or receiving another picker
result cannot redirect recovery.

Recover explicitly asks the runtime to verify and recover that active journal.
Close Access and other writers first. This is an exclusive offline operation,
using the same checked native intent and file association rules as the existing
CLI recovery route. It does not grant broader database editing eligibility.
Malformed, foreign, incomplete or unsupported artifacts refuse through the typed
failure lifecycle. There is no Open anyway, automatic deletion, sibling search
or fallback to a different journal/action decoder.

## Ownership and outcomes

The desktop captures attempts and dispatches actions. Shared UI owns the modal,
keyboard behavior and sectioned `recovery.ftl` catalogs. Runtime owns
`SaveWorker::start_reserved_recovery`, deriving the reserved sibling and using
Access's existing `OfflineWriteSession::recover_update`. It shares the same
nonreplaceable worker with edits, insertion, deletion and initial-table Save.

While work or retained resources remain, Open, Create and window closure cannot
replace the owner. Cancellation requests stopping; it does not establish rollback.
Only actions supplied by the actual retained owner are offered. Explicit resource
release preserves diagnostic files and does not claim that recovery succeeded.
Stale attempt/operation IDs cannot resolve or acknowledge a different owner.

The neutral result view reports known commit/rollback facts, completion and
namespace cleanup separately. Recovery does not invent changed-row counts.
ACE 16 first-table schema facts appear only for a committed replacement.
The terminal report remains until acknowledged. After acknowledged successful
recovery, the exact captured source is opened afresh and its first user table is
selected normally. If reopening fails, the known completed outcome remains
visible alongside that error. Other failures and released uncertain outcomes
leave the file closed.

Windows retains its private retired artifact under the existing NTFS policy;
that fact reaches the fresh document's revision-bound status. It does not claim
Unix directory barriers or durable deletion. See the
[offline-session contract](../access/writes/OFFLINE_WRITE_SESSIONS.md) and
[application ownership](../application/RUNTIME.md).

## Reference and verification boundaries

This is a TableCraft journal workflow. Microsoft documents
[Access Compact and Repair](https://support.microsoft.com/en-us/access/compact-and-repair-a-database),
which is a different operation. The actual retained Windows/VirtualBox Access
1920×1141 datasheet reference was reopened; scale is unrecorded. It contains no
recovery prompt. This modal is an explicit adaptation, not observed Access
recovery-layout or interaction parity.

Headless scenarios use real paired pending/committed Jet 4 journals and ACE 16
V2 first-table journals, with independent BEFORE/AFTER fixtures. They exercise
inert Open/Cancel, exact captured paths, native outcomes, stale actions, retained
failures, release, close vetoes and failed reopening. Synthetic `restart-prompt`
and `restart-result` scenes have no database, edit, native row or runtime owner;
they cannot qualify physical recovery. Final local checks and offscreen evidence
are recorded in [presentation preview](PRESENTATION_PREVIEW.md) and
[implementation status](../IMPLEMENTATION.md).

This covers only the reserved active path. Retired artifacts at other paths,
caller-selected journals and aliases need explicit separate diagnosis or the
existing CLI confirmation route. There is no general recovery browser, native
Access Compact and Repair implementation, live multiuser session or physical
crash/power-loss qualification. Native Windows/Linux execution and Access
comparisons remain separate from local macOS tests and cross-compilation.
