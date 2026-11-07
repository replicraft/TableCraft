# Ribbon presentation coverage

Normal and synthetic hosts display the same complete Ribbon structure.
Unavailable commands remain disabled with “Not available yet” tooltips and
accessible descriptions; they never run a database operation. Captured layout, visible command coverage and native
interaction compatibility are separate claims.

## Current groups

| Tab | Visible groups | Evidence and limits |
| --- | --- | --- |
| Home | Views, Clipboard, Sort & Filter, Records, Find, Text Formatting, Zoom | Retained Home capture guides layout and command order. Working commands use current services; unavailable formatting and group launchers are disabled previews. |
| Table Design | Views, Tools, Show/Hide, Field/Record/Table Events, Relationships | Retained Table Design capture guides the visible strip. Schema mutation and macro commands remain disabled. |
| Query Design | Views, Results, Query Type, Query Setup, Show/Hide | Published command procedures and the actual Query Setup crop guide the groups. No complete modern Ribbon capture is retained. Query types and setup options beyond the existing editor are disabled previews. |
| Create | Templates, Tables, Queries, Forms, Reports, Macros & Code | The published Access 2010 Create crop supplies selected visual evidence. Modern commands are documented previews; exact release-specific layout remains unqualified. |
| External Data | Import & Link, Export | Disabled provider, saved-task and export previews. No new provider or export operation is implemented by this strip. |
| Database Tools | Tools, Relationships, Analyze, Move Data, Add-ins | Disabled previews; exact release-specific density remains provisional. |
| Help | Help and feedback commands | Disabled previews; no account or online service is implied. |
| Table Fields | Views, Add & Delete, Properties, Validation | Disabled field-management previews. Full galleries and their native interaction remain unfinished. |
| Table | Properties, Before Events, After Events, Macros & Code | Disabled table/data-macro previews. No VBA or data-macro execution is supplied by these controls. |

## Font catalog and caption alignment — 5 November 2026

View, Paste and Zoom place their chevrons 3.5 logical points below the actual
caption, independently of the command's shared height. Refresh All retains its
inline final-line arrow. Font family and size selectors share a 22-point row;
the second formatting row has a seven-point gap. The top row is bounded before
its separators measure available height, retaining the compact default strip.
Refresh All and Format Painter use updated original artwork in all four variants.

