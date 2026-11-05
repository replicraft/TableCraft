# Datasheet presentation and window chrome

Presentation choices belong to the shared UI. They do not change stored values,
SQL comparison, database collation, native Format properties or edit input syntax.
The runtime supplies typed display facts alongside the existing scalar text.
`tablecraft-display` owns the renderer-neutral formatter, options and validated
scalar facts; runtime, UI and desktop import those types directly. UI controls
edit the preferences, while the same headless engine formats facts without
reparsing rendered strings. Display has no egui, window or database dependency.

## Regional display

Options separates Date format, Time format and Currency. Each starts with System
locale. The desktop discovers the regional locale independently of the selected
translation language. ICU4X/CLDR supplies regional patterns and currency formatting;
Jiff supplies validated civil date/time components. OS-specific custom regional
pattern overrides are not imported. An unavailable locale is visibly identified
and uses the documented fallback rather than changing SQL behavior.

Date choices include regional, ISO, day/month/year, month/day/year, short, long
and Raw. Time choices include regional, hours/minutes, hours/minutes/seconds,
12-hour, fractional and Raw. Qualified native date-only and time-only Format
properties keep their separate shapes. The default General Date behavior also
keeps a date without a time, or a time without a date, when the typed value has
that shape. Column names do not select formatting.

Currency is an exact scaled integer, never converted through binary floating
point for display. The selected currency controls its symbol and minor-unit
precision. Access Currency stores no ISO currency code: a regional currency is
a display convention, not an inferred financial unit or an exchange conversion.
Explicit currency choices are available. Native formatting support remains
bounded; arbitrary Access custom-format programs are not claimed.

Raw preserves diagnostic storage representation: legacy DATE bits, extended
civil date/time precision, or the four-place Currency amount. A Raw date or time
choice exposes the full stored date/time representation. Copy uses the current
display text; editing retains its separate typed input contract.

Typed NULL is displayed distinctly from empty text and the literal string
`NULL`. Boolean checkbox controls are centered. Field headers are centered and
retain original field identity separately from captions.

## Opening and reading

A successful desktop open selects the first non-system local table in catalog
order and requests its first page. Stored linked paths remain inert. If the table
cannot be read, its read error remains visible. A database without a local user
table still exposes its catalog. Reopening after a completed save restores the
saved table/page instead of replacing it with the first table.

The title follows `Name : Database | File.accdb (Access 2016)` and exposes the
captured absolute source path on hover. The version label describes the file's
compatibility profile; it does not identify the Access release that last saved
it. Editing the Open input cannot retitle an in-flight successful open.

Table tabs have a source-bound Close action. Closing a table keeps the database
and independent query workspace open. Refresh rereads the current native page.
Header sorting and Home Ascending/Descending request a complete native view,
retaining original native row identities and active or temporarily disabled
filter criteria. They keep the table editable through existing reviewed workflows
rather than publishing an unrelated query result or sorting the visible page.
Remove Sort retains filter criteria; Clear All Filters retains the sort. A view
with zero filter criteria is shown as unfiltered and cannot toggle a nonexistent
filter. The first profile has one sort field; sort values and native comparison
policy remain runtime-owned. Ordinary SQL ORDER BY and query results retain their
separate contracts, and query-result Ribbon sorting is not implemented.

## Window and status

The default theme follows the system, with explicit Light and Dark choices.
The condensed title strip retains Save, Undo and Redo icons. Commands without a
working history/save contract remain disabled; no display-only mutation pretends
to persist a database change.

Windows and Linux use the shared title strip with real minimize, maximize/restore,
close and drag requests. macOS retains native window decoration. Native platform
interaction still requires platform verification; headless command assertions do
not establish window-manager behavior.

The left status label identifies the current view. View buttons and datasheet
zoom sit on the right. Zoom changes datasheet density without scaling the Ribbon
or navigation. The navigation heading is All Objects, with a taller search field
and an original magnifier. The application icon is an original red database
cylinder; Settings uses a symmetric gear.

Preview exports use a full-viewport root UI. They deliberately remove the test
harness's eight-point demonstration frame, which is absent from the native host.
The rendered platform, logical dimensions and scale belong in verification
records. No native application activation is needed for those exports.


## New row

Home and the record navigator expose New row for a current native user table,
including an empty table. The dialog separates Omitted, NULL and Value for each
field, requires a successful runtime review, and consumes that review through
the same save worker as cell editing. Changing input invalidates the review.
Cancellation does not claim rollback; retained save resources require resolution
and acknowledgement before source replacement or window closure.

