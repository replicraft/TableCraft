# Native desktop preview

The first implementation provides native startup, Selawik workspace, table
navigation, bounded datasheets with scalar/stored-field cells and explicit
read-only SQL. Runtime tests
exercise real reads/queries and cancellation; headless rendering covers empty and
populated states. Native visual/platform verification is recorded below. Broader
editing and application designers remain work.

Run with `cargo run -p tablecraft-desktop -- path/to/example.accdb`, or omit the
path and use File → Open. Password input is masked and is consumed/erased
when opening. Original database files are opened for bounded snapshot reads;
close other writers before opening. The preview does not provide a live shared
file editing session. It opens files up to 64 MiB, buffers pages of 100 rows and
uses separate native/query/display work limits. Limits are operation ceilings,
not a guarantee of an exact process memory cap. Unsupported tables/queries show
errors rather than omitting fields or running providers.

## First working scope

Start with a native Access file browser: explicit file selection, inert catalog
inspection, table navigation and bounded scalar datasheets. Add a SQL editor and
explicit read-query execution over the same selected stable/offline source. Show
actual results and typed errors from the existing services. Opening a database or
selecting an object must never execute stored queries, expressions or active
content implicitly.

Use `NativeBrowser` for table pages and `projected_page` for explicitly selected
scalar columns. Retain complete browsing defaults. Existing source-bound
multivalue, attachment and stored-history readers can supply requested parent
panels as connected follow-on slices; opening a panel must not turn a selected
payload result into complete collection membership. Attachment FileData stays
stored and encoded until the user explicitly requests decoding. Visible current
Memo text and stored history remain distinct.

The query editor initially submits the documented read-only SQL profile to the
existing parser, binder, retrieval and execution services. Keep source parameters,
collation policy and native/query/output limits explicit. Do not translate SQL in
widgets, route it through another SQL engine or treat parser acceptance as an
execution capability. Display unsupported operations as explicit outcomes.

## Presentation and ownership

Use Rust with egui/eframe for the native host and custom Access-familiar
Microsoft/Office-style presentation: ribbon command groups, object navigation,
document tabs, datasheet, query editor and status area. Toolkit defaults are
implementation mechanics; review the actual product layout, density and states.
Selawik from Microsoft's official release 1.01 is bundled with its OFL notice in
`assets/ui/fonts`. The presentation theme installs
it with fallback glyph coverage. Glyph fallback, DPI and text sizing still need
verification on each claimed platform; bundling the font establishes no visual
parity.

The native host is `apps/tablecraft-desktop`; shared presentation lives in
`crates/presentation/tablecraft-ui`. The host owns startup, native window setup and
the UI action bridge. `crates/application/tablecraft-runtime` owns the actual
`Worker`, private `Document` read session, open/page/query commands and owned
results. Database snapshot capture, source lifetime, native browsing and query
execution belong to that runtime session, not desktop widgets or presentation.
ACE retains format interpretation, Access adapts it, and query semantics remain
in the query crates. The runtime and CLI must retain GUI-free normal dependency
closures. Reuse working contracts rather than introducing empty packages.

Preserve native window controls and OS window management. Keyboard navigation,
focus restoration, visible pending/error states, selectable cell text, scrolling
and resizing belong in the first usable browser. Unsupported ribbon commands
must not appear to succeed. Forms/report designers, printing, arbitrary native
editing and VBA execution are outside this first slice; their broader product
requirements remain open.

## Background work and publication

The shared runtime runs file inspection, page reads and query evaluation outside
the frame thread. Its worker owns the `Document` session, native resources and
operation budgets. The desktop submits owned open/page/query commands and
renders owned results. Widgets must not hold a live database connection, implement
backend admission or borrow a reader across frames. Backend errors belong to the
runtime contract; the UI presents them without reclassifying a failure as success.