The Font selector browses a host-supplied catalog, including bundled families and
installed system families discovered through desktop-only Fontique. A focused search field is its
first item and filters names case-insensitively while typing. Empty matches have
a localized message; closing/reopening clears the search. The bounded 128-character
filter sits above the virtualized, independently scrolling results with one scrollbar. Selawik remains selected; applying family,
size and other text formatting is unavailable. Font discovery does not load
system faces into egui or change database fields. See the [dependency contract](../development/DEPENDENCIES.md#desktop-font-family-discovery--5-november-2026).

Paste opens its existing literal-text workflow, with Paste Special and Paste
Append visible but unavailable. The menu stages an exact UI target and returns
focus to the actual rendered native cell before requesting clipboard text. A
bounded focus wait never requests an old widget ID. Delayed replies retain the
existing source/focus validation and still require the normal edit review.

Table-view Copy from a temporary Ribbon strip returns focus to the selected
rendered cell after publishing the complete clipboard value. This requires a
current-pass command witness and no popup, dialog, paste or backend-action owner.
It does not extend query Copy or other output-only command behavior.

## Menus and unavailable controls

Icon-only formatting commands identify themselves by translated name on hover.
Unavailable icons also show “Not available yet”; these descriptive tooltips do
not make the commands callable. English and German pointer-hover regressions
check the settled tooltip text and unchanged disabled behavior.

Working menus use visible chevrons, actual popup accessibility and exact existing
capability checks. Select, Refresh All and Zoom expose their native
menu shape while delegating only to the existing record selection, current-table
refresh and read-only cell viewer; additional choices remain disabled. Preview dropdowns use disabled chevrons to show their native
role; they do not open fabricated working menus. Clipboard and Text Formatting
preview launchers remain disabled and expose no accessible activation action.

Status view buttons appear only for admitted switches to another view of the
current object. The percentage zoom controls appear only when a visible datasheet
has columns whose metrics can be scaled. These rules do not change the separate
cell-value Zoom dialog or enable schema editing.

## Normal-host coverage correction — 5 November 2026

Previously, the normal host hid controls that were present only in synthetic
previews. Home now always includes Cut, Format Painter, Save, Spelling, More,
Text Formatting and both group launchers. Create, External Data, Database Tools
and Help are visible in normal mode; table tabs retain their object context.
Query Design also shows its unavailable query-type and setup controls.
These presentation changes add no database operations or editing permissions.

## Compact default layout and contextual controls — 5 November 2026

The English Home strip fits the desktop's default 1280×820 logical window without
horizontal overflow at 1×, 1.25×, 1.5× and 2×. View and Zoom use narrower command
widths; formatting uses compact selectors and icon spacing with visible dividers
between font, list/indent, color, alignment and border controls. Refresh All places
its chevron beside the final caption line; View and Zoom retain below-caption
arrows. Light and dark offscreen frames were reviewed at the actual default size
on macOS with Windows chrome emulation, against the retained Windows Access Home
capture. Other locales and narrower windows may still require overflow.

Delete now opens a real dropdown whose Delete row entry uses the existing
reviewed deletion route. Merely opening it performs no operation. Changing its
selected native row, object, presentation publication or capability closes the
stale menu; selection rechecks the captured target. No broader delete modes are
implemented. The retained Home image establishes the dropdown affordance; there
is no retained Access capture of its opened menu.

Contextual Table Fields, Table, Table Design and Query Design selections return
to Home when their object context ends. Global tabs remain available. The disabled
Query Design Return selector has a localized accessible name/value and no callable
activation contract.

## Expanded, tabs-only and temporary display

The session starts with the command strip expanded. The trailing chevron,
Ctrl+F1 or a tab double-click toggles tabs-only display. Clicking a collapsed tab
shows its current commands over the document; Pin restores the expanded layout.
Escape dismisses an open child menu before the temporary strip. Outside clicks,
focus leaving the strip, object/context changes, Backstage and modal workflows
also dismiss temporary controls. This presentation state never requests database
operations or changes the loaded records; working commands retain their normal
capability and target checks.

[Microsoft's keyboard contract](https://support.microsoft.com/en-us/accessibility/windows/use-the-keyboard-to-work-with-the-ribbon)
includes Access and documents Ctrl+F1. The [Access 2010 interface guide](https://support.microsoft.com/en-us/access/guide-to-the-access-user-interface)
documents tab double-click. The retained Home capture shows the expanded strip
and a trailing chevron. No collapsed, temporary or pinned Access capture is
retained; overlay dismissal and pin placement are current UI adaptations, not
qualified native interaction parity. No fullscreen Ribbon mode is implemented.

Headless workspace integration also checks the Font search as a child popup:
typing and clipboard paste stay in search without editing records or SQL; F6
retains that input owner, Escape closes the catalog before the temporary strip,
and outside record clicks or object switches dismiss stale presentation.

The synthetic scenes `ribbon-collapsed` and `ribbon-temporary` share the ordinary
workspace presentation and retain the same resource-free host. Native graphical
review briefly resumed on 6 November 2026 and is suspended again at the user's
request. Current verification uses offscreen rendering and headless tests.

## Keyboard navigation

F10 enters the selected Ribbon tab. Left/Right moves across available tabs;
Down enters the selected tab's enabled commands, opening a temporary strip when
collapsed. Ctrl+Left/Right moves between groups with enabled commands, while
Tab/Shift+Tab traverses enabled commands. Navigation changes focus and Ribbon
presentation without dispatching a database action. Enter/Space retains each
control's existing activation and capability checks.

Child popups and their text fields own their input. Escape closes a child popup
before dismissing a temporary strip; from expanded commands it returns to the
selected tab, then to the document. Find, modal reviews and active input-method
composition keep their existing ownership. F6 remains pane navigation, and
Shift+F10 remains a context-menu shortcut. Focus targets come from enabled
controls rendered in the current pass, rather than remembered widget IDs.

The [Access keyboard reference](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
documents F10, tab arrows, Down, group navigation and command traversal. This is
partial keyboard support: standalone Alt and letter KeyTips are not implemented.
Traversal across group boundaries and focus styling remain TableCraft
adaptations; the retained screenshots show no active keyboard-focus or KeyTip
state. Verification results belong in the [presentation record](PRESENTATION_PREVIEW.md).

## New-record shortcut

Ctrl+Plus opens the existing reviewed New Record dialog when a current local
native table cell, row selector or appended New-row control owns focus. On
macOS TableCraft uses Command+Plus, matching its other platform-command
shortcuts; this is an adaptation, not native Mac Access parity. Logical Plus
also accepts layouts where Plus is unshifted, while Equals requires Shift.
Owned repeats are consumed without opening another request, and the shortcut
does not change application or datasheet zoom.

Text editors, Find, modal reviews, Ribbon/object tabs, query results and unsaved
table designs retain their own input ownership. Admission still checks the
actual table/document target. The command opens the existing omission-aware
review workflow; it does not provide inline new-row editing or change record
commit and Escape behavior. New-row controls expose the localized shortcut in
their tooltip.

[Microsoft's Access keyboard reference](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
documents Ctrl+Plus for adding a record. The retained Windows datasheet capture
(`Bildschirmfoto_2026-10-28_12-07-35.png`, 1920 × 1141; scale unrecorded)
shows the appended new row; keyboard behavior is grounded in that Microsoft
contract and headless interaction checks, rather than a native Access run.

## Remaining qualification

This inventory does not establish complete native Ribbon parity. Full galleries,
contextual control states, native keyboard access keys and complete modern
captures for the uncaptured tabs remain unfinished. The unavailable controls are useful
for layout review while preserving explicit implementation boundaries. See the
[visual references](VISUAL_REFERENCES.md) and [presentation review](PRESENTATION_PREVIEW.md)
for observed captures, render dimensions and actual checks.