This is an explicit offline workflow, rather than Access's inline blank record.
It adds no fake row to the datasheet or generated-value placeholder. Actual native
admission may refuse a table or input. See the [runtime insertion contract](../application/RUNTIME.md#reviewed-offline-row-insertion)
for platform, default, input and preservation limits. Unsupported Home editing or
history commands remain unavailable.


## Delete row

Home Delete row reviews the actual current native user-table record. Its dialog
shows the original record position and qualified field presentation, defaults to
Cancel, and requires explicit Apply. Enter on the initial Cancel or Escape closes
without deleting. The dialog states that Undo is unavailable. Sorted/query output,
system tables and absent rows cannot issue this intent.

The same save owner retains uncertain outcomes and resources until recovery or
release and acknowledgement. Successful refresh uses complete native membership
so deleting a later page's last record does not leave an unexplained empty page.
This is a provisional explicit offline confirmation, not native Access autosave
or an Undo implementation. See [runtime limits](../application/RUNTIME.md#reviewed-offline-row-deletion).


## Input instructions

Cell edits and New row fields use shared typed instruction tags, translated
through Fluent in all eight UI languages. Original field names are not translated.
Decimal input explicitly uses `.` even when regional display uses a comma. The
UI never reparses formatted Currency/date text as a replacement value.


## Home record navigation and selection

Home Go To reuses the footer's revision-bound record navigation and checked
one-based record input. First, previous and next keep their existing native
source semantics; Last requires observed EOF and an actual final loaded ordinal,
never a declared row total. A failed request retains the previous page/selection.

Select record uses the existing selection for the visible table, ad hoc query or
saved-query result. It preserves the selected field, copies one complete displayed
record and does not grant native edit/delete identity to query output. Select All
is not simulated with a visible-page prefix.


## Long-value previews

Datasheet cells, read-only review cells, tooltips and accessibility descriptions
use a bounded Unicode preview: at most 512 scalars, 1,024 UTF-8 prefix bytes and
eight logical lines. Shortened previews append an ellipsis and translated
context. Native NULL, checkbox, stored-cache and collection provenance remain
separate. Ordinary literal text ending in an ellipsis is not marked shortened.

The original Cell text is retained unchanged, including decomposed Unicode and
line separators. Explicit single-cell Copy still publishes the complete display
text. This does not apply the separate record-TSV clipboard limit to a cell or
represent a preview prefix as a complete stored value.


## Modeless Find presentation

Home Find and the command/Ctrl+F shortcut open a modeless native-table Find window
when the host advertises the complete search service. The capability defaults to
disabled; synthetic preview controls do not enable a native search. Other modal
owners, including Zoom, take precedence and cancel the exact pending Find request.
Query-result Find remains pending, and no Replace control dispatches writes.

Initial controls use current field, whole-field matching, All and case off;
these are explicit local defaults, not imported Access client settings. Idle
Find follows a newly selected native field. Requests capture exact source,
publication, options, formatting policy and a UTF-16 occurrence span. Newer
selection/options, cancellation or source replacement prevents a stale response
from moving the datasheet. A failed or not-found search retains the previous page.
Only a matching checked native page can select the reported record and field.
The current renderer selects the cell; substring highlighting is not implemented.

Formatted search has a separate host capability and does not silently substitute
raw values. The search pattern has a 64 KiB UTF-8 ceiling. These controls follow
Microsoft's [Find workflow](https://support.microsoft.com/en-us/access/use-the-find-and-replace-dialog-box-to-change-data)
and [keyboard contract](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access);
full native interaction comparisons remain unverified.


## Read-only Zoom box

Home Zoom, a scalar cell's context menu and Shift+F2 on the actual focused cell
open a resizable shared viewer for table and query values. It retains the complete
original display text, native provenance and a source/publication/selection binding.
Changing that binding or display policy closes the viewer. Theme and UI-language
changes retain the same value. Query results stay read only.

Opening scans at most 8 MiB of UTF-8 text and builds sparse byte checkpoints.
Painting lays out visible text rows rather than cloning or laying out the full
value each frame. Very long lines continue on bounded visual rows; original line
breaks remain intact for explicit complete-value Copy. Values above the explicit ceiling remain outside viewer admission; the original
cell value is retained. Text-size controls affect this viewer.

OK, Cancel and Escape dismiss the read-only viewer without changing stored data.
For an eligible native cell, Edit through review delegates to the existing typed
reviewed-edit workflow; it does not mutate text inline or bypass native admission.
The [Microsoft Zoom reference](VISUAL_REFERENCES.md#published-dialog-references--5-november-2026)
guides the rectangular text body and footer. Full native Zoom editing, font/style
and rich-text behavior are not claimed by this read-only implementation. No
native Access comparison or OS-window interaction is established here.


## Native selection filters

Home Selection exposes Equals and Does not equal for an eligible native scalar,
with Is Null/Is Not Null labels for a stored NULL. The runtime reads the exact
stored value from its immutable source; UI formatting and copied text never become
filter operands. A new criterion replaces the previous criterion on that field;
criteria on different fields combine with AND.

Toggle Filter temporarily switches between matched and complete source membership
while retaining criteria. Applying a new Selection while the retained filter is
turned off starts fresh criteria rather than extending inactive criteria, while
retaining the sort. Advanced > Clear All Filters removes the criteria only after
a matching runtime-issued native view arrives. Both counts come from checked complete
runtime membership, and an empty result keeps its columns and clear/toggle actions.
Page and record navigation use visible offsets with the same retained filter;
explicit original ordinals continue to identify native edit and collection targets.
A failed or stale read retains the prior page and filter. Switching document views
retains the table's filter; a plain new-table publication or Close table clears it.
These controls follow Microsoft's [filter workflow](https://support.microsoft.com/en-us/access/apply-a-filter-to-view-select-records-in-an-access-database).
Custom common filters, partial-text selection filters and Filter by Form remain
unimplemented; synthetic scenes supply no native Access comparison evidence.


## Native view sorting

An issued native view handle can own filter criteria, a single sort field, or
both. Its immutable physical row map may follow arbitrary display order; original
ordinals continue to identify review and collection targets. A non-source-ordered
map is admitted only with the requested or retained sort witness. Sort metadata,
filter count, mode and cardinalities are checked before publication; failed reads
keep the previous page and view. A new sort replaces the current sort field.

After an acknowledged save, the host can reapply the opaque definition to the
freshly opened source, with a new handle and checked sort/filter metadata. This is
an explicit read after save, not persisted native OrderBy/FilterOnLoad support.
Microsoft's [typed sorting guidance](https://support.microsoft.com/en-us/access/sort-records-on-text-numeric-or-date-values)
and the retained Home screenshot guide the controls. Native application ordering,
selection and save/reopen comparisons remain separate qualification work.