Tag each request and completion with source identity and a monotonically changing
view generation. Opening another source, changing a table, projection or query,
and closing a view invalidate earlier results. A late completion cannot replace
a newer view even when cancellation arrives after the backend has finished.
Cancel superseded work through the existing callbacks, retain typed cancellation
and limit errors, and keep the UI responsive while a worker unwinds.

Publish a buffered page/query result only after its operation succeeds. Do not
cache a progressive cursor suffix or cancelled prefix as a complete table. Keep
complete membership directories separate from selected parent payloads and
filtered query results. Reopening or replacing a source must create fresh
source-bound handles; the initial browser requires a stable/offline file, not an
unqualified concurrent Access session.

## Editor restrictions and release gates

The initial browse surface remains a snapshot. An explicit offline cell-edit
preview now invokes normal typed editing; the SQL editor executes admitted reads only;
it is not an unrestricted action-query or scripting console. A later editing UI
must call the same typed command/session services as other hosts, with fresh
source/constraint checks, exact target capability, consuming revision/lease
semantics and durable recovery where required. No button may bypass unsupported
validation, data macros, locking or native-save restrictions.

The connected editing slice belongs in the runtime, using the existing
[offline write session](../access/writes/OFFLINE_WRITE_SESSIONS.md); desktop must
not launch CLI commands or reproduce native planning. The current filesystem
session is macOS/Linux-only and does not supply Access multiuser coordination.
Collect an edit before acquiring its exclusive lease. Under that lease, compare
the current complete image with the viewed source before resolving a fresh
snapshot-local row handle; a displayed ordinal alone is not write authority.
Keep query-result updateability separate from direct table editing. Apply must
retain validation, journal/recovery outcomes and fallible release, then publish a
fresh read view only after acknowledged completion. A cancelled or failed
operation cannot be represented as a successful toggle or edited cell. This is
the integration direction, not a currently implemented GUI write capability.

Before calling the first desktop slice usable, verify a real file-open → catalog
→ table page → explicit SELECT workflow, unsupported/corrupt file errors, empty
and nullable results, paging/projection, cancellation and stale-generation races.
Use existing independent fixtures and backend results. Include delayed reads,
resource exhaustion, worker failure and window close during work. Check keyboard
and focus behavior, long names, display scaling and native window operation on
the claimed platform; preserve backend and headless CLI regression checks.

A read-only developer milestone can proceed while broader backend compatibility
remains incomplete, provided its limits are visible. A general editing release
requires separately qualified writes, constraints, recovery and native Access
save/reopen behavior for its claimed operations. Shared-file concurrency,
application workflows, accessibility and platform packaging each need their own
observed evidence. None follows from a passing desktop build or independent
reader comparisons.

[UI index](README.md) · [Native reads](../access/NATIVE_READING.md) ·
[Native queries](../access/queries/NATIVE_QUERIES.md) ·
[Query execution](../query/QUERY_EXECUTION.md) ·
[Editing contracts](../foundation/DB.md) ·
[Access parity decision](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md)

## Local verification — 4 November 2026

The macOS/aarch64 desktop builds and passes strict all-target lint alongside
runtime and shared UI. Three runtime functions exercise real native table/query
services, unsupported action SQL and generation/cancellation handling. One
headless egui function renders empty and populated shared workspace states with
Selawik; it handles texture deltas explicitly. Twenty-three dependency-policy
tests pass, and resolved normal dependency trees for CLI, runtime, ACE and query
contain no egui/eframe/winit dependencies. Formatting and documentation links pass.

Native TableCraft startup and its empty-window layout/accessibility tree were
observed through computer use. An initial sandbox launch produced macOS service
errors; an outside-sandbox launch and the private development bundle allowed
window inspection. Interactive file-open/table/query verification remains pending:
the computer-use tool exposed fields but its input actions did not change them.
This does not establish platform keyboard/accessibility or a full native workflow
pass. Windows/Linux device, packaging and accessibility checks remain unrun.

The first compile corrected the installed egui style API. The first rendering
test needed explicit disposal of egui texture deltas, and a subsequent compile
followed the font notice's user rename to `OFL-Selawik.txt`. The final runtime
three-test and UI one-test passes, build and strict checks are the current result.

