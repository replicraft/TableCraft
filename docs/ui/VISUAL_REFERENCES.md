# Access visual references

The initial shared desktop presentation is being corrected against three retained
screenshots under private `docs/temp/Access Sample/`. They are local reference
material, not copied assets or a public fixture requirement:

| Private reference basename | Visible state | Useful observations |
| --- | --- | --- |
| `Bildschirmfoto_2026-10-28_12-07-35.png` | Table1 datasheet, Home ribbon | Light title chrome, white command ribbon, compact object navigation, separate object tab and dense gray-header grid; blue selected row and yellow active field header. |
| `Bildschirmfoto_2026-10-28_12-07-50.png` | Table1 Design view | Field Name / Data Type / Description grid above a separate lower Field Properties area; contextual Table Design ribbon. |
| `Bildschirmfoto_2026-10-28_12-09-31.png` | File / Account Backstage | Full-page Backstage with left navigation and content sections, distinct from an object document or small popup. |

These are Windows Access captures inside VirtualBox. The title describes an
Access 2016 database while the Account screen identifies Microsoft 365 Apps,
version 2610, build 20527.20088. Do not infer a particular application's UI release
from the database header or title alone. The screenshots include OS/VM chrome;
that chrome does not belong in TableCraft's shared workspace. Account details
are partly obscured and are not product content to reproduce.

## Immediate presentation correction

Use a compact light-gray application title area, white command surface and red
accent rather than a large burgundy banner. Reduce broad button padding and
heading emphasis. Move the functional table/query document switch above its
content, separate from file-opening commands. Keep actual table names visible.
The left object pane follows the reference's compact search and Tables grouping.

The datasheet uses smaller rows and headers, a narrow record-selector strip, gray column
headers and fine grid separators while preserving virtual rows, resizing,
keyboard focus and complete cell copying. Reference pixel sizes are proportions
from a captured display, not platform-independent logical measurements. Current
colors and spacing are approximations requiring visual comparison at the target
scale; they are not measured pixel-exact matches.

The retained datasheet capture does not paint ordinal numbers or a `#` header in
its record-selector strip. The working selectors now follow that organization
with an approximately 18-point strip, while keeping record numbers in accessible
labels, hover text and the bottom navigator. Current-row/selected-row highlights,
selector clicks and full-record Copy remain functional. No pencil, lock or new-row
symbol is drawn without the corresponding supported state.

The active datasheet field uses the observed yellow header, blue current row and
white active cell with a red outline. Home exposes actual View and Clipboard
groups: one large View dropdown offers the existing Datasheet/Table Design
actions for the current table object, and Copy uses the
retained selected cell. Selection and read-only commands do not establish record
editing, rectangular selection or query-result updateability.
The document strip now joins directly to the grid with compact tab padding,
without the former broad central-panel gutter and separator. Default cell
alignment comes from an explicit runtime presentation hint derived from typed
values: numbers and dates align right, ordinary text left. Numeric-looking text
is still text. This does not inspect or implement stored TextAlign overrides;
NULL and expression-only states retain a conservative left-aligned fallback.
Headers, full copied text and the existing checkbox rendering remain independent.

The sample's checked Yes/No fields now render from typed Boolean values plus
their inspected native checkbox declarations. A Boolean type alone does not
override a stored text-box choice or stand in for missing field metadata.

The bottom record navigator follows the datasheet capture's placement below the
grid. Current bounded pages expose known offsets and checked EOF; the capture's
total-record count and Last command do not supply missing backend capabilities.
The working Queries group follows the object pane's grouping. Explicit saved-query
inspection and typed parameter entry have their own documented behavior, but none
of these three captures shows an Access parameter dialog. The current parameter
form must not be described as a visually verified copy of that dialog.

The normal database workspace keeps its existing working command scope. The
explicit synthetic preview additionally shows disabled Ribbon commands to review
Access grouping, iconography and density, as requested on 4 November 2026. Those
controls have explanatory tooltips and cannot dispatch actions. Mock responses
for existing views stay inside a presentation-only host without database services.
They do not establish support for new editing, Create tools, query design or
Microsoft account services.

File/Open uses the Backstage reference's full-page structure and narrow left
navigation. The retained capture shows Account, not Open or Options; TableCraft's
Open form and theme/language Options are provisional surfaces. Dark colors are a
TableCraft adaptation: no dark Access capture is retained. Original generated TableCraft SVGs supply distinct colored command artwork,
with reproducible geometry, explicit theme/disabled variants and hashes under
`assets/ui/icons/tablecraft/`. Their command meanings follow the captures while
retaining original artwork.

