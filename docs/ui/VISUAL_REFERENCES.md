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

## SQL workspace qualification — 6 November 2026

The retained reference inventory has no SQL-editor or query-results capture.
Query Design screenshots establish QBE layout; the Totals references show table
datasheets. Neither establishes SQL-editor spacing or result-provenance messages.
The [SQL workspace](SQL_WORKSPACE.md) therefore records those surfaces as
TableCraft adaptations, with scoped offscreen evidence in the
[presentation record](PRESENTATION_PREVIEW.md#scratch-sql-result-ownership--6-november-2026).

## Published dialog references — 5 November 2026

Four additional Microsoft Support images were fetched and opened headlessly. The
reference files remain in ignored local cache; public documentation links the
originals. The articles list supported Access releases rather than identifying
the exact application build captured in each image.

| Surface | Published reference | Observed layout and scope |
| --- | --- | --- |
| Parameter input | [Microsoft parameter-prompt article](https://support.microsoft.com/en-us/access/why-does-access-want-me-to-enter-a-parameter-value) and [actual dialog image](https://support.microsoft.com/en-us/access/media/ac-enterparametervaluedialogbox.jpg) | Compact prompt with a literal parameter label, one input and OK/Cancel. Microsoft's [parameter-query workflow](https://support.microsoft.com/en-us/access/use-parameters-to-ask-for-input-when-running-a-query) specifies a separate prompt for each parameter. Explicit VALUE carrier choices and NULL controls remain TableCraft profile extensions. |
| Find | [Microsoft record-finding article](https://support.microsoft.com/en-us/access/find-records-in-an-access-database) and [actual Find dialog image](https://support.microsoft.com/en-us/access/media/ac-findreplacedb.gif) | Compact left-label/input rows, scope/match/direction lists, case/format checkboxes, and Find Next/Cancel at the right. The legacy image shows a Find/Replace pair, but only the Find tab contents are pictured. The Replace tab uses the documented Microsoft workflow and remains a visual adaptation until a corresponding capture exists. The exact pictured application build and scale are unknown. |
| Zoom | [Microsoft Zoom-box article](https://support.microsoft.com/en-us/access/use-the-zoom-box-to-enter-text-and-expressions) and [actual Zoom image](https://support.microsoft.com/en-us/access/media/ac-zoom-box.png) | Resizable rectangular text body, bottom Font/OK/Cancel controls and a resize grip. Blue explanatory callouts belong to the article image, not the application. TableCraft uses a source-bound read-only viewer with text-size controls and separately reviewed editing. |
| Attachments | [Microsoft attachment article](https://support.microsoft.com/en-us/access/attach-files-and-graphics-to-the-records-in-your-database) and [actual dialog image](https://support.microsoft.com/en-us/access/media/access-attachments-dialog-box.png) | Filename-first list with paperclip symbols, a side action column and dismissal/confirmation footer. TableCraft currently exposes stored metadata and Close; payload opening/export and attachment mutation require separate capabilities. |

Observed image SHA-256 values:

- Parameter prompt: `cd54cd50aa01ee70a7ab578db15299cb7f0e1b24722326893eeb810f4c18d5d9`.
- Attachments: `adcc6b059d2289a147aa12461a88dac9da67bdab0174ad0b9025f6a47119631c`.
- Find: `4563876ea3b8591ae8fda5d75f6c77929cf73c334e24500b86680be90240ea53`.
- Zoom: `ba262cf21e026309d6f27f9f423041477f6be24b32128aadadb124a900b332ad`.

These references extend the Home/Design/Account record. They supply no dark-mode,
multivalue/history, offline-edit/recovery or native platform interaction evidence.
Viewing the images does not establish validation timing, parameter ordering,
focus traversal or native save behavior.

## Published Query Design references — 5 November 2026

The private reference directory contains no Query Design capture. Four public
Microsoft images were downloaded to ignored local cache and opened headlessly
before considering that surface. The exact rendered build, platform and scale
are unknown; filenames and supported-release lists alone do not establish them.

| Viewed image | Observed composition | SHA-256 |
| --- | --- | --- |
| [AND criteria](https://support.microsoft.com/en-us/access/media/city-dob.gif) | An upper Contacts field-list card, a horizontal splitter and a lower column-oriented grid with Field, Table, Sort, Show, Criteria and or rows. Criteria occupy the same grid row. Green numbered callouts are article annotations. | `951ed0bb5e231485cce3c4df43073ae2c50617d3769070c7342b0e6188f24712` |
| [OR criteria](https://support.microsoft.com/en-us/access/media/city-or-dob.gif) | The same upper/lower composition, with alternative criteria on different rows and checked Show cells. | `5786b7f7a362ab722959983eb69d8c94cae61a87dc0b48de0b66aaf82ec9b2e0` |
| [Show Table dialog](https://support.microsoft.com/en-us/access/media/access2016-show-table-dialog.png) | Tables, Queries and Both tabs above a list; Add and Close at the lower right. Only implemented source kinds may be offered by TableCraft. | `e640e79673502f1f7a5a20b92a4ffd85153367b2e64cf554c2b96d4db058090f` |
| [Join line](https://support.microsoft.com/en-us/access/media/join-line.jpg) | Two source cards connected at their field rows; the red ellipse is an article annotation. This is not evidence that TableCraft implements join editing. | `54153d6a4e87d37f3aeeb937818fa11f4abe3ac10785429e7f775eae65a068ea` |

Microsoft's [select-query procedure](https://support.microsoft.com/en-us/access/create-a-simple-select-query)
separates adding sources, selecting fields and running the query. Its
[criteria examples](https://support.microsoft.com/en-us/access/examples-of-query-criteria)
combine cells within a criteria row with AND and alternative rows with OR.
Clearing Show hides an output field while retaining its criteria. The visible
grid must preserve that structure rather than substitute an unrelated form.

[Join behavior](https://support.microsoft.com/en-us/access/join-tables-and-queries)
uses real relationships or qualified primary-key metadata for automatic joins;
matching names alone do not authorize a fabricated join or key glyph. Design
constructs SQL, but not every SQL query has a visual representation: Microsoft's
[SQL-specific query guidance](https://support.microsoft.com/en-us/access/run-a-query)
excludes UNION, pass-through and data-definition queries from Design view, and
unequal joins also require SQL view. A bounded TableCraft designer must refuse
unsupported conversions while preserving the original SQL and existing results.
Changing views or inspecting source metadata must not silently run a query.

These references establish selected light-layout and documented workflow facts,
not a native TableCraft interaction pass, dark-mode match or complete QBE support.
The existing explicit saved-query inspection/Run boundary remains in force.

## Published datasheet Totals references — 5 November 2026

Microsoft's [Totals-row article](https://support.microsoft.com/en-us/access/display-column-totals-in-a-datasheet-using-a-totals-row)
contains three public images, downloaded and opened headlessly from their actual
Microsoft URLs. They remain in ignored local cache rather than tracked assets.
Their filenames suggest an older Office presentation, but the exact application
build, platform and display scale are not established by the article.

| Viewed image | Observed presentation | SHA-256 |
| --- | --- | --- |
| [Initial Totals row](https://support.microsoft.com/en-us/access/media/o15-ac-totalsrow.jpg) | A separate row follows the asterisk/New action row. The first field reads Total; the other fields are blank. The pink outline is an article annotation. | `c24563115d1d2523fce84e70d494815f68a20493ea041af45e977cb65acabe63` |
| [Completed Totals row](https://support.microsoft.com/en-us/access/media/o15-ac-totalsrowcomplete.jpg) | Totals align with the existing columns and use bold text; no native record selector or additional record number appears. | `01ffc2a9dc7f53866d15a9bbf2ab5fe63ca7363de74c9b57030b455ca977c65d` |
| [Per-column selector](https://support.microsoft.com/en-us/access/media/o15-ac-totalsrowsum.jpg) | The active aggregate cell has a small dropdown. None, Sum, Average and Count are visible in this captured menu. The first field shows 4 after choosing Count instead of its default Total caption. The image does not establish the entire available menu. | `4f2d70dfa3524f59e5afa3f641ca84d621224ba2deaff8a62714f5a49fb7bfed` |

A fourth image, [the selector in Working with datasheets](https://support.microsoft.com/en-us/access/media/total1.gif),
was also downloaded and opened from that [Microsoft article](https://support.microsoft.com/en-us/access/working-with-datasheets).
Its SHA-256 is `72ea954841e183829cbd38a5786a0540a6d9fedc5d0ae1448b9bf20ab0d6eb79`.
It shows ten records, a Total row immediately below them and blank grid space
below that row. Its dropdown includes None and all seven documented aggregates.
These short-record images do not establish whether a long datasheet pins Totals
while records scroll; viewport placement during overflow remains unverified.

The article describes numeric Sum, Average, Minimum, Maximum, Count, sample
Standard Deviation and sample Variance; Date/Time supports Count, Average,
Minimum and Maximum. Text offers None and Count, also demonstrated by
Microsoft's [query introduction](https://support.microsoft.com/en-us/access/introduction-to-queries).
The detailed Date/Time entries qualify the article's shorter introductory
statement about nonnumeric columns. These are datasheet choices, not permission
to expose every SQL aggregate or extend unsupported native type profiles.

The [Microsoft Access SQL Count contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
excludes NULL for Count(column); Count(*) is a different operation. Empty strings
remain present values. The Totals row is hidden by toggling Home → Records →
Totals; it cannot be cut or deleted as a record.

The retained private Home image supplies the Ribbon placement but has Totals
off. These public images add actual Totals-row composition evidence, not native
interaction, dark-mode, filtered-view recalculation or database compatibility
proof. Computing over the complete currently filtered view remains an
implementation interpretation pending direct native comparison, rather than an
observed result from these images.

## Object navigation and tab behavior references

Microsoft's [Access user-interface guide](https://support.microsoft.com/en-us/access/guide-to-the-access-user-interface)
distinguishes selecting a Navigation Pane object from opening it by double-click,
Enter or context-menu Open. The [Navigation Pane
article](https://support.microsoft.com/en-us/access/use-the-navigation-pane)
records double-click as the default and single-click opening as a configurable
alternative. TableCraft currently implements the default selection/open
separation; it does not expose that alternate setting.

The [object-tab article](https://support.microsoft.com/en-us/access/show-or-hide-object-tabs)
documents tab activation, individual closing and view switching. Its modern
Microsoft 365 tab colors differ from the older Windows presentation described
on the same page; the retained Home/Design captures continue to govern this
workspace's visual treatment. These behavioral articles are not additional
locally observed screenshots or native interaction evidence.

Microsoft's [Run a query article](https://support.microsoft.com/en-us/access/run-a-query)
also establishes an important current difference: Access opens SELECT/crosstab
queries by executing them. TableCraft's saved-query Open remains inert inspection
with a separate explicit Run action. Object-tab fidelity must not silently
remove that execution boundary or imply full native opening parity.

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


## Datasheet display and condensed chrome — 5 November 2026

The retained Home/datasheet capture was reopened for the current corrections,
alongside the user's date/time/currency and table-close comparison. The shared
UI now centers headers and checkbox controls, distinguishes typed NULL from empty
text, supplies a table-tab Close action, and uses All Objects with a taller search
field. The condensed title/Ribbon organization is retained. A red database
cylinder and regular settings gear replace the prior app mark and irregular gear.

The title exposes a captured path on hover; its Access version is a file-profile
compatibility label. Status has a left view label and right view/zoom controls.
Windows/Linux decoration is replaced by explicit viewport requests; macOS keeps
native decoration. Headless viewport-command tests do not verify a native window
manager. The renderer removes the test harness's artificial eight-point outer
frame instead of offsetting production layout to compensate.

Regional formatting and read-action limits are described in
[display preferences](DISPLAY_PREFERENCES.md). The explicit offline editing
workflow, read-only sorted query results and visible NULL marker remain deliberate
current presentation differences from Access's inline editing and blank NULLs.
They do not establish general Access interaction or database compatibility.


The Zoom reference was opened as a 427×300 image from Microsoft's public Support
article. Its exact application build, display scale and interaction timing are
unknown. The borrowed image remains in ignored `.cache/access-public-references/`
and is not tracked or shipped. It supplies layout evidence, not a native
comparison of TableCraft's virtual text, copy, keyboard or reviewed-edit behavior.

## Compact shell follow-up — 5 November 2026

The actual retained Home image and published Zoom image were reopened for this
pass. They guide the record-strip icon order and compact position/count box,
separate grid/ruler status icons, thin zoom track, small table-tab close artwork,
Tables collapse chevron and Click to Add header. The source title's Access 2016
label is a compatibility-family convention; engine profile codes remain distinct
and do not identify the producing application.

Quick Access spacing, TableCraft branding and original vector geometry adapt
those surfaces for this application. Font customization in the current Zoom
viewer exposes its existing text-size choices, not a full native font dialog.
Copy/review controls and read-only limits remain visible. Options has no matching
Access capture in this set.

Create, External Data, Database Tools, Help, Table Fields and Table have no
corresponding retained captures. Their disabled synthetic panels are provisional
visible command previews. They must not be presented as exhaustive native
dropdowns or working macro/VBA, forms, reports, imports or administration.
Light/dark offscreen review and exact sizes are recorded in
[presentation preview](PRESENTATION_PREVIEW.md#compact-shell-record-strip-and-full-tab-previews--5-november-2026).

## Table Design inspection follow-up — 5 November 2026

The actual retained Table Design image was reopened for this pass. Its narrow
selector gutter, Field Name/Data Type/Description columns, ruled blank area,
full-width split and General/Lookup tabs guide the shared inspector. The lower
property sheet and help area have separate bounded scrollports. A field selector
triangle indicates selection; no primary-key icon is invented from unrelated
column flags. Uninspected lookup facts remain explicit.

The capture supplies light-layout evidence. Dark styling, translated compact
layouts, keyboard Copy and contextual help remain adaptations rather than native
interaction qualification. The [presentation review](PRESENTATION_PREVIEW.md#table-design-grid-and-property-inspection--5-november-2026) records
render dimensions and synthetic scenarios.

## Query Design composition follow-up — 5 November 2026

The actual published criteria and join-line images and Show Table dialog were
reopened from the ignored local cache. Compact source-card headers, vertical field
rows, visible explicit join edges, a horizontal divider and ruled QBE cells follow
those observed structures. Blank columns have no draft identity; no primary-key
glyph, relationship or join is inferred. The Add Tables dialog retains its local
user-table capability and displays unsupported source-kind tabs as disabled.

These references have no complete modern Query Design Ribbon capture or dark
mode. Existing commands and their execution boundaries are preserved; this layout
pass does not qualify the full native designer. Current synthetic scenarios are
recorded in the [presentation preview](PRESENTATION_PREVIEW.md#query-design-layout-follow-up--5-november-2026).

## Ribbon dropdown and query-command follow-up — 5 November 2026

The actual retained Home capture was reopened to compare menu disclosure marks.
Existing View, Selection, Advanced and Go To popups now use shared chevrons and
menu accessibility. Direct actions keep their existing plain-button appearance.
The Query Design mock adds only documented Builder and Show/Hide command
previews; its full grouping remains provisional without a complete retained
Ribbon image. See the [presentation record](PRESENTATION_PREVIEW.md#ribbon-menu-affordances-and-query-command-previews--5-november-2026)
for the primary command references and original Parameters artwork boundary.

## Startup, File/Open and expanded Ribbon references — 5 November 2026

Two actual Access 2016 tutorial captures were downloaded to ignored local cache
and opened for this correction. They are third-party tutorial screenshots, not
TableCraft/native comparison results; orange circles/arrows are tutorial
annotations and are not application controls. Rendered application build,
platform configuration and display scale are unknown.

| Capture | Observed composition | SHA-256 |
| --- | --- | --- |
| [Access 2016 start screen](https://www.quackit.com/pix/microsoft_access/microsoft_access_2016/howto/how_to_create_a_blank_database_in_microsoft_access_2016_1.png) | 952 × 686 image; red Recent/Open Other Files region beside a template gallery; no Back action. | `f70fffe39cf4313e11778f7735733ec29112161c3be90fa465a591166918f4b9` |
| [Access 2016 File/Open](https://www.quackit.com/pix/microsoft_access/microsoft_access_2016/howto/how_to_open_a_database_in_exclusive_mode_in_microsoft_access_2016_2.png) | 960 × 686 image; File rail, locations/Browse and recent files; Back can return to the start screen. | `02e3ef0fc4c2fb388dba941d734797d1aa29849caab1e0820ba89edc8feb2f90` |

Microsoft's [Access interface guide](https://support.microsoft.com/en-us/access/guide-to-the-access-user-interface)
corroborates the startup-without-database and File/Open workflow, but its older
version does not qualify modern pixel geometry. The retained private Account
capture continues to guide the modern full File rail. TableCraft keeps its
existing explicit path/password opening controls and local picker; recent-history,
creation, cloud locations and account services are not invented. Mock template
and maintenance commands remain visibly unavailable.

Two additional Microsoft images were opened for Ribbon coverage:

- [Access 2010 Create crop](https://support.microsoft.com/en-us/access/media/o14-ac-create-a-crop.gif): actual older Templates/Tables/Queries/Forms/Reports/Macros groups; SHA-256 `be8383425bdaa39e4bc1e3c6230420905bb0f14c5bfbdf7082e988c98d62c6a0`.
- [Query Setup crop](https://support.microsoft.com/en-us/access/media/o15-ac-addorrow.png): actual insertion/deletion rows and columns, Builder and Return arrangement; SHA-256 `49546726264c65bfbe5b536e41326187cd7b054e23fbfd5e53f7d5b85f957ce8`.

The [Ribbon inventory](RIBBON.md) records current groups and remaining
qualification. An older crop or documented command name does not establish a
complete modern Ribbon match.