## GUI implementation sequence

The active goal is the Access-compatible application, rather than a single empty
window or a prerequisite to finish every engine profile first. Work proceeds in
reviewable slices, with backend changes justified by the same user workflows:

1. Connected opening, password/error handling, table navigation, bounded pages and
   explicit read SQL; verify the public runtime and real desktop path.
2. Native file picking/drop, separate object/query tabs, keyboard navigation,
   accessible labels and cells, selection/copy, column metadata and responsive
   paging/search/filter/sort controls through shared services.
3. Query design, schema/relationship inspection and designer workflows, retaining
   native names, semantics and explicit unsupported cases.
4. Connected datasheet editing and creation through qualified typed sessions,
   shared validation and journal/recovery. Expand backend profiles needed by
   actual edits; do not expose a cell as editable solely because it is readable.
5. Imported and authored forms/reports, application navigation, non-VBA actions
   and macros, printing/preview and preservation. Deferred VBA execution remains
   visible rather than silently changing imported application behavior.
6. Platform integration, document associations, accessibility/DPI/fallback review,
   packaging, upgrades and reproducible Windows/macOS/Linux workflow verification.

The initial developer preview does not complete any of the later milestones.
Existing backend roadmap items and native Access comparisons retain their own
acceptance boundaries alongside these UI-driven tasks.

### Connected workflow and keyboard follow-up

The public runtime now passes five tests through its actual Worker Open/Page/Query
interface: file-backed catalog/read/query, immutable reads after the source file
changes, failed-open session clearing and retry, exact missing/wrong password
errors and successful retry, stale invalid-path suppression and cancellation.
The encrypted test uses a temporary derivative of the existing licensed example
through the production encryption writer; independent engine goldens, rather than
this test, remain the crypto oracle. Runtime code is split into document/source,
rows, query, display and control owners.

Table pages and query results now retain separate slots, with host completion
routing by request kind. Shared shortcuts focus the path (Command/Control+O), run
an admitted query (Command/Control+Enter) and cancel current work (Escape).
Eight UI test functions pass across the original batch and one targeted rerun:
view/admission state, key events, visible-row virtualization over 10,000 rows,
Tab focus, full cell copy/accessibility value and the shared render smoke. The
shortcut fixture was corrected for egui's actual key-event API; the accessibility
assertion uses Label's AccessKit value property. No production workaround was
added for either test API assumption. Strict lint for all three packages passes.
Cells have real selectable/focusable Label semantics; complete grid topology and
screen-reader/platform qualification remain future work.

A subsequent mailbox review found and fixed a fast-result publication race and
concurrent pending-request ordering. Submission/cancel now serialize generation,
old-result clearing and pending replacement under the mailbox lock; native work
still runs outside locks. All six runtime tests pass after the correction,
including 2,048 submissions from concurrent producers with highest-generation
result checks. Strict lint passes again. This verifies task coordination rather
than database shared-file concurrency.


### Shared workspace ownership and labels

Workspace rendering now has focused chrome, navigation, query and content modules
while State composes them. Public state/actions, separate result slots and existing
tests retain their behavior. Path, Password and SQL controls have persistent
visible labels connected through actual egui label relationships. The font notice
continues to load from the root asset directory. All eight UI tests and desktop/UI
strict lint pass after this extraction; this is not additional platform accessibility
or native workflow qualification.


### Native selection and background completion

Desktop now advertises file-selection capability to the shared UI: Browse and
Command/Control+O request the host picker. A host without that capability retains
path focus. Busy state refuses picker requests. Desktop owns the parented native
async dialog and polls it with a repaint waker, adding no executor or database
read path. Selection dispatches the existing Open task with an exact PathBuf;
manual path edits invalidate the native binding. Cancelling leaves the current
document, password and results intact. The preview filter includes native MDB,
ACCDB, MDE/ACCDE, ACCDR and ACCDC inputs; content qualification remains authoritative.