The read-only schema inspector now follows the observed Design-view organization,
with a central fields grid and lower selected-field pane. Native rendering and
selection were observed for the public example on macOS. It exposes inspected
facts only, rather than the screenshot's complete editable property set. A generic right-side field
inspector is not established by these references. Native type codes, supported
read carriers and uninspected properties must remain distinguishable; a familiar
property-sheet shape does not justify inventing Required/default/validation facts.

The object pane now uses full-width rows with individually chosen table/search
icons. The inactive selected table has a light surface and thin outline, as in
the retained capture; focus uses the current-row color and a visible outline.
Full original names remain accessible and on hover when the visible name is
truncated. Widget identity includes its document and exact table/query identity,
so filtering preceding objects cannot retarget keyboard focus.

The Design view's lower General sheet now spans the available width with compact
bordered label/value rows. Values remain read-only, selectable literal text.
Plain Up/Down operates only while the currently selected displayed field owns
focus; one-shot scrolling reveals its next field. Property-text selection,
modified keys and a replaced declaration do not reuse a stale field target.
Backstage uses left-aligned icon/text entries, with About and Options at the foot
of the rail. Its active artwork uses the same original generated set as the
Ribbon; earlier Fluent provenance remains with the retained source subset. These changes are visual adaptations of the
retained captures, not a qualification of unseen Access interactions.

## Review each new surface against the captures

Use the actual local images while implementing the corresponding surface, not
only this written description. Compare ribbon grouping and density, object/tab
placement, grid spacing, active/selected states and the Design pane arrangement.
Observe the rendered TableCraft window at a known logical size and scale; record
which surface was inspected and which platform remains unverified. Retain exact
native measurements in future form/report models rather than deriving them from
screenshot pixels or a layout library's preferred units.

These three captures do not establish Relationships, Query Design, Form Design,
Report Design or Print Preview layouts. Use corresponding actual Access references
when implementing those surfaces; record any interim layout as provisional rather
than extrapolating it from the datasheet or a library demo.

A library or replacement icon set cannot redefine the reference appearance.
Use command-specific original artwork and compare its density and meaning with
the captures; an icon library's styling alone is not evidence of an Access match. Preserve labels,
keyboard/focus behavior and native embedded application images. Keep the source
captures private and out of CI requirements.

Long literal document names now use compact, bounded tab captions with full
hover/accessibility names and conditional horizontal scrolling. Focus follows
source identity, independently of translation, and reveals overflow only on
gained focus. The workspace title reserves the existing status label and elides
only its visible filename; neither change claims exact native tab interaction.

## Provisional lifecycle surfaces

The 5 November mock review covers query parameter overflow, collection loading,
errors, unassigned/empty/NULL contents and edit/save/recovery outcomes. No retained
Access capture establishes these dialog layouts or their dark appearance.
Their current rendering is provisional, using the existing shared UI states;
it is not a reproduction qualified against Access. Local review uses the macOS
offscreen GPU renderer at 1280 × 820 logical pixels, 1× scale and Windows keyboard
conventions. Native window and assistive-tool behavior remain separately unverified.

## Verification limits

These references establish observed layout, not complete interaction, keyboard,
accessibility, query or database semantics. Selawik is the selected bundled font;
the captures do not prove every displayed font uses it. Retain native window
management and fallback glyphs, then compare actual rendered TableCraft windows
at representative scales. Headless tests and successful builds alone do not
establish screenshot fidelity or native Access behavioral compatibility.

[Desktop preview](DESKTOP.md) · [UI index](README.md)


## Ribbon and title correction — 5 November 2026

PaintCraft's retained legacy Ribbon implementation supplied a layout reference
for compact quick-access and tab geometry. TableCraft still follows the actual
Access Home/Design captures: a small original app mark, mock-only disabled Save /
Undo / Redo quick-access buttons, a gray tab bar with contextual red labels and
an inset bordered command tray. Native window decorations remain host-owned.
No unsupported quick-access action or fake window control is activated.

Document tabs use neutral gray borders, small object glyphs and a compact
selected surface rather than the former blue fill/red selected underline.
Existing source identity, focus, full-caption accessibility and overflow behavior
remain independent of this paint. Close buttons are absent until actual document
closing exists.

The 49 original vector concepts distinguish Selection, Advanced and Toggle
Filter; Validation, Lookups, Macros and Dependencies likewise have their own
compositions. Builder and Insert/Delete Rows remain explicitly disabled mock
previews placed like the Design capture. Each icon has light/dark and neutral
disabled artwork; original geometry and hashes live with the root assets. None
copies Microsoft logos, upstream glyph paths or PaintCraft artwork.
