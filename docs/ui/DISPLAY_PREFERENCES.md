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
Header sorting and Home Ascending/Descending execute a complete bounded query
with database comparison rules and publish a separate, read-only sorted result.
They do not sort only the visible page or assign native edit ordinals to reordered
rows. Tables with fields outside the scalar query profile, including calculated
fields requiring an explicit cache policy, currently refuse a whole-table sort.
SQL text is preserved separately; sort identifiers are composed as owned
query nodes rather than interpolated into SQL.

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