Three desktop tests pass future completion/cancellation and non-Unicode path
identity; two new UI tests pass host-capability/shortcut admission (ten UI tests
total). A fourth desktop test verifies task completion through eframe's logic
callback with no visible UI pass. Runtime/dialog results are consumed there so
background processing does not depend on drawing the window. Desktop/UI strict
lint and the native binary build pass after these changes.

During native observation the old preview accepted path input and submitted Open,
but complete catalog results were not observed. Its sampled worker was idle; the
review identified UI-only completion polling and corrected it as above. Reopening
the updated preview produced a live process, but the computer-use connection then
timed out. Native picker/table/query interaction remains unverified; these tool
failures are not recorded as a successful workflow or a native database failure.

## Screenshot-grounded presentation

The [retained Access visual references](VISUAL_REFERENCES.md) govern the initial
layout correction: compact light chrome, a white command surface, document tabs,
object navigation and dense datasheets. The references and their application
version limits are recorded there. Current controls remain a read-only preview;
visual resemblance does not expand editing or native compatibility claims.

The corrected shared UI passes all ten existing presentation tests, including
virtualization, cell copy/focus and keyboard admission; all seven runtime and four
desktop tests also pass. Strict Clippy for the three packages and the native build
pass. A rebuilt native launch was attempted for visual comparison, but computer-use
inspection timed out; the sampled process was still in the macOS dynamic loader.
The corrected native rendering and interactive catalog/page/query sequence are
therefore not yet observed. The earlier native screenshot depicts the old layout.

## Read-only Table Design and native workflow follow-up

An explicit View schema command is available on the current table and in each
catalog table's context menu. It uses runtime `Describe` and
`NativeBrowser::describe`, without loading user records or required/default/
validation property maps. The shared Design document follows the captured central
field grid and lower Field Properties area. It shows original names/numbers, type
codes, storage widths/flags, available read carriers and complex/Decimal facts.
Reader NULL support is labelled separately from the uninspected Required property.
Unsupported fields remain visible, even when the table's scalar page refuses.
Table pages, query results and metadata retain independent slots; replacing the
source clears its schema and the usual generation guard rejects old completions.
This is an inspector, not a schema editor. Descriptions/captions remain uninspected.

The schema addition passes two real-worker fixture cases, two shared state/render
cases and a desktop source-replacement/adaptation case, alongside retained checks.
The diagnostic observation now runs in a dedicated integration-test process after
a batch exposed tracing-core's scoped-subscriber registration race. Its complete
operation/privacy assertions remain enabled.

The corrected light workspace was subsequently observed on macOS. A private
development bundle containing a regular executable replaced the earlier symlinked
preview bundle; no application security settings were changed. The public ACE
2007 example copied to a temporary directory opened and enumerated its catalog.
Selecting ROULETTE_TB displayed 38 records. The SQL editor executed
`SELECT TOP 3 [NBR_VALUE], [COLOR_ID] FROM [ROULETTE_TB] ORDER BY [ORDR_NBR]`
and displayed `(2, 0)`, `(0, 65280)`, `(28, 0)`. Browse opened the native picker;
Cancel preserved the document, SQL and complete query result. These observations
supersede the earlier unobserved catalog/page/query/picker states above. They
qualify this macOS/public-example workflow only.

The Desktop-path attempt remained in the OS file-open syscall, while the same
bytes in the temporary directory completed. Protected-folder consent/access and
Desktop-path opening still need qualification; the observation does not establish
a specific OS permission cause. A blocking filesystem open is not interruptible
by the cooperative cancellation callback. Table Design native observation is
now confirmed on the rebuilt preview: tblContacts displayed its seven field
declarations, initially selected ID, and selecting FirstName updated the lower
pane to native field number 1, variable storage and original flags 0x02/0x01.
The document surface is white and navigation table icons use decorative vector
geometry; the previous Unicode symbol was visibly missing from the bundled fonts.
Native design rendering and field selection are observed; schema editing remains
unimplemented.

