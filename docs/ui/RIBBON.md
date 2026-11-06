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

## Menus and unavailable controls

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

## Remaining qualification

This inventory does not establish complete native Ribbon parity. Full galleries,
contextual control states, native keyboard access keys and complete modern
captures for the uncaptured tabs remain unfinished. The unavailable controls are useful
for layout review while preserving explicit implementation boundaries. See the
[visual references](VISUAL_REFERENCES.md) and [presentation review](PRESENTATION_PREVIEW.md)
for observed captures, render dimensions and actual checks.
