# Reviewed saved-table deletion

The Navigation Pane's Delete command and focused-row Delete key capture an exact
closed local user-table target. System and linked tables are refused. Native
stored-object tabs must first close; source-free SQL drafts can remain open.
The host must enable the command only when its real consuming runtime and native
save path are connected. An unavailable command remains visible and disabled.

The confirmation first requests bounded read-only native inspection. Yes stays
disabled until that exact observation admits deletion. The dialog reports the
native declared record count rather than inventing a scan count. Yes then receives
focus and requests a fresh consuming native review followed immediately by the
save worker. There is no separate pending schema edit or Ctrl+S step. No and Escape
cancel only the matching pending inspection/review; cancellation after save
acceptance requests cancellation without implying rollback.

This follows Microsoft's [screen-reader table deletion instructions](https://support.microsoft.com/en-gb/accessibility/access/use-a-screen-reader-to-create-tables-in-access-desktop-databases)
for Navigation Pane selection, Delete, initially focused Yes, and confirmation.
Microsoft's [DROP statement contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/drop-statement-microsoft-access-sql)
requires the table to be closed. Native dependencies and preservation constraints
remain engine/runtime admission checks; this UI does not assume automatic
relationship deletion, cascade changes or free-page reclamation.

Inspection, consuming review and save actor IDs are separate. The UI and host
share one immutable request with `Arc`, so delayed results cannot substitute a
new table name or document revision. Failed reads, retained recovery resources,
unknown actor outcomes and failed full-image reopening retain distinct states.
An unknown outcome or saved-but-not-reopened source blocks further mutation;
hiding its dialog does not clear that restriction. Recovery and release controls
reflect only the retained actor's actual permissions.

After an actual known-AFTER acknowledgement and resource release, the host must
verify the operation-specific complete native reopen witness. Shared UI publication
then validates a newer document revision with exactly the selected table absent,
all remaining table/query metadata and ordering preserved, and the same path and
format. Empty table catalogs are valid. Publication preserves SQL drafts and tab
IDs, clears a deleted navigation selection and invalidates native result handles.
A `Saved` message or matching display DTO alone supplies no native authority.

The retained Navigation Pane screenshot was opened during this work: Windows
Access in a 1920×1141 VirtualBox capture, with unknown display scale. It contains
neither an expanded Delete menu nor a deletion dialog. Those surfaces follow the
primary documented workflow, not a claim of screenshot-verified dialog parity.
Current interaction checks are headless source-free presentation tests; they are
separate from engine admission, native saves, independent-reader reopen evidence
and Windows Access behavior.