The final connected checks pass eight runtime unit tests, one isolated diagnostic
integration test, thirteen shared UI tests and five desktop tests (27 total). Strict
Clippy for all three packages, formatting, dependency boundaries, documentation
links and the native binary build pass. No full engine-suite rerun or Windows/
Linux/native Access comparison is claimed for this presentation slice.

A pending Describe reply also preserves a newer Table/Query tab choice. Hosts
record an explicit Design activation intent before requesting metadata; tab
navigation or source reset clears that intent. The reply still updates the
metadata slot, but activates Design only while that original intent remains.
The focused regression covers tab changes, normal completion and source reset;
source identity remains enforced by the host/worker generation checks.

## Stored cells and complete collection summaries

The shared [runtime read contract](../application/RUNTIME.md) now supplies typed
cells through the existing Page command. Metadata selects ordinary scalar paging
or qualified stored-field browsing once, without retrying failures. Calculated
results retain cached provenance, including NULL results; no formula runs.
Multivalue, attachment and history summaries distinguish absent and assigned-empty
collections, and complete counts include duplicate/NULL items and NULL FileData.
Current Memo text remains a separate scalar from history membership. The native
reader eagerly validates complete bounded collections; this is not lazy loading.
Child contents, attachment decoding/export/opening and history expansion are not
yet exposed by this slice.

Cells render their value before a compact annotation. Tooltips and accessible
context retain the full provenance/membership description. Copy publishes the
complete display text without its badge; a literal text NULL keeps ordinary
scalar facts. Runtime output accounting includes typed cell/row/header envelopes
and collection iteration under its independent presentation budget.

A temporary copy of the privately supplied sample now opened and displayed all
sixteen Table1 fields in the native macOS window. Its attachment count and stored
calculated result were present alongside the scalar/date/currency fields, rather
than refusing the table through the earlier scalar-only path. No private database
or screenshot is added to the public repository. Formatting properties, checkbox
widgets, child inspection and broader Access interaction parity remain work; raw
value display is not a claim to reproduce those native presentation rules.

Native observation found that a one-row table's horizontal scrollbar overlapped
the row. The datasheet now reserves a full-height viewport with solid scrollbar
space. An actual-widget test exercises horizontal wheel movement and scrollbar
dragging for both one-row and hundred-row tables, using logical response bounds
rather than Retina screenshot coordinates. Its initial geometry failure was due
to the headless clock not advancing visibility animation; the settled strict
geometry assertion and both input paths pass with a deterministic frame clock.

The final rebuilt macOS preview visibly reserves the scrollbar below the full
datasheet. Clicking its right-hand track revealed the final attachment and cached
calculation columns with value-first annotations; the one-row data remained
uncovered. All 39 focused checks pass: fourteen runtime unit tests, one isolated
diagnostics test, eighteen shared UI tests and six desktop tests. Strict Clippy
and the native build pass. The shared-fixture refactor separately passed strict
checks for 43 consumers and 77 representative engine/adapter/CLI tests.

## Explicit collection contents

A collection cell's View contents command, or Enter while that cell has focus,
now opens a read-only dialog through the shared runtime service. It shows ordered
multivalue members, stored attachment metadata or Memo history. Assigned-empty
and absent collections remain distinct. Attachment payloads are neither decoded,
opened nor exported; history keeps original DATE bits beside its rounded civil
display. Child IDs are stored metadata, not durable editing handles.

Targets retain the current document revision, original table/column names and
global parent ordinal. Dialog replies must match the current request and target;
closing, navigating or reopening cannot resurrect stale contents. Parent data,
query results and schema remain intact. The modal owns its keyboard shortcuts;
closing a matching pending dialog cancels only that request. Complete native
membership and bounded presentation must succeed before publishing any result.

Native inspection caught an overlapping modal grid that headless semantic checks
had missed. The datasheet now receives a bounded viewport below the heading,
controls and metadata. A settled-frame test checks actual text geometry against
those controls, rather than recreating the layout in a test-only widget.

