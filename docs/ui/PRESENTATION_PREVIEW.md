# Presentation preview

The synthetic preview supports screenshot-guided visual and interaction review
independently of database behavior. It reuses the shared workspace
renderer and existing view/action model. Its separate desktop host never creates
a database worker, file dialog or save worker. Synthetic responses cannot open,
query or modify a real file. Unavailable Ribbon commands remain disabled in both normal and synthetic hosts.

```sh
cargo run -p tablecraft-desktop -- --mock
cargo run -p tablecraft-desktop -- --mock --mock-view design
cargo run -p tablecraft-desktop -- --mock --mock-language ja --mock-dark
```

Other scenes cover Options, ad-hoc SQL, saved queries, attachments, multivalues, stored
history, edit drafts, reviewed edits, recovery, File/Open, About, empty tables,
loading and read errors. They review existing
presentation states; simulated validation/save outcomes are not engine evidence.
The normal optional database path conflicts with mock mode.

Saved-query Run now collects declared parameters in staged dialogs, one value at
a time. The `saved-parameters` scene starts at the first prompt for visual review;
normal inspection stays inert. Earlier inline-parameter-pane records below refer
to the preceding presentation. The current original icon inventory is 134 designs
with 536 themed/disabled SVGs, including the attachment paperclip.

File → Options selects System, Light or Dark and the interface language. These choices
apply to the current session. Selawik remains the primary font; bundled regional
Noto Sans CJK fallbacks supply Japanese, Korean and Simplified Chinese glyphs.
Font sources, hashes and OFL notices live in `assets/ui/fonts/`.

Fluent (`fluent-bundle`, from fluent-rs) handles bundled resources. Each locale
has 20 section files: `shell`, `ribbon`, `backstage`, `datasheet`, `query`,
`saved-query`, `design`, `editing`, `insertion`, `deletion`, `collections`,
`navigation`, `objects`, `diagnostics`, `settings`, `find`, `filtering`, `zoom`, `totals` and `query-design`
`.ftl` files under `assets/ui/locales/`. Languages are English, Spanish, German, Russian, Japanese
(`ja`; `jp` is accepted as an input alias), Korean, Simplified Chinese (`zh-CN`)
and Brazilian Portuguese (`pt-BR`). Catalog tests check syntax, exact message and
variable parity, formatting and fallback diagnostics. Translations are initial
engineering translations pending native-speaker review.

User/database names, SQL, stored expression text, cell contents and raw service
error details remain literal. UI language does not select database comparison,
wildcard, time-zone or regional value-format policies. Missing/invalid messages
have a diagnosed English fallback; counters retain no message arguments.

The Ribbon uses original generated SVG command icons from
`assets/ui/icons/tablecraft/`, with light/dark and neutral disabled variants.
Their generator and hash manifest retain reproducible source geometry; the
artwork uses the repository license. Earlier dated verification records below
refer to the prior Microsoft Fluent subset, which remains inert source material.