The rebuilt macOS preview now visibly separates the heading, Close button and
metadata from the child grid. Explicit attachment inspection on a temporary
sample copy displayed its stored filename/type and encoded byte count, retaining
NULL metadata. Close restored the same parent table. This is a local display/read
observation, not attachment decoding or a native Access behavioral comparison.

All 52 focused checks pass: twenty runtime unit tests, one isolated diagnostics
test, twenty-four shared UI tests and seven desktop tests. Strict Clippy and the
native binary build pass; dependency guard checks and its twenty-three tests pass.
No full engine-suite rerun, Windows/Linux run or platform screen-reader claim is
made for this slice.

## File/Open Backstage

File opens a full-page surface beneath the retained title and status areas,
replacing the object navigation and document content while it is visible. The
left rail and red selected-section accent follow the retained Access Account
capture. Open contains the actual local path, masked password and native picker;
About contains TableCraft information and the bundled font notice. The reference
does not establish the exact layout of Access's Open page.

These controls submit the same host actions and runtime commands as before.
Successful Open completion dismisses Backstage; a failed Open remains visible
with its error. Existing failed-open source clearing is unchanged. Native picker
cancellation leaves the selected source, input and result state intact; actual
selection retains its exact native path before explicit opening. There is no new
file-reading or provider path inside the UI.

Back and Escape navigate away without cancelling the current task or clearing
table/query/design state. Query execution shortcuts apply only to the visible
query workspace. Command/Control+O retains the host's native picker route where
available; hosts without one show Open and focus its path. Task cancellation
remains an explicit command. Unsupported Save, Print, account/cloud and designer
commands are not represented as working controls.

Native macOS observation passed missing-file error display, successful retry and
success-only return to the workspace, table loading, File entry, native picker
cancellation and Escape returning to the retained table. Computer-use transport
briefly disconnected; resetting that tool restored observation without changing
application behavior. The rail's initial toolkit blue selection was corrected to
the reference's pale background and red accent.

All 57 focused checks pass: twenty runtime unit tests, one diagnostics integration
test, twenty-seven shared UI tests and nine desktop tests. The three Backstage
action/layout checks also pass after the rail-color adjustment. Strict Clippy,
native build, formatting, dependency boundaries and documentation links pass.
Open layout checks include the desktop minimum 900 × 580 and default 1280 × 820
logical sizes, with long path/error text. Actual Windows/Linux and platform
screen-reader qualification remain unrun.

## Active cells and working Home commands

The loaded datasheet retains a selected cell separately for table pages, query
results and collection contents. The screenshot's yellow active-column header,
blue current row and red active-cell outline make that state visible. Mouse and
plain arrow keys move within the loaded data; keyboard movement reveals its
target without continuously forcing scroll position. No implicit page fetch,
editing or new-record row is supplied by selection.

Publishing new Data resets that slot's selection, including same-name/same-offset
replacements. Ordinary document-tab and File navigation retain each slot's
position. A default first selection does not request keyboard focus from a text
editor. Keyboard Copy belongs to the focused selected cell; the Home Copy command
can use the retained selection after toolbar focus changes. Both publish its full
display value through the toolkit clipboard, without badges. Hidden results and
Design cannot supply a cell copy; a collection dialog uses only its own contents.

Home contains working Views and Clipboard groups. Datasheet and Table Design
apply to the active table/schema owner; the Query tab cannot redirect them to a
previous table. Table Design submits the existing metadata command, while Copy
uses presentation state only. Unsupported editing, formatting, paste and sorting
commands are not filled in as decorative functionality.

Native macOS observation confirmed the yellow header, blue row, red active-cell
outline and readable left text inset. Fifteen separate Right presses retained
cell focus through the last field and revealed it horizontally; Left and Enter
then opened the adjacent attachment contents. Home Copy published the complete
selected text through the system clipboard, verified by pasting into a local
field without opening a different source. Home Table Design and Datasheet used
the real metadata route and retained page respectively.

The checks caught a nested-scroller reveal bug, centered text caused by a
full-cell label layout, and egui's automatic directional focus escaping after the
first arrow. The implementation now issues horizontal reveal outside the inner
vertical scroller, paints text within one full-cell response and owns consumed
arrow focus. Tests use actual response bounds and separate keydown/keyup frames;
manual-scroll checks wait for the toolkit animation to settle. Initial guessed
click coordinates and an ambiguous text-prefix lookup were corrected in the test
helper, without changing the intended selected-cell assertions.

All 62 focused checks pass: twenty runtime unit tests, one diagnostic integration
test, thirty-two shared UI tests and nine desktop tests. The final UI/desktop
batch was rerun after the visual/focus corrections; strict Clippy, native build,
formatting, dependency boundaries and documentation links pass. The private
sample remains byte-for-byte unchanged. These checks do not establish editable
cells, full grid accessibility, Windows/Linux behavior or native Access parity.

## Native Yes/No display controls

The renderer now receives exact Boolean facts separately from stored field
display-control metadata. A checked or unchecked read-only checkbox requires a
non-NULL Boolean and an explicit native checkbox declaration (106). A literal
text `true`, numeric -1/0 or NULL cannot become a checkbox from its spelling or
value. Cache provenance remains independent, and Copy retains the original full
cell text. Clicking, Space and Enter select/focus the cell without changing data.
Accessibility exposes the checked state and read-only property without a toggle
action; platform screen-reader qualification remains separate.

Text-box declarations (109) retain text, and combo boxes (111) or other
unsupported controls do not silently become checkboxes. Absent and uninspected
settings remain distinct, with explanatory context. Query results and child
contents do not inherit a native field control through an alias or parent name.
Field Format/lookup rendering, inferred defaults and general display-control
support remain work.