The actual retained [Access captures](VISUAL_REFERENCES.md) govern light
Ribbon/datasheet/Table Design/Backstage proportions. Published Microsoft parameter
and attachment dialog images extend the [reference record](VISUAL_REFERENCES.md#published-dialog-references--5-november-2026).
Dark mode and other uncaptured dialogs remain visual adaptations. Reference
inspection alone does not establish native interaction parity. Offscreen render evidence is separate
from native platform interaction and Windows Access behavior.

## Render review

```sh
TABLECRAFT_PREVIEW_DIR=.cache/ui-previews \
TABLECRAFT_PREVIEW_LOCALE=ja \
TABLECRAFT_PREVIEW_REGIONAL_LOCALE=ja-JP \
TABLECRAFT_PREVIEW_THEME=dark \
TABLECRAFT_PREVIEW_SCENE=options \
cargo run -p tablecraft-ui --example render_preview
```

Omit `TABLECRAFT_PREVIEW_SCENE` to export all 81 scenes. Artifacts are PNGs at
1280 × 820 logical pixels, default 1× scale, using Windows keyboard conventions on the
local renderer. Animated pending and focused-text scenes advance exactly three frames rather than waiting
for their spinners to become idle. The development-only renderer requires a GPU adapter; it does not
open an OS window. Keep generated images in ignored local output directories.

Set `TABLECRAFT_PREVIEW_WIDTH` and `TABLECRAFT_PREVIEW_HEIGHT` to review another
viewport, for example the desktop minimum of 900 × 580. Use a different output
directory to keep those artifacts separate from the default-size review.

Set `TABLECRAFT_PREVIEW_SCALE` to a finite value from 1 to 2 to review display
scaling (for example `1.25`, `1.5` or `2`). Width and height remain logical
points; a 900 × 580 frame at 2× exports 1800 × 1160 physical pixels. Scaled
filenames include an `@2x` or corresponding fractional-scale suffix so they do
not overwrite the default 1× frames. Use distinct directories for different
logical viewport sizes. Offscreen DPI simulation does not verify native monitor
switching, operating-system scaling or a platform's window chrome.

## Local verification — 4 November 2026

The coordinated presentation/host batch passes 70 shared UI tests and 23 desktop
tests, including actual theme/language selection and localized mock startup.
All eight locales × both themes × twelve scenes draw successfully with zero
localization diagnostics. Complete catalog message/variable parity, exceptional
fallback behavior, literal argument preservation and CJK glyph coverage pass.
Strict Clippy, formatting, documentation links and dependency-boundary checks pass;
the engineering scripts' 60 tests also pass. No full engine-suite rerun was needed
for this presentation-only change.

The local macOS offscreen renderer exported 192 default-size PNGs at 1280 × 820,
1× scale with Windows keyboard conventions. Representative light/dark, CJK,
long German/Russian labels, collection and edit/review surfaces were opened and
inspected. Five additional Russian scenes were exported at the desktop minimum
900 × 580; compact datasheet, history and review renders were inspected. The
review corrected field-grid width/gutters, long Ribbon wrapping, input boundaries
and collection-column widths. Overflowing Ribbon and record controls remain
horizontally scrollable rather than clipping inaccessible commands.

These are local mock/render observations, not Windows execution, screen-reader
qualification or native Access behavioral comparisons. Translations and surfaces
without retained Access captures still require the separate reviews noted above.

## Follow-on navigation and state review — 4 November 2026

That review covered sixteen scenes. Empty tables retain their
actual declared headers alongside the empty notice, with no invented new-record
row. Loading and read-error scenes preserve the last synthetic snapshot;
cancellation is local to the mock host. The default mock Table Design command
now uses the same existing ViewSchema intent as the native host and prepares its
synthetic declaration locally instead of requiring a preloaded schema.

Object navigation uses full-row hit regions and stable document/object widget
IDs, including distinct identities for a table and saved query with the same
name. Filtering earlier rows preserves focus; replacing the document discards
it. Its eight navigation messages now have their own `navigation.ftl` in every
language. Backstage has left-aligned Back/Open/About/Options SVG controls and
independent section scroll state.

The lower General property sheet is compact, full-width, left-aligned and
read-only. Its overflow scrollbar appears only when needed; actual scroll tests
reach Validation Text and the collapsed Native details. Plain Up/Down moves only
the focused current field and reveals it once, including across virtual rows.
Modified keys, property-text selection, replaced metadata and modal overlays
remain isolated. Ribbon commands similarly reveal only on gained focus; manual
scrolling is preserved. Disabled mock and background field controls expose no
Click/Focus actions and reject forced assistive activation.

The coordinated batch passes 78 shared UI tests and 23 desktop tests, including
all 256 language/theme/scene drawing combinations with zero localization
errors. The updated renderer exported all 256 default-size PNGs; representative
empty, loading, error, About, navigation and Design renders were opened for review. Strict Clippy, formatting, documentation links and dependency guards
pass. The 35 curated Fluent SVGs and retained MIT notice match their recorded
hashes; normalization preserves upstream geometry. These observations retain the
same local-render/native-platform limits as the preceding review.

## Query, collection and save lifecycle review — 5 November 2026

The preview now has 39 scenes. The additional cases use existing shared UI
transitions and synthetic responses; they add no database or task service.
Choose one with `--mock-view NAME` or `TABLECRAFT_PREVIEW_SCENE=NAME`:

| Area | Additional scene names |
| --- | --- |
| Queries | `query-running`, `query-error`, `saved-inspecting`, `saved-running`, `saved-error`, `saved-parameters` |
| Collections | `collection-loading`, `collection-error`, `collection-unassigned`, `collection-empty`, `collection-null`, `attachment-null`, `history-null` |
| Editing and saves | `edit-checking`, `edit-validating`, `edit-rejected`, `save-applying`, `save-cancelling`, `save-resolving`, `save-saved`, `save-cancelled`, `save-failed`, `recovery-diagnosis` |

Saved-query rerun/error scenes retain their previous complete sample result.
The parameter pane has an explicit overflow scrollbar; wheel tests reach its
last date input, NULL control and hint without changing bindings. Collection
scenes distinguish unassigned, assigned-empty and actual NULL members. Empty
collections retain declared headers. Initial metadata column sizing samples a
bounded display prefix while preserving complete values and user resizing.
Collection resizing is scoped to the document, parent table and field; other
parent rows of that field retain their widths without affecting unrelated fields.

Terminal save headings distinguish completed, cancelled and needs-attention
outcomes. The last heading does not imply rollback: the literal host message can
retain an established save outcome or unresolved resource-release facts.
Applying/cancelling/resolving scenes exercise existing presentation states only;
no real save, receipt or journal is created by the mock host.

The coordinated batch passes 86 shared UI tests and 23 desktop tests, strict
Clippy and formatting. All 624 scene/language/theme drawing combinations pass
with zero localization diagnostics. Local default-size macOS GPU renders were
reviewed for parameter scrolling, attachment MIME widths, empty headers and
terminal outcome presentation. These observations do not qualify native window
behavior, accessibility tools or Access compatibility. Query parameter,
collection, edit/save and dark surfaces still lack retained Access captures.

## Datasheet and View control refinement — 5 November 2026

Comparison with the actual retained Home/datasheet and Table Design captures
identified excess View-group width, left-aligned numeric values and a broad gap
above the grid. The existing two view actions now share a localized large View
dropdown. Its icon changes with the current table view; it preserves busy/query
and modal restrictions. Keyboard and assistive activation use the same existing
actions. The compact document strip joins directly to the grid.

Cells carry an explicit horizontal-alignment hint through the presentation DTO.
The runtime derives default right alignment from actual numeric/date carriers,
and desktop only maps the hint. Plain text, including numeric-looking text,
remains left-aligned; the renderer does not parse formatted strings. This adds no
reader or query capability and does not implement native TextAlign overrides.

The batch passes 87 UI tests, 23 desktop tests and three focused runtime display
tests, with strict Clippy across all three packages. All 624 scene/locale/theme
drawing combinations remain passing. The updated renderer exported 39 light
English frames; datasheet and Design were compared with the private captures.
A Russian dark datasheet was also reviewed at 900 × 580, 1× scale. View-menu tests
exercise Enter, Escape, assistive activation and existing source/busy guards.
These remain local renderer/test observations, not Windows Access comparisons.

## Long names and compact dialogs — 5 November 2026

`--mock-view long-names` supplies a synthetic 64-character table name and a long
workspace filename. Document tabs use bounded one-line captions, retain complete
literal names for hover/accessibility and keep source identity independent of
translated labels. Horizontal overflow remains scrollable; gaining focus reveals
a tab once without overriding later manual scrolling. Disabled background tabs
expose no activation/focus actions.

The title bar reserves space for its current status and truncates only the visible
workspace title. Edit and collection dialog bodies scroll within the viewport;
critical close, review, apply and recovery actions retain their existing rules
outside the variable-height content. Full literal diagnoses and edit values remain
available rather than being shortened to fit. These dialogs still have no retained
Access visual capture; their layouts remain provisional.

The batch passes 94 UI tests and 23 desktop tests, strict Clippy, formatting,
documentation links and dependency guards. All 640 scene/locale/theme drawing
combinations pass with zero localization diagnostics. Compact regressions use
actual wheel events to reveal the final diagnosis line while retaining visible
actions. One tab test initially exceeded the harness settling limit because its
hover tooltip was animating; it now moves the pointer away after scrolling before
waiting for idle, retaining every behavioral assertion.

Six local GPU frames were exported at 900 × 580, 1× scale: English light long
names, datasheet, review and attachments; Russian dark long names and recovery.
The long-name/title, ordinary title, review and recovery layouts were opened for
visual inspection. These remain macOS offscreen observations; no native Windows
or assistive-tool qualification follows from them.

## Display scaling and query keyboard review — 5 November 2026

The renderer now accepts explicit 1×–2× scale while keeping logical viewport
sizes fixed. The existing Ribbon overflow/focus test runs at 1×, 1.25×, 1.5× and
2×, retaining physical-pixel edge checks, exact glyph clipping and manual scroll
assertions. Four scaled frames were exported and their physical dimensions checked:
English light datasheet at 1350 × 870, Russian dark long names at 1125 × 725,
Japanese light Design at 2560 × 1640 and Chinese dark parameters at 1800 × 1160.
Cyrillic/CJK and existing selection/focus presentation were reviewed. The Japanese
navigation heading exposed an orphaned final character; it now truncates on one
line with its full name retained for hover/accessibility, following the Access
capture. No speculative pixel-snapping changes were needed.

Long multiline SQL is now bounded by a scrollable editor so Run remains visible.
Full SQL and the existing run shortcut remain unchanged. Actual Tab traversal of
12 saved-query prompts reproduced a focused NULL control outside the viewport;
parameter controls now reveal themselves on gained focus only. Modal Tab/ShiftTab
and Escape probes found no additional defect and retained save-owner restrictions.
Existing SQL-editor Tab indentation is unchanged. That review did not yet implement workspace pane cycling; [Microsoft's keyboard reference](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
documents F6/Shift+F6 navigation and supplies the contract for the following slice.

The batch passes 96 UI tests and 23 desktop tests, strict Clippy, formatting,
documentation links and dependency guards. The complete 640 locale/theme/scene
draw checks remain passing; the four Ribbon scales are a separate focused
geometry check. Invalid zero, oversized, NaN and infinite renderer scales refuse
before output-directory creation. Offscreen review still does not qualify native
monitor transitions, Windows interaction or screen-reader tools.

## Workspace pane navigation — 5 November 2026

F6 moves to the next available pane and Shift+F6 moves backward. The current
cycle includes Document, query Results, Design Properties, Navigation and Ribbon,
skipping absent panes. It uses the existing table/query/editor and metadata
surfaces. Focusing a pane issues no database read, query or editing action.
Ctrl+F6 and other modified variants remain available for their separate contracts.
Backstage, popup menus, disabled hosts and collection/edit modals suppress this
workspace cycle, preserving their existing keyboard and save-owner behavior.

The registry uses enabled focusable responses from the current draw. Returning
to a virtualized table/result selection reveals its original cell or record
without changing selection mode. Design entry retains a logical source/field
witness until the selected field renders, with an eight-frame retry ceiling. New publications invalidate pending
entry even when names, source revisions and page offsets match. Read-only
property values retain literal text selection and show a focus outline.
These local semantics follow the documented shortcut direction; exact Windows
traversal order remains unverified.

For visual review, `TABLECRAFT_PREVIEW_F6_STEPS=1` through `5` sends that many
actual F6 key presses after preparing the chosen mock scene. Initial scene focus
determines the starting position. Output names include `-f6-N` before any scale
suffix, preserving the unmodified scene frames. The renderer still constructs
only the resource-free mock host.

The coordinated batch passes 102 UI tests and 23 desktop tests, including actual
virtualized Table/Results selections in cell and record modes and compact Design
entry. Strict Clippy, formatting, documentation links and dependency guards pass.
All 640 locale/theme/scene draw checks remain passing. Follow-up scoped pane and
Ribbon checks passed after bounding the Design retry and adding the visible
Ribbon-tab focus outline found during review.

Six local mock frames were exported with actual F6 input: four English light
Design steps at 900 × 580, Russian dark Design at 1.25× and Japanese light Query
at 2×. Field/property and Ribbon focus outlines were inspected; the Ribbon frame
was exported again after its outline correction. An oversized renderer F6 sequence
refuses before output-directory creation. These are offscreen UI observations,
not native Windows keyboard or screen-reader qualification.

## Long service reports and failure recovery — 5 November 2026

The additional `long-read-error`, `long-query-error` and `long-saved-error` scenes
retain previous complete table/query snapshots and exact saved-query input
choices. Their multiline English reports are literal synthetic host payloads;
interface labels still use the selected Fluent language.

Workspace and saved-query failures share a bounded, full-width scroll region.
Copy details copies the complete literal report, including line breaks, quotes
and control characters, independently of the selected cell. Its interface copy
has a separate `diagnostics.ftl` section in every locale. The report remains
selectable; a failure never retries an operation or clears a complete result.
At compact sizes, SQL/parameter viewport heights reserve space for existing
Run/Inspect controls and a visible previous result.

A reproduced deferred-focus race also now preserves newer Navigation focus:
queued pane entry compares its focus witness before applying, and discards an
entry superseded by a newer focus choice. The witness is comparison-only and is
never requested as a future focus target. Failures do not introduce automatic
focus moves, reruns or result publication.

The batch passes 107 shared UI tests and 23 desktop tests, strict Clippy,
formatting, documentation links and dependency guards. All 688 scene/locale/theme
drawing combinations pass with zero localization diagnostics. Tests cover full
literal copying, actual wheel access to the report's final line, simultaneous
long SQL/report layout and 12 saved prompts with a retained visible result.
The newer-focus race first failed against the prior behavior and passes after
the comparison guard; existing virtualized pane-entry cases remain passing.

Five compact local GPU frames were exported: English light read/query/saved
failures, Russian dark saved failure at 1.25× and Japanese light query failure at
2×. Representative query/saved and Russian layouts were opened for review.
Inline failure placement remains provisional because no retained Access capture
establishes this surface. No native window, Access or screen-reader qualification
follows from these offscreen observations.

## Replacement reports and delayed pane focus — 5 November 2026

A replacement literal service report now starts at its beginning, even when its
length and geometry match the previous report. An unchanged report retains its
scroll position through theme and language changes. The existing scroll/copy
scenario exercises both transitions and copies the complete replacement text.
The replacement assertion failed against the previous scroll identity before the
fix; reports remain scoped to their owning source.

Pane entry also yields to a newer focus choice after the queued request has been
consumed but before its virtualized cell, record or Design field appears. The
comparison-only witness never becomes a restored focus target. Existing logical
selection and cell/record mode remain unchanged.

The coordinated batch passes 108 UI tests and 23 desktop tests, including all
688 scene/locale/theme drawing combinations. Formatting, strict selected-package
Clippy and the native desktop build pass. These are presentation checks, with no
new database functionality or native Access compatibility claim.

A subsequent focused Options round-trip check passes through the actual shared
File, Options, Dark, Japanese and Back controls. It retains typed saved-query
inputs, their exact retry target, both original complete result allocations and
an independent ad-hoc SQL draft. Returning to the workspace exposes the enabled
translated saved-query Run control without localization diagnostics. This is a
headless interaction check; it does not close the native locale-selection limit
recorded in the [desktop observation](DESKTOP.md#native-mock-observation--5-november-2026).

## Headless readability and localized navigation — 5 November 2026

Selected editable text now uses the normal text ink over its blue selection
background in both themes. egui uses its selection stroke color for selected
glyphs; the red accent remains on TableCraft's explicit focus outlines. A system
badge on a selected, focused navigation row likewise uses normal ink; ordinary
badges retain their muted color. The palette audit measured the former selected
text pairs at approximately 3.96:1 (light) and 4.37:1 (dark), versus 9.73:1 and
8.66:1 with normal ink. This addresses those specific pairs, not general
accessibility conformance.

A compact Russian dark saved-query scenario exercises F6 through prompts,
results, Navigation and Ribbon, plus reverse entry, at 1.25x and 1.5x. The harness
sets scale during construction and asserts an actual 900 by 580 logical viewport;
changing zoom after construction would test a smaller effective viewport.
Traversal retains the exact typed retry inputs, selected result cell, complete
result allocation and separate SQL draft, without database actions.

The coordinated headless batch passes 112 shared UI tests and 23 desktop tests,
including the existing 688 locale/theme/scene drawing combinations. Formatting,
strict selected-package Clippy, documentation links and dependency guards pass.
Focused paint checks inspect actual selected glyphs, selection fills and focus
rings in both themes, plus badge colors across focus/selection transitions.
No native application was activated or manipulated for this batch.

## Mock workflow consistency — 5 November 2026

Successful synthetic table, Design and supported SQL responses clear a prior
workspace failure report. An unsupported SQL draft still produces its explicit
mock refusal. Releasing retained synthetic save resources now yields the existing
terminal needs-attention presentation rather than claiming a completed save;
recovery remains a separate action. Neither path writes a database.

Backstage Open failures reuse the shared bounded literal report display, including
Copy details and replacement-report scroll behavior. The compact-layout check
retains the exact path/password and keeps Open/Back reachable; copying reports
never dispatches a database action. The retained Backstage capture shows Account,
so Open diagnostic placement remains an adaptation.

The normal desktop picker title/filter and host-generated cancellation/stale-view
notices now use the selected Fluent locale. Backend error payloads and database
content remain literal. Native dialog display is deliberately untested while
non-headless verification is suspended.

Closing and reopening a synthetic collection now retains its original assignment,
member count and literal NULL/duplicate payloads. Loading/error scenes retain
their explicit lifecycle state. Scene-specific children apply only to their
matching parent identity and membership; opening a different row uses that row's
ordinary fixture. Parent summaries and selection are retained.

The coordinated batch passes 117 UI tests and 23 desktop tests, including all
688 scene/locale/theme drawing combinations, strict selected-package Clippy,
formatting, documentation links and dependency boundaries. No native window or
file dialog was opened for this work, and no new database capability was added.

## Complete mock retry admission and Open render — 5 November 2026

Admitted saved-query inspection/execution and record navigation clear a previous
global cancellation report, matching the other synthetic read routes. Stale
source targets preserve the report and do not initiate a retry. Record navigation
now publishes from the exact requested ordinal rather than a rounded page
boundary. A request for record 8 first reproduced the old offset-0 failure, then
returned offset 7 with CustomerID 1008 after the correction.

The resource-free `long-open-error` scene adds a populated synthetic path, masked
synthetic password and long literal failure report to Backstage Open. The current
inventory is 44 scenes. Two current-source PNGs were exported and opened: German
light long names and Russian dark Open failure, both at 900 by 580 logical points
and 1.25x (1125 by 725 physical pixels). The Open frame keeps Copy details, path,
masked password, Open and Back visible; the diagnostic body scrolls separately.
The long-name frame retains distinct title/status areas, truncated object names
and scrollable Ribbon/record commands. These are offscreen macOS observations,
not native window, Windows or assistive-technology qualification.

The sandbox initially had no GPU adapter; the same offscreen renderer succeeded
with GPU access outside that restriction. It created no application window and
used no native UI automation. The user's suspension of non-headless checks
remains in effect.

The final coordinated batch passes 118 UI tests and 23 desktop tests, with all
704 scene/locale/theme drawing combinations and zero localization diagnostics.
Strict selected-package Clippy, formatting, documentation links and dependency
boundaries pass. No new backend functionality or native interaction qualification
is implied.

## Mock action admission and localized save reports — 5 November 2026

The synthetic host checks save ownership independently of the visible disabled
controls. Stale close/acknowledge actions must match the current request or save
identity; recovery/release additionally require their retained capability flag.
A cancellation request leaves the matching save pending and cannot overwrite an
already finished outcome or imply rollback. Terminal cancelled outcomes remain
separate explicit fixtures. Unrelated open/read actions cannot replace a save
owner before acknowledgement.

Host-authored save reports use the shared `editing.ftl` section in each locale.
Current-operation status stays distinct from the last completed receipt, including
matched/changed counts and recovery/release restrictions. This formats typed host
outcomes; it does not translate arbitrary backend errors or database content.
The [acceptance boundaries](DESKTOP.md#current-acceptance-boundaries) distinguish
source/headless evidence from the remaining native/reference/language review.

Verification covers all 122 UI tests and 23 desktop tests: 121 UI tests and the
desktop suite passed in the final coordinated run; the expensive scene-matrix
function passed separately and was deliberately skipped in that repeat. Its
704 scene/locale/theme combinations remain passing. Existing desktop receipt
checks now compare complete English and German reports, exact counts and retained
flags, with zero localization diagnostics. Formatting, strict selected-package
Clippy, local documentation links and dependency boundaries pass. Verification
remained headless throughout.

## Ribbon, title and original icon review — 5 November 2026

The title strip now uses a small original app glyph and compact mock-only disabled
Save/Undo/Redo quick access, followed by the literal document title and existing
status. Gray Ribbon tabs are separate from the inset rounded command tray;
contextual tabs retain their red styling. Object tabs now use neutral borders
and small object glyphs. The retained Access Home/Design captures were opened
while implementing these surfaces; PaintCraft's legacy Ribbon was consulted for
compact layout mechanics, without copying its artwork.

The active icon set has 49 original command compositions and 196 light/dark and
neutral disabled SVG variants. Selection, Advanced, Toggle Filter, Validation,
Lookups, Macros and Dependencies have distinct artwork. Builder and Insert/Delete
Rows are inert mock previews in the captured Design arrangement. The owning
generator reproduces all files and their SHA-256 inventory exactly; the older
Fluent subset remains inert source material with its original notice. About now
credits the original artwork and its repository license in every locale.

Seven final PNGs were opened: light/dark icon sheets at 16/32 pixels and five
assembled workspace frames. English light Home/Design and dark Home use
1280 by 820 logical points at 1x. German light Design uses 900 by 580 at 1.25x;
Japanese dark Home uses 900 by 580 at 2x. Initial assembled frames exposed
run-together Ribbon labels: frameless buttons had suppressed their inner margins.
Explicit measured tab widths now preserve side padding, and a painted-glyph gap
assertion covers File/Home/Create/External Data. The frames were regenerated and
reviewed after that correction; the earlier local frames remain separate.

The taller chrome also reproduced a compact long-SQL/result clipping regression.
The existing calculated editor height was being overridden by egui's implicit
64-point scroll minimum. Applying the existing 24-point editor minimum restores
the intended allocation; the unchanged Run/result visibility regression passes.

The final batch passes 124 UI tests and 23 desktop tests, all 704 scene/locale/theme
draw combinations, and actual decoding of every icon variant at command sizes.
Strict selected-package Clippy, formatting, documentation links and dependency
boundaries pass. No native application window was opened or manipulated. These
are offscreen macOS observations; native Windows fidelity remains unqualified.


## Published parameter and attachment dialog alignment — 5 November 2026

The additional Microsoft reference images were opened and their source URLs and
hashes recorded in the [visual reference record](VISUAL_REFERENCES.md#published-dialog-references--5-november-2026).
Shared saved-query Run now starts a source-bound staged session. Each OK advances
one declaration; only the final confirmation commits changed inputs and emits
the existing typed run action. Parameterless runs remain direct. Cancel/Escape
preserve original inputs, complete results and selections. Publication, target,
declaration and original-input witnesses refuse stale acquisition.

Literal prompt labels, one input and a right-aligned OK/Cancel footer follow the
published layout. Explicit VALUE type and NULL controls retain the current typed
profile. Numeric/date content validation still happens at query run; no prompt-time
validator, implicit unresolved-name parameter behavior or Help action is added.
An over-limit draft retains the existing input-limit explanation and reachable
Cancel. Enter belongs to the actual focused widget: value submission advances,
Cancel dismisses, and the type chooser owns its popup keys. Background F6 and
query shortcuts stay gated while acquiring input; backdrop clicks keep it open.

The attachment metadata viewer now has a localized heading and original paperclip.
Its existing filename-first metadata, NULL/assignment distinctions and Close action
are unchanged. Loading-to-Ready checks reproduced a shifted Close identity and a
stale short body clip. An explicit request-bound Close scope preserves focus, and
bounded body remeasurement keeps declared headers and rows visible. Attachment
opening, exporting and mutation remain outside this metadata-only view.

Six final offscreen PNGs were opened: English light unprompted saved query,
parameter acquisition and attachments at 1280 by 820 / 1x; German dark prompts at
900 by 580 / 1.25x; Japanese light prompts at 900 by 580 / 2x; Russian dark
attachments at 900 by 580 / 1.5x. Initial prompt frames exposed ineffective footer
alignment. Measured localized button widths and a flexible spacer now align the
painted footer with the input while retaining OK-to-Cancel keyboard order. The
three prompt frames were regenerated after that fix; initial local frames remain
separate. Literal synthetic parameter names and file metadata stay untranslated.

The final batch passes 127 UI tests and 23 desktop tests, with all 704 scene /
locale / theme combinations and the 50-design icon decode inventory. Formatting,
strict selected-package Clippy, documentation links and dependency boundaries
pass. All source changes remain in presentation; host service contracts and
backend operation scope are unchanged. No native window or UI automation was used.


Regional formatting in exported previews is selected independently with
`TABLECRAFT_PREVIEW_REGIONAL_LOCALE` (default `en-US`). It is not inferred from the
Fluent translation locale. The exporter uses a full-viewport root UI instead of
the test harness's artificial eight-point demonstration frame. Record logical
size, pixel scale, build platform and chosen regional locale with reviewed images.

## Explicit new-row presentation

The `new-record` and `insert-review` scenes show the same bounded row-insertion
modal used by the desktop host, with clearly labeled synthetic fields and no
file persistence. Home and the record toolbar expose New row without adding an
invented stored row, generated value or native row ordinal to the datasheet.

Each field distinguishes Omitted, NULL and Value. Changing the choice retains
its text; an accepted review binds the exact current inputs before applying.
Save cancellation, retained resources and acknowledgement use the existing
single save owner. Backend admission remains responsible for supported types,
constraints and native insertion; the dialog does not assume defaults or skip
required programs. This explicit offline dialog is a provisional application
workflow, not a verified replica of Access's inline new-record row.


The `delete-checking` and `delete-review` scenes exercise the existing reviewed,
offline single-row deletion UI with synthetic source-bound rows. Home's Delete
placement follows the retained Access Home Ribbon capture. No deletion confirmation
capture is retained: this bounded confirmation layout remains provisional. It
shows the actual reviewed row and count, initially focuses Cancel, and exposes no
implicit acceptance on Enter. Native Access deletion interaction has not been
verified by these headless previews.


The `find` and `find-searching` scenes expose modeless Find controls and a bounded
synthetic pending state. They never search a file or manufacture a match. Native
Find remains host-capability gated; query-result Find and Replace are not
implemented. The public Microsoft Find image in the [reference record](VISUAL_REFERENCES.md)
guides control arrangement; no native interaction or exact scale is inferred.


The `zoom` scene shows the shared resizable read-only viewer over an explicit
synthetic scalar. Sparse virtual rows make long-value review possible without a
full per-frame text layout. The source has an explicit 8 MiB UTF-8 ceiling;
complete Copy retains original line breaks, and eligible native edits delegate
to the existing reviewed workflow. The [public Zoom reference](VISUAL_REFERENCES.md#published-dialog-references--5-november-2026)
was opened headlessly; its blue article annotations are not UI assets. This scene
provides presentation review, not native Access behavior verification.


The `filtered`, `filter-empty`, `filter-disabled` and `filter-error` scenes use
predetermined synthetic memberships over the invented fixture. They show retained
counts, original native ordinals, empty results, temporarily disabled criteria and
failed reads that preserve the previous view. Their explicit synthetic notice is
not a native filtering or mutation result.


The `sorted` scene shows predetermined descending Customer ID membership over the
complete invented fixture. It exercises an editable native-view presentation,
original physical identities, zero filter criteria, the heading sort glyph and
real Remove Sort intent. Its explicit synthetic notice supplies no native
sorting, editing or save/reopen comparison evidence.

The German light and dark `sorted` frames were exported and opened on macOS at
1000 × 680 logical points and 1.25 pixel scale (1250 × 850 pixels), using `de-DE`
regional formatting. The local review files are
`.cache/ui-native-sort-review/de-light-sorted@1.25x.png` and
`.cache/ui-native-sort-review/de-dark-sorted@1.25x.png`. Both show descending
Customer ID values, the header direction glyph, enabled Remove Sort and disabled
filter controls for the zero-criterion view. The unfiltered count and sort status
remain readable in both themes. Rendering used the offscreen GPU exporter; no
native application window or Access comparison was involved.

## Independent object tabs — 5 November 2026

The `objects` scene opens two tables, a saved-query inspection and the scratch SQL
editor, then reactivates Customers without a native worker. Each tab has its own
close control and stable identity; the ordinary datasheet scene starts with just
the actual Customers object and the functional SQL opener. Neither scene creates
a placeholder Query or separate Design object. The current 57 × eight × two
matrix passed (912 combinations), alongside focused lifecycle/layout checks.

German/de-DE objects and datasheet exports were inspected in both themes at
1000 × 680 logical pixels, 1.25 scale (1250 × 850 physical pixels), using the macOS
offscreen GPU renderer. PNGs remain ignored at
`.cache/ui-objects-review/de-{light,dark}-{objects,datasheet}@1.25x.png`.
The retained actual Home capture was reopened. Compact tab placement and close
controls were compared; no exact retained mixed-object or dark-mode Access
capture exists. No native window was opened, and these images do not establish
platform interaction or native database compatibility.

## Unloaded object states — 5 November 2026

The `object-loading` and `object-error` scenes have a real opened synthetic Orders
tab without a row publication. They distinguish that state from loaded-empty
results and retain the complete existing diagnostic renderer. Retry dispatches
only synthetic responses in mock mode; it has no database worker. Loading is
explicitly animated for bounded offscreen frame export. The current matrix is
59 scenes × eight locales × two themes (944 combinations).

German/de-DE light/dark loading/error images were rendered and inspected at
1000 × 680 logical pixels, scale 1.25, using macOS offscreen GPU output. PNGs stay
ignored under `.cache/ui-object-status-review/`. There is no matching retained
Access state capture; the exact presentation is provisional. No native window
was opened. The preview-only animated classification was verified by successful
bounded loading-frame export after the full functional/matrix gate.


## Field filters and datasheet polish — 5 November 2026

The inventory now contains 61 scenes, adding `field-filter` and
`field-filter-error`. The complete 976-combination matrix passed with eight
languages and both themes. All 194 UI checks passed; the final memory-layout and
title-command presence follow-up passed 193 functional checks with only the
already-passed matrix omitted. Synthetic filter Apply deliberately returns a
retained mock error rather than pretending to evaluate native predicates.

Ten German/de-DE PNGs were exported and opened under ignored
`.cache/ui-polish-review/final/`: Datasheet, Options, field-filter, field-filter
error and empty filtered table in both themes. They use 1000 × 680 logical
pixels at 1.25 scale (1250 × 850 physical pixels), macOS offscreen GPU rendering
and Windows keyboard conventions. Native window controls are platform-compiled;
these macOS frames do not qualify Windows/Linux title controls. The unprivileged
renderer had no GPU adapter; the same windowless renderer succeeded with local
GPU access. No application window was opened or activated.

The revised gear/cylinder, compact shell, formatting examples, NULL marker,
centered captions, retained error input and New row/empty-message separation were
visually reviewed. The actual retained Home capture was reopened. There is no
retained Access Options/common-filter/dark-theme capture, so those presentations
remain adaptations rather than verified pixel matches or native interactions.


## Datasheet Totals — 5 November 2026

The inventory is 63 scenes, adding `totals` and `totals-error`. Each language now
has 19 section files, with a dedicated `totals.ftl`. Totals scenes contain fixed
invented complete-view results; choosing an unsupported synthetic computation
returns a clearly labeled mock failure and never calculates over displayed rows.
The final 1,008-combination matrix passed; together with the final 197 functional
checks this establishes 198 UI checks.

German/de-DE light/dark Totals and Totals-error frames were exported and opened
at 1000 × 680 logical pixels, scale 1.25, using macOS offscreen GPU rendering
and Windows keyboard conventions. Artifacts remain ignored in
`.cache/ui-totals-review/`. Selected toggle outline, real column widths, bold
numeric alignment, reachable overflow totals and retained error controls were
reviewed. Final error frames were re-exported after removing duplicate mock text.
No application window opened. The four viewed Microsoft references establish
short-table placement and operation choices; adaptive overflow placement remains
provisional. See the [feature contract](DATASHEET_TOTALS.md).

## Visual SELECT design — 5 November 2026

The inventory contains 66 scenes. The new `query-design`,
`query-design-tables` and `query-design-error` scenes render authored synthetic
drafts without querying a database. The dedicated `query-design.ftl` brings each
locale to 20 sections. Run and Add Tables have original generated SVGs, bringing
the icon inventory to 52 concepts and 208 themed/disabled files.

Six German light/dark PNGs were exported and opened at 1000 × 680 logical points,
scale 1.25 (1250 × 850 physical pixels), with `de-DE` regional formatting on the
macOS offscreen renderer using Windows UI conventions. Actual Microsoft source
card/QBE and Show Table images were reopened for comparison. Explicit vertical
card layout and a bounded upper pane fix a rendered horizontal-field overflow;
a 900 × 580 interaction check now observes field positions and visible criteria.
The table chooser uses a full-width bounded list and right-aligned Add/Close.

The full 66 × eight × two drawing matrix passes (1,056 combinations). Native
window-manager behavior and Windows Access interaction remain unverified; joins
use the documented context-menu workflow rather than native drag-and-drop.

## Find and Replace — 5 November 2026

The current inventory is 68 scenes. `replace` and `replace-error` add the
modeless Find/Replace tabs, literal replacement input, explicit Review/Apply
explanation and unavailable atomic Replace All. These are synthetic controls;
the preview never verifies a native match or manufactures an edit capability.
Active Find/Replace carets use the renderer's bounded animated-scene frames.

All 202 UI tests pass, including 68 scenes × eight locales × two themes
(1,088 combinations). Four German light/dark replacement and error PNGs were
exported and opened at 1000 × 680 logical points, scale 1.25 (1250 × 850 physical
pixels), using `de-DE` on the macOS offscreen renderer with Windows keyboard
conventions. Inputs, tab labels, right-side buttons and error reports remained
visible. The actual Microsoft Find dialog image was reopened; it depicts only
the Find tab, so the replacement panel remains a documented visual adaptation.
No native window or Windows Access interaction was exercised.

## Compact shell, record strip and full-tab previews — 5 November 2026

The inventory is 74 scenes. `ribbon-create`, `ribbon-external-data`,
`ribbon-database-tools`, `ribbon-help`, `ribbon-table-fields` and `ribbon-table`
show the six newly selectable synthetic tabs. Their grouped commands are disabled
and cannot dispatch backend work. Existing Home and Design commands retain their
real service routes. The new panels are representative visible command previews;
individual dropdowns and native behavior are not fully reproduced.

The title starts with the red database mark and TableCraft, followed by spaced
Quick Access controls. Open uses the existing file chooser; customization changes
only session visibility. Save/Undo/Redo remain explicitly unavailable document
services. The source path stem supplies the first document name and Database is
the generic kind label. Both ACE 16 storage families use the Access 2016 title
convention; raw Ace2016/Ace2019 metadata remains distinct. Currency's System locale
choice includes its resolved regional currency code, independently of UI language
and a field's native currency format.

The record strip uses distinct First/Previous/Next/Last/New icons, an editable
record position, checked count, filter state and search. Unknown complete counts
show `?`; a loaded prefix is not a total. Existing page controls and full filter
counts remain in Page tools. Search currently opens the existing Find workflow;
in-place Access quick-search parity is not qualified. Status views use separate
datasheet/ruler geometry.
The zoom track has a narrow handle and no redundant label, with 100% centered.
Home Zoom presents its body first and Font/OK/Cancel below; existing read-only,
Copy and reviewed-edit boundaries remain explicit.

Tables can collapse without changing selected objects or saved-query visibility.
The smaller object-tab X retains its original hit target. Native datasheets append
a disabled Click to Add header after stored fields. Its blank body is outside
Data, selection, copying, sorting, totals and row identity. Field creation is not
implemented by this placeholder. Wide tables reveal it through horizontal scroll.

The original artwork inventory is 103 concepts / 412 variants. Dark strokes and
accents are brighter; neutral disabled Ribbon artwork keeps its opacity while
commands retain disabled input/accessibility state. Font dropdown chevrons use
widget geometry rather than unsupported font characters.

Across the coordinated gate and focused follow-ups, all 207 UI and 63 desktop
tests pass, including the final 74 × eight × two matrix (1,184 combinations).
Strict UI/desktop Clippy, workspace formatting, local documentation links,
dependency boundaries and all 412 manifest hashes were checked.

Light/dark Home, Zoom, Options, Query Design and all six new tab PNGs were opened
at 1600 × 900 logical pixels, 1× scale, with en-US regional policy. German dark
Home and Zoom were also opened at 900 × 580 and 1.25× (1125 × 725 physical pixels).
All used macOS offscreen rendering with Windows keyboard conventions; no native
window opened. The retained Home and published Zoom images were reopened for
comparison. Matching Access captures do not exist for the six added panels, dark
mode or Options, so those remain visual adaptations. These checks establish
headless presentation and existing service routing, not Windows Access parity.

## Table Design grid and property inspection — 5 November 2026

The retained Table Design capture was reopened for the selector gutter, column
proportions, full-width divider and lower General/Lookup property area. The grid
keeps actual field selection separate from inert blank rows. Its divider supports
dragging and focused Up/Down keys; F6 enters the current field or a read-only
property value. Selected properties provide contextual help. Lookup remains
explicitly uninspected, without inferred defaults or primary-key artwork.

Keyboard Copy publishes the complete focused observation, including text clipped
in the cell, subject to a 1 MiB ceiling. Explicit text selections keep normal
subset-copy behavior. Copy failures remain visible; schema replacement cannot
retarget an old property focus. These controls inspect metadata and never edit
the schema or evaluate stored expressions.

`design-long` adds 66 synthetic declarations, long literal metadata and mixed
observation states. `design-uninspected` contrasts missing, absent, uninspected
and unsupported properties. Both use the existing metadata model; no database
service or native behavior was added.

Local offscreen review uses en-US light/dark at 1600 × 900 logical pixels and
German dark at 900 × 580 logical pixels, 1.25× scale, on the macOS GPU renderer.
Artifacts remain ignored under `.cache/ui-table-design-review/` and
`.cache/ui-table-design-minimum/`. These renders do not qualify native window,
assistive-technology or Windows Access interaction behavior.

## Query Design layout follow-up — 5 November 2026

The actual published criteria, join-line and Show Table images were reopened for
this pass. Source cards now have compact header bands and full-width field rows;
only explicit existing join edges are drawn. A full-width divider supports
dragging and focused Up/Down keys. The lower QBE sheet uses ruled cells, centered
Show checkboxes and inert blank columns outside the draft model. Field expressions,
sorts and AND/OR criteria retain the existing bounded local edit and explicit
canonicalization/Run workflow. Changing layout never builds or executes SQL.

Add Tables now follows the reference's tabs, list and right-aligned footer.
Queries and Both remain disabled with an explanation because this source picker
currently supports only local user tables. Opening the dialog stays metadata-only;
Add and target-bound double-click retain their existing service requests.

`query-design-empty` shows an authored empty editor without invented sources or
results. `query-design-wide` has 32 bounded synthetic projections, eight criteria
rows, a hidden criterion column and an explicitly authored two-source join. These
are presentation fixtures, not evidence of native Access execution.

Offscreen review used eight en-US frames at 1600 × 900 logical pixels, 1×,
and six German light/dark frames at 900 × 580 logical pixels, 1.25×, on the
local macOS GPU renderer. Artifacts remain ignored in
`.cache/ui-query-design-polish/` and `.cache/ui-query-design-polish-minimum/`.
The review caught and corrected a blank-label checkbox offset and translated
Ribbon-caption overlap. Native window checks remain suspended.

The final UI gate passed all 218 tests, including 78 scenes × eight languages ×
two themes (1,248 drawing combinations). The earlier host batch passed all 63
desktop tests. Strict UI/desktop Clippy, formatting and dependency/documentation
checks passed. Interaction cases preserve full OR rows and whole oversized-input
refusal, exact Add Tables requests, bounded double-click targeting, blank-cell
inertness and keyboard splitter movement without query actions.

## Ribbon menu affordances and query command previews — 5 November 2026

The actual retained Home image was reopened for menu indicators. View, Selection,
Advanced and Go To now share a menu-aware renderer. Small commands reserve a
separate arrow slot; large menus reserve space below their complete translated
label. Plain Run/Find/Zoom actions do not acquire a false dropdown contract.
Menu accessibility retains enabled state and the actual popup's expanded state.

The synthetic Query Design Ribbon adds disabled Builder, Totals, Parameters and
Property Sheet commands. They have no accessible Click/Focus action, do not
dispatch even after forged activation, and are absent from the ordinary host.
The existing View, Run and Add Tables actions retain their original workflows.
Parameters uses original query-panel/control artwork, with light/dark and neutral
disabled variants. The generator and manifest now describe 104 concepts and 416
SVGs. No backend functionality or query-builder capability was added.

Microsoft's [basic-query training](https://support.microsoft.com/en-us/access/training/video-create-basic-queries),
[query introduction](https://support.microsoft.com/en-us/access/introduction-to-queries) and
[lookup query guidance](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries)
support the command/group associations. No complete modern Query Design Ribbon
capture is retained; its added grouping and density remain provisional. The
private Home capture supplies light-layout evidence, not dark or native
interaction qualification.

Offscreen review covered eight frames: en-US light/dark Home and Query Design
at 1600 × 900 logical pixels, 1×, and German light/dark versions at 900 × 580
logical pixels, 1.25×, on the local macOS GPU renderer. Ignored artifacts are
in `.cache/ui-ribbon-menus/` and `.cache/ui-ribbon-menus-minimum/`. The Parameters
icon remains distinct from the adjacent Property Sheet and Builder artwork at
actual small-command size. Native window checks remain suspended.

The coordinated final gate passed 220 UI tests and 63 desktop tests, including
all 1,248 scene/language/theme combinations. Strict UI/desktop Clippy, formatting,
dependency/documentation checks and all 416 manifest hashes passed. This evidence
qualifies the local shared presentation and mocked dispatch boundaries; it does
not qualify native Access interactions or backend operations represented by
disabled commands.

## Startup, capability-aware footer and expanded Ribbon — 5 November 2026

`startup`, `startup-open` and `templates` extend the synthetic inventory to 81
scenes. Startup has no database, no document Back action and no inactive view or
zoom controls. File/Open, Options and About can return to startup; File pages
with an open database return to the current workspace. The mock New page and
unavailable file commands perform no creation, save, print or account operation.
Recent history stays explicitly unavailable rather than displaying invented
user files. The original manual path/password opening controls remain working.

Footer view buttons now require an admitted change to another view of the exact
active object. Percentage zoom requires a visible datasheet with columns, even
when it has zero records; design-only, startup, Backstage and unloaded surfaces
hide the section. This does not change the separate cell-value Zoom dialog.

The Ribbon pass extends visible mock groups and command artwork throughout the
covered tabs. See the [explicit inventory](RIBBON.md) for current coverage and
remaining native galleries/keyboard/capture qualification. All added command
previews and group launchers remain disabled and have no dispatch authority.

The mock Home Select/Refresh All/Zoom popups delegate only to existing commands;
opening a menu alone does not run an action. Unsupported bulk selection and
refresh-all entries remain disabled. Group captions and launchers have separate
paint bounds, with a shared compact command baseline for two-line menus.

Final offscreen review exported 26 en-US light/dark frames at 1600 × 900 logical
pixels, 1×, covering startup, File/Open, New and all currently selectable Ribbon
surfaces. Eight German frames at 900 × 580 logical pixels, 1.25×, cover startup,
File/Open, Home and Query Design. They were rendered on the local macOS GPU
without native windows. Ignored artifacts are in `.cache/ui-startup-ribbon/` and
`.cache/ui-startup-ribbon-minimum/`.

The final UI gate passed all 224 tests, including 81 scenes × eight languages ×
two themes (1,296 drawing combinations). The host batch passed all 63 desktop
tests. Strict UI/desktop Clippy, formatting, dependency/documentation checks and
all 536 original SVG hashes passed. Actual interaction tests cover meaningful
startup return paths, hidden unusable footer controls, mock menu delegation,
whole disabled-command refusal and separate caption/launcher geometry. These
checks do not establish native Access compatibility or implement disabled tools.

## Schema type labels and translated File rail — 5 November 2026

All eight Design catalogs now distinguish declared presentation names, reader
carrier descriptions and host-authored Field Size summaries. Existing synthetic
scenes use these labels without changing their source metadata or stored text.
Unknown labels stay literal; Native Details keeps exact diagnostic identifiers.
See the [schema label contract](DESKTOP.md#localized-schema-presentation-labels).

File rail width is measured from the complete translated interface labels, with
separate allowance for cell padding, the panel separator and configured scroll
gutter. This preserves wording such as German Save As instead of abbreviating
it. The minimum/maximum width remain bounded and existing navigation is unchanged.

Offscreen review uses all eight languages in light/dark Design at 1600 × 900
logical pixels, 1×. German and Japanese Design/File/Open also use 900 × 580 logical
pixels at 1.25×. Artifacts remain ignored in `.cache/ui-type-labels/` and
`.cache/ui-type-labels-minimum/`; the local renderer is macOS. Native windows
remain suspended and these images do not qualify Windows Access interaction.

All 227 UI tests passed across the batch and focused follow-ups, including the
final 1,296 scene/language/theme matrix. Strict UI/desktop Clippy, formatting and
dependency/documentation checks passed. Review covered 16 Design frames at
1600 × 900 and eight compact German/Japanese Design/File frames at 900 × 580,
1.25×. The full-label regression pays the actual panel separator reservation
rather than weakening the non-elision check.


## Normal-host Ribbon visibility — 5 November 2026

Normal and synthetic hosts now share the complete Home strip, including Text
Formatting, Cut, Format Painter, Save, Spelling, More and both group launchers.
Unavailable controls expose localized “Not available yet” tooltips and accessible
descriptions. Native captions Paste, New and Delete retain the existing narrow
reviewed workflows; normal Replace still requires its actual capability.
Main tabs and contextual Query Design groups also remain visible outside mock mode.

The resource-free offscreen renderer accepts `TABLECRAFT_PREVIEW_HOST=normal` to
use synthetic data with the normal presentation flag. It opens no database or OS
window. Set a separate output directory to distinguish these observations:

```sh
TABLECRAFT_PREVIEW_HOST=normal TABLECRAFT_PREVIEW_SCENE=datasheet \
TABLECRAFT_PREVIEW_WIDTH=1600 TABLECRAFT_PREVIEW_HEIGHT=900 \
TABLECRAFT_PREVIEW_THEME=dark TABLECRAFT_PREVIEW_DIR=.cache/ui-previews/normal-ribbon \
cargo run -p tablecraft-ui --example render_preview
```

The retained Windows Access Home image and the user's comparison were reviewed.
Normal-presentation light/dark frames were rendered offscreen on macOS at
1600×900, 1×, with Windows chrome emulation. A German dark frame used 900×580
logical points at 1.25×; the overflowing Ribbon remains horizontally scrollable.
These renders establish visible coverage, not native Access interaction parity.
Native window verification remains suspended.

Verification: the selected desktop suite passed all 63 tests. The UI batch passed
229 tests, including the 1,296 scene/language/theme matrix; its new Home inventory
test initially selected both Quick Access and Ribbon Save. After selecting the
Home control explicitly, that focused test passed, covering all 230 UI tests
across the batch and follow-up. Strict UI/desktop Clippy, formatting, documentation
links and dependency boundaries passed. Disabled formatting controls have explicit
names/values and reject unsolicited accessibility activation. Existing Copy,
record selection, refresh, editing and query workflows retain their checks.


## Compact Home Ribbon — 5 November 2026

The default 1280×820 normal-host Home frame now fits the complete English strip.
View/Zoom command widths and caption gutters are tighter; font selectors and
formatting icons are compact, with six visible subgroup dividers across the two
formatting rows. Refresh All's chevron is beside All rather than below the whole
caption. Its caption lines are centered. Delete opens a real menu before the
existing Delete row review action; changed target/publication/capability closes
its old menu. Native layout evidence comes from the retained Home capture, not an
opened Delete-menu capture.

Headless regressions inspect actual command bounds, divider geometry and caption
arrow placement at 1×, 1.25×, 1.5× and 2×. The default-size light/dark frames were
rendered offscreen on macOS with Windows chrome emulation. Other languages can
still overflow; existing pointer scrolling and focused-command reveal remain.
An attempted keyboard extension to egui's nonfocusable native scrollbar did not
pass its focused test and was excluded. No keyboard-scrollbar claim is made.

Final verification passed all 234 UI tests (including the 1,296 scene/language/theme
combinations), all 63 desktop tests, strict selected Clippy and formatting. Link
and dependency-boundary checks passed. A 900×580 German dark frame at 1.25× also
retained complete translated captions with pointer overflow; native window checks
remain suspended.


## Ribbon display states and command discovery — 5 November 2026

`ribbon-collapsed` retains the tabs and expands the document's available height;
`ribbon-temporary` overlays the ordinary commands without moving the records.
Both synthetic scenes remain resource-free and can exercise normal presentation
with `TABLECRAFT_PREVIEW_HOST=normal`. The trailing control, Ctrl+F1 and tab-owned
pointer pairs change only session UI state. Pin restores the expanded strip.
The source contracts and uncaptured Access interaction limits are in
[Ribbon coverage](RIBBON.md#expanded-tabs-only-and-temporary-display).

The full selected batch passed 241 UI tests, including the 83-scene × eight-locale
× two-theme matrix (1,328 combinations), and all 63 desktop tests. An additional
rapid-click case reproduced a wrong toggle after clicking Collapse followed by
Home twice; typed same-tab pairs fixed it without accepting the prior control's
click as part of the pair. All seven focused display tests passed afterward,
covering all 242 UI cases across the batch and follow-up. Strict selected Clippy,
formatting, documentation links and dependency boundaries passed.

Focused tests cover actual Ctrl+F1/tab clicks, child-menu Escape ownership,
outside record selection, F6 dismissal, object changes, Backstage/modal guards,
working Refresh dispatch, pinning and loaded-row identity. An English/German
actual hover test also verifies translated names on icon-only commands and the
unchanged unavailable message. No new native write or query functionality was added.

Default 1280×820 normal-host collapsed/temporary frames were rendered offscreen
in light and dark themes on macOS with Windows chrome emulation at 1×. The
expanded Home strip still fits that window; a German temporary frame used
900×580 at 1.25× with horizontal overflow. Native app windows were not opened.


## Caption chevrons, font catalog and replacement artwork — 5 November 2026

View/Paste/Zoom arrows now follow the caption instead of the shared command
bottom; the geometry regression observes all three plus Refresh All's inline
arrow. Family and size selectors use equal bounds at 1×, 1.25×, 1.5× and 2×,
with a seven-point second-row gap. An initial offscreen frame exposed separators
stretching the top row, so its height is explicitly bounded before layout. The
complete English strip retains its default-window width and height limits.
Refresh All and Format Painter now have original stacked-record/refresh and
broad-bristle artwork; all 536 generated icon variants decode successfully.

Fontique discovers platform families once in the desktop host. The local macOS
headless probe observed 451 system names and 455 merged families, retaining the
four bundled families. Its cache retains one shared family allocation. This is
local discovery evidence, not Windows/Linux qualification. The pure UI catalog
admits bounded names and browses virtualized rows; a 500-family regression checks
one scrollbar and visible-row rendering. Applying font formatting is unavailable;
Selawik stays selected and no system font binary is imported into egui.

Paste Special and Paste Append are visible disabled choices. Paste text uses the
existing exact-target review intent. The first menu approach tied clipboard focus
to a disappearing/disabled Ribbon control; the final bounded UI intent focuses
the actual rendered cell first. A current-frame cell stamp avoids replaying egui
interaction through `Context::read_response`. Normal and temporary Ribbon tests
retain delayed clipboard ownership and the original literal payload without
weakening source/focus checks or requesting a remembered widget ID.

The full batch passed 246 UI and all 64 desktop tests. Its combined menu case
initially selected the same row twice across Paste/Delete subcases; after making
that precondition actually change DeleteTarget, the focused case passed. A new
large-catalog case also passed, covering all 248 UI cases across batch/follow-ups.
Final strict selected Clippy, formatting, documentation links and dependency
boundaries pass. CLI normal dependencies exclude Fontique; its CLI-specific
license bundle is unchanged. A future desktop archive needs desktop dependency
notices. No native app windows were opened.

Normal-host offscreen light/dark Home frames were reviewed at 1280×820, 1× on
macOS with Windows chrome emulation. The icon contact sheet also reviewed light,
dark and disabled variants at 16, 28 and 56 pixels. Native reference evidence
remains the retained Windows Access Home image. Font-picker layout is a UI
adaptation without a retained Access picker capture.


## Font-dropdown search — 5 November 2026

The requested search field is the first dropdown item and receives focus on open.
Names filter case-insensitively as text changes; clearing restores the catalog,
empty results have a localized message and reopening starts with an empty filter.
The search is capped at 128 characters and stays above the virtualized list.
Applying font formatting remains unavailable. Search labels and empty-state text
are translated in all eight Ribbon catalogs.

Four focused font tests and four localization tests pass, plus strict UI Clippy
and formatting. Actual input tests cover English/German focus, field clicks that
keep the dropdown open, filtering, empty results, clear and reopen. The existing
500-family test retains one scrollbar and visible-row rendering. No broader
backend or desktop suite was rerun for this presentation-only change.

Offscreen normal-host light/dark frames at 1280×820, 1× on macOS with Windows
chrome emulation were reviewed with a `noto` filter. Use the existing renderer
with `TABLECRAFT_PREVIEW_SCENE=datasheet` and
`TABLECRAFT_PREVIEW_FONT_SEARCH=noto`; output uses the `-font-search` suffix.
Standalone previews use bundled font names. This requested picker extension has
no retained Access capture, and no native app windows were opened.


## Parameter input guards and settings labels — 5 November 2026

Saved-query parameter prompts now reject oversized Text, Paste and IME insertions
whole before editing/layout. The existing 1 MiB aggregate admission includes names,
kinds and all other literals; it is not reduced to the standard small editor's
64 KiB scalar allowance. The old staged value and committed parameters/results
remain unchanged on rejection. The existing input-limit diagnostic stays visible
and OK/Enter refuse until an actual text/type/NULL correction. Other editor
profiles retain their previous limits through the shared helper's default entry.

Actual-widget tests cover mixed same-frame events, delayed correction and the
exact 1 MiB boundary followed by one extra byte. A separate actual Cancel→pointer
value-focus plus oversized paste in one frame also passes unchanged production;
the guard already sees egui's resolved focus before editing. This test checks
both the real focus handoff and unchanged literal/diagnostic/refused confirmation.

Language/date/time/currency selectors now reference their existing visible label
nodes. German/Japanese AccessKit tests resolve each by its setting name while
retaining values, locale, loaded rows and an empty action queue. This does not
establish native screen-reader traversal. Options remains an uncaptured Access
layout adaptation; parameter layout retains its recorded public reference.

The font-popup workspace regression also passes actual typing/clipboard paste,
F6 ownership, nested/parent Escape while busy, outside record clicks and object
switching without SQL/native-selection mutation or database actions. No new
backend functionality is supplied by this slice.

The selected batch passed all 253 UI and 64 desktop tests, including the
1,328 scene/language/theme matrix; the additional focus-acquisition regression
then passed, covering all 254 UI cases across the batch/follow-up. Strict selected
Clippy, formatting, links and dependency guards passed. Four normal-host Options
and saved-parameter frames were rendered offscreen in light/dark at 1280×820, 1×
on macOS with Windows chrome emulation; representative light Options/dark prompt
frames were inspected. No native app windows were opened.