Microsoft documents that DisplayControl affects datasheet presentation and allows
Yes/No fields to use checkbox, text-box or combo-box controls. See the
[DisplayControl reference](https://learn.microsoft.com/en-us/previous-versions/office/aa195849(v=office.11))
and [control-code enumeration](https://learn.microsoft.com/en-us/office/vba/api/access.accontroltype).
The [default-new-field guidance](https://support.microsoft.com/en-gb/access/add-a-check-box-control-to-show-yes-no-values)
does not establish the declaration of an existing field whose metadata was not
inspected. The retained Access screenshot supplies the actual visual reference.

The rebuilt macOS preview displayed both actual sample Yes/No declarations as
checked boxes. Repeated arrow navigation reached the field; mouse click, Space
and Enter retained its original value. Home Copy published the original `true`
text through the real system clipboard, verified by pasting into a local field
without submitting another Open. The private source/copy comparison remains
unchanged. This observes local presentation and interaction, not a Windows Access
differential or platform screen-reader qualification.

All 72 focused checks pass: twenty-five runtime unit tests, one diagnostic
integration test, thirty-four shared UI tests, nine desktop tests and three
native property-accessor tests. Strict runtime/UI/desktop and minimal-engine
Clippy, native build, formatting, dependency boundaries and documentation links
pass. No full engine-suite rerun is claimed. Synthetic native fixtures cover
explicit checkbox/text-box codes, unknown codes/flags/types/widths, absent
properties, a missing carrier, corrupt present framing and unchanged bytes.

## Connected offline editing preview

A native scalar cell's context menu offers **Edit cell…** as a candidate request.
Runtime determines actual eligibility. Enter a replacement or request NULL, then
choose **Review change** to obtain a native validated before/after result.
**Apply to original file** consumes that exact review. Editing the input or
changing the source invalidates it. Query results, collection summaries and
calculated cells cannot supply direct native update targets.

Close other writers first. Draft/review hold no file lease; Apply uses the normal
exclusive offline session, exact whole-image precondition and checked journal.
Desktop only dispatches and presents runtime results. A running or retained save
blocks navigation and window closure until its outcome/resources are resolved;
cancellation never claims rollback. Successful acknowledgment refreshes the
source and original table. Retained setup/release failures can require diagnosis.
The [runtime contract](../application/RUNTIME.md#reviewed-offline-cell-edits)
records the platform, restart-discovery and native profile limitations.

This is an explicit offline workflow. It does not implement Access
record autosave, live shared editing or general query-result updateability.
Native macOS observation now covers the draft, review, apply, acknowledgment and
refreshed-table sequence on a disposable independent Jet 4 fixture. Score row 2
changed from 20 to 27. Comparing the entire 57,344-byte file with the original
found only the expected byte at offset 53,232 changed; no journal entries remained.
A second draft containing invalid integer text retained that text and showed a
validation error, with no Apply action. Cancel preserved the exact saved image.
The window then closed normally. This does not qualify Access-native behavior,
other write profiles or Windows/Linux GUI operation.

The native automation initially failed to focus the text area through its
accessibility index; the unchanged review correctly reported zero changes. A
coordinate click based on the actual screenshot focused the input, after which
the changed-value sequence above passed. No no-op review was reported as a
changed-value save.

All 88 focused checks pass: thirty-three runtime unit tests, one diagnostic test,
thirty-nine shared UI tests, thirteen desktop tests and two Access image-guard
checks. The edit tests use independent synthetic Jet 4 inputs and real normal
session/journal services; they do not establish a new ACE 16 native differential.
Strict runtime/UI/desktop Clippy, native desktop build, formatting, dependency
boundaries and twenty-four guard tests pass. No full engine-suite rerun or
Windows/Linux desktop observation is claimed for this slice.

## Record navigation across pages

The table datasheet now has a bottom record navigator with First, Previous,
Next and an explicit one-based record-number input. Moves within the loaded page
select locally; other positions use the existing bounded runtime Page service.
The selected field is retained and the target cell receives focus/reveal once.
The separate Previous/Next page buttons remain explicitly labelled as page moves.

A pending navigation retains the old page. Only the exact document revision,
table, request, starting offset and requested field can publish its result.
Checked EOF, invalid positions and failed/cancelled requests preserve the prior
page and selection. Switching views or sources, or entering a collection/edit
dialog, invalidates pending navigation. A save cannot be superseded by a record
request. Declared row counts supply neither an observed total nor a Last-record
operation; those broader navigation controls remain work.

Incomplete save setup now offers explicit resource release without deleting or
repairing retained files. Acknowledgment can then close that save outcome; the
reserved active-journal guard still prevents reopening an unresolved source.
Failed/released outcomes independently retain the current operation result and
last completed receipt, so an earlier known commit is neither hidden nor treated
as the current operation's result. Uncertain unlock still requires diagnosis.

Native macOS observation passed record 101 via the focused Go input and plain
Return, Previous across the boundary back to 100, checked EOF at requested record
999 with the prior page intact, and First returning to record 1. Score retained
selection and actual keyboard focus. The navigator remained below the grid; the
entire synthetic 105-record database stayed unchanged.

A native setup-collision check created a sentinel bundle only after review.
Apply reported retained setup failure without changing the database. Explicit
Release reported acknowledged resource release; Close cleared the stale view.
A subsequent Open refused the retained reserved entry. Both source and sentinel
bytes remained exact. This checks actual GUI dispatch and the normal resource
owner path, not filesystem crash recovery or native Access interoperability.

The focused batch passes forty-five shared UI tests, eighteen desktop tests,
thirty-three runtime unit tests plus diagnostics, four low-level setup-unlock
checks and twenty-nine Access unit tests. The additional focused Return test
caught the toolkit's focus surrender on submission; navigation now accepts that
field's own lost-focus Return as well as focused Return, with exact modifier
checks. Strict selected-package Clippy, native build, formatting, dependency
boundaries and documentation links pass. No full engine-suite rerun is claimed.
