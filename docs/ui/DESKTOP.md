# Native desktop preview

Datasheet column widths and horizontal position follow the current object,
source revision and ordered column names across native page changes. Record
selection and widget identities remain page-local. A different object, source
or column schema gets its own geometry; collection layouts retain their separate
width policy. This is session presentation state, not saved database formatting.

The unused datasheet viewport continues subtle grid rules below real records and
the available insertion row, following measured widths, zoom and scroll position.
These lines are background decoration: they add no records, selection targets or
clipboard cells. Empty-state messages and totals remain above the background;
collection layouts retain their separate presentation.

Native table pages can carry checked sparse row membership independently of
their displayed offset. Editing, deletion and collection targets resolve the
original snapshot ordinal through that membership; query results cannot acquire
native identity from their row position. Malformed maps refuse publication or
target construction without a contiguous fallback. Visible numbering remains a
separate display position. Selection filtering uses dedicated scoped paging and
Find routes; generic unfiltered navigation never bypasses a mapped view.

The scratch query object also has a bounded [visual SELECT designer](QUERY_DESIGN.md).
Its metadata requests and SQL build/import are inert; Run explicitly invokes the
existing query service. Unsupported imports retain the original SQL and previous
results. Saved-query editing and broader designer parity remain unfinished.

For the current presentation-only mock mode, light/dark themes and bundled
Fluent languages, see [presentation preview](PRESENTATION_PREVIEW.md). The native
interaction records below retain the scope and state observed at their capture.

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

## Object workspace contract

The workspace now represents each opened table, saved query and the single
scratch SQL editor as a real object tab. An object target binds its kind and
literal catalog identity to the current document revision; a tab adds a fresh
workspace identity. Equal names in a reopened file do not authorize an old tab,
row target or runtime handle.

A single Navigation Pane click selects an object without replacing the current
document. Double-click, Enter and the explicit Open command open or activate its
tab. Selecting an existing document tab activates its retained session; closing
an inactive tab leaves the current object and its native handle alone. These
selection/open distinctions follow Microsoft's [Navigation Pane
contract](https://support.microsoft.com/en-us/access/guide-to-the-access-user-interface).
Table Datasheet and read-only Design are two modes of the same table object,
not separate table tabs. The View control and tab context menu switch those
modes. Tab activation and individual closing follow the documented [object-tab
workflow](https://support.microsoft.com/en-us/access/show-or-hide-object-tabs);
this implementation does not imply tab reordering, Close All or overlapping
windows.

Each table session owns its page, selection, metadata, selected Design field,
view mode and retained filter/sort definition. Native handles remain private to
the desktop/runtime bridge and accompany the exact owning tab. Saved-query
sessions retain their own inspected definition, committed typed parameters,
results and selection. The one scratch SQL draft remains available while other
objects are active; its results and selection belong to its own tab. Closing
the scratch object discards that draft. Inactive sessions park presentation
values rather than duplicating complete row buffers, and activation reapplies
the current display policy.

Saved-query Open deliberately remains inert inspection followed by explicit
Run. Access normally executes a SELECT or crosstab when the user opens it by
double-click or Enter, as described in Microsoft's [Run a query
article](https://support.microsoft.com/en-us/access/run-a-query). The current
TableCraft boundary is an explicit compatibility difference: opening a saved
object cannot execute it, prompt for execution parameters or run active content.
Run uses the existing sequential parameter prompts and qualified read-query
service; opening another saved tab does not reuse another object's parameters
or results.

Managed publications must match the active object's exact kind and catalog
binding. Table pages cannot replace a query session, and scratch-query results
cannot replace a table or saved query. Standalone shared-UI consumers with no
managed object retain their existing isolated publication contracts; this does
not grant native row identity to query results. Host request generations and
runtime source validation remain necessary in addition to object identity.

The shared workspace admits at most 16 open objects and accounts for at most
64 MiB of retained presentation storage. Admission uses checked size accounting
and refuses an unretainable publication or object transition. This is a UI
retention ceiling, not a process-memory guarantee or an increase to native
snapshot, query, comparison, display or operation-specific resource pools. Those
services retain their independent limits and checked completion requirements.

An acknowledged offline save reopens a fresh read source. The host can retain a
source-free workspace layout: object identities by catalog kind/name, table
mode/page preferences, scratch SQL and saved-input preferences. It does not
restore old rows, query results, native handles, reviews or tab authority. New
tabs bind the fresh document revision; inactive objects load lazily when
activated, and retained filter/sort preferences are revalidated by the runtime
against the new source. Saved definitions are inspected afresh and are never
rerun automatically. This in-memory layout handoff is not persistent workspace
storage or a resumed native-project feature.

The consuming save worker remains a single workspace owner. Review dialogs gate
object transitions; once a save starts, its worker and terminal outcome remain
owned until acknowledgment. Object switching, closing and source replacement
cannot abandon that owner or transfer its rights to another tab. A fresh view
is published only through the existing explicit acknowledgment/reopen path.

The focused evidence surfaces are the shared UI object/session tests and the
real desktop tests in `workspace/objects/tests.rs`, alongside saved-query,
picker/source and editing tests. They exercise independent object state,
selection versus explicit open, same-table modes, stale actions, inactive close,
and save-owner/reopen behavior using existing independent fixtures. Local execution
passed all 186 UI cases across the 185-case functional gate and separate preview
matrix, plus all 45 desktop cases. The 57-scene × eight-locale × two-theme matrix
covers 912 combinations. Strict seven-package Clippy, formatting, dependency
boundaries and documentation-link checks passed. The resource-refusal regression
also verifies that a rejected filter publication ends its pending request while
retaining the preceding page and capability. These results are local service/host
evidence, not native Access comparisons.

Four German/de-DE light/dark objects/datasheet PNGs were opened and inspected at
1000 × 680 logical pixels and 1.25 scale (1250 × 850 physical pixels), using the
macOS offscreen GPU renderer. Images remain ignored under
`.cache/ui-objects-review/`. The actual retained Home capture was reopened;
there is no exact retained mixed-object or dark-mode Access capture. No native
application window was opened. Platform clipboard/window-manager and Windows
Access behavior remain unqualified by this slice.

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
it is not an unrestricted action-query or scripting console. The connected
cell-edit UI calls the same typed command/session services as other hosts, with fresh
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
the current restricted offline cell-edit contract. General GUI writes and
query-result updateability remain outside that implemented slice.

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

## Current acceptance boundaries

The normal-host read-only journey now has a first native macOS observation for
one supplied single-row ACCDB: File/Open, automatic first table, explicit SELECT,
schema inspection, datasheet return and object close/reopen. This is a narrow
passing journey, not the complete browser acceptance gate below. The later
[multi-row browser record](PRESENTATION_PREVIEW.md#multi-row-browser-checks-and-headless-verification--6-november-2026)
adds observed ACE 16 paging, filtering and sorting, plus a headless Jet 4 journey
covering later pages, complete-view totals, SQL and schema inspection. It also
records the bounded runtime EOF checks.
Broader fixture coverage, native error/cancellation traversal and the observed
repaint/observation uncertainty remain separate qualification work. The earlier
[native journey](PRESENTATION_PREVIEW.md#first-real-file-browser-journey--6-november-2026)
remains a dated observation.

The current request prioritizes polished mocks of existing functionality. The
broader Access-compatible GUI goal is not complete merely because this slice
builds or its mock scenarios pass.

| Requirement | Current evidence | Remaining qualification |
| --- | --- | --- |
| Shared presentation and thin desktop | UI/runtime dependency guards; desktop command and DTO bridge; resource-free mock host | Eventual web host is not implemented or implied. |
| Selawik and root font assets | Root assets, recorded source hashes/OFL notices, locale-prioritized CJK fallbacks and glyph checks | Native font/DPI behavior on each claimed platform. |
| Fluent translations | Eight locales, twenty section catalogs each, key/variable parity and literal-data separation | Native-speaker review of engineering translations. |
| Light and dark themes | Shared semantic palette, actual selected-text/badge paint checks, inspected offscreen frames | No retained Access dark-mode capture establishes visual parity. |
| Ribbon artwork | Original generated, hash-inventoried SVGs with theme/disabled variants; disabled preview commands cannot dispatch | Exact Windows interaction and pixel fidelity remain unqualified. |
| Existing mock workflows | Source-bound scenes, admission/state checks and the dated [presentation verification](PRESENTATION_PREVIEW.md) | Synthetic outcomes do not qualify backend operations. |
| Access reference fidelity | Retained Home/datasheet, Table Design and Account Backstage captures guide their corresponding surfaces | Published parameter and attachment images extend the reference record; other dialog/dark layouts remain adaptations, and native interaction parity is still unqualified. |
| Native behavior | Earlier macOS observations are recorded below with their scope and dates; current verification is headless-only at the user's request | Current full keyboard/assistive-tool traversal, Windows/Linux window behavior and monitor-scale transitions remain unqualified. |

These limits do not authorize new database functionality or resume deferred
SQLite, project or VBA work. Current checks use headless tests and offscreen
renders without opening native windows. Earlier native observations remain
historical; headless checks cannot establish native compatibility by substitution.

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
Cells retain selectable/focusable value widgets. The shared datasheet now places
those widgets under explicit AccessKit Grid, Row, ColumnHeader and GridCell
containers, with the existing record selector exposed as RowHeader at column zero.
The selector header is intentionally blank. Field columns begin at one; the header
has row index zero and records use their displayed position across pages, independently
of sparse physical row identity. Only rendered records enter the accessibility tree.
The total row count includes the header and is supplied only on a known final page;
it remains unspecified while more pages exist or an empty nonzero page cannot
establish the true end. Click to Add, insertion controls,
totals and painted empty-grid rules do not invent stored fields or records.
Headless topology checks cover sparse sorted membership, bounded rendered rows,
actual node bounds and unknown/empty-page count boundaries. All 274 shared UI and
65 desktop tests, plus strict all-target UI/desktop Clippy, passed for this change.
These are renderer semantics; native screen-reader/platform qualification remains
future work.

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

## Displayed record selection and clipboard

Datasheet row selectors now select one complete displayed record and expose an
accessible RowHeader selection state. Shift+Space toggles between that record
and its first field while the grid owns navigation focus; cell clicks and arrow
navigation return to cell selection. This follows Microsoft's
[record-selection shortcut](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
and [record-selector workflow](https://support.microsoft.com/en-us/access/ways-to-add-edit-and-delete-records).
Text editors and modal controls keep their own keyboard input.

Home, the row context menu and keyboard Copy use one selection owner. A selected
record copies all displayed fields, without headers, through the existing csv
crate's tab-delimited writer. Necessary quoting and its final LF count toward
the 1 MiB limit; failure publishes no partial clipboard prefix. NULL and stored
collection cells copy their displayed text/summaries, without child payloads.
Single-cell Copy retains its full-value text contract. This is an explicit
displayed-text interchange profile, not native Access clipboard or paste
qualification.

Record selection cannot issue single-cell edit or collection-opening commands.
New Data publications reset it; table, SQL, saved-query and child-dialog
selections remain separate. Query row selection grants no native row capability.
The focused shared UI suite passes 57 tests, including actual selector/toolbar
interaction, keyboard event ordering, publication reset, quoting expansion and
failure atomicity. Native observations are recorded separately below when run.

## Caption headers and native policy follow-up

Native page headers display qualified Text/Memo Caption metadata while retaining
the original field name in hover/accessibility context and every edit, collection
and record-navigation target. Duplicate captions cannot redirect those targets.
An explicitly empty caption remains metadata, with the current header falling
back to its original name. Query aliases and synthetic child headings inherit no
native caption. Headerless record TSV is unchanged.

The connected checks pass 60 UI tests, 49 runtime unit tests plus diagnostics with
editing enabled, 41 standalone read-only runtime tests plus diagnostics, and 20
desktop tests. Strict engine-library and selected-package checks, native build,
formatting, dependency boundaries and documentation links pass. These include
independent Text/Memo property cases, exact column allocation accounting, actual
header rendering, duplicate-label targets and the thin desktop DTO mapping.

The first native visual attempt at this stage remained unverified. The preview process was live
in its AppKit event loop, but the UI tool reported no capturable window; a later
attempt to inspect the Dock stalled and was interrupted. This is neither proof
of a rendering regression nor a successful native check. The earlier native
observations above remain historical evidence for their corresponding builds.
Subsequent fresh-preview observations are recorded below. No Windows or Access
differential comparison is claimed.

## Startup visibility and stored Field Properties

The desktop requests visibility once from its first logic callback, including
an initially occluded logic-only frame. It emits no later reveal request when
the user hides or minimizes the window. An actual framework-output test covers
that lifecycle; it is not a claim that the earlier UI tool failure was caused
by occlusion or has been reproduced and fixed.

Table Design now receives seven inert field-property observations from the same
object-bound native property service as table pages. General displays Required,
Allow Zero Length, Default Value, Validation Rule, Validation Text, Description
and Caption; absence, uninspected metadata and unsupported declarations remain
distinct. Existing storage and reader diagnostics live in collapsed Native
details. Defaults and rules are selectable text, never evaluated. The desktop
only maps the owned runtime observations into the shared UI DTO.

A fresh macOS preview displayed the Open screen, opened an independent Jet 4
fixture and showed its `Approved?` caption with original `Score` accessibility
context and `No`/`Yes` text-box values. The blank selector strip and complete-row
Home Copy were observed; pasting into the SQL editor yielded exactly
`1\tNo\n`, without headers. Selecting Score in Table Design displayed Required
Yes, default `0`, rule `In (0,-1)`, description and caption; selecting Id showed
absent properties without inventing defaults. Canonical property labels were
made human-readable after this visual review and checked in the actual renderer
test. The configured content size was 1280 by 820 logical points, with a 2x native
capture. These are functional observations, not a pixel-exact Access comparison.

The public ACE 2007 example copy also opened and listed tables and saved queries
inertly. The window later became unavailable to the UI tool before the new
query-policy run could be completed; that run is not counted as verified native
UI evidence. Both database copies remained byte-for-byte unchanged. Runtime
tests cover the stored comparison/wildcard execution path separately.

## Saved-query objects and typed parameters

The object pane now lists Tables and Queries from a single inert native catalog
capture. Search and the system-object switch apply to both groups. Selecting a
query inspects its supported read definition and declared prompts; it does not
execute it. **Run saved query** collects one declared parameter at a time in a
staged modal prompt, then executes the selected definition only after final
confirmation. A parameterless query runs directly. Cancelling acquisition leaves
committed inputs, results and selection intact. VALUE prompts require a concrete type choice, and NULL
is independent from retained text. Prompt/type/NULL controls expose their actual
label relationships and purposes through AccessKit.

The selected saved query and the ad-hoc SQL editor retain separate inputs,
results and cell selections. Their Run shortcut follows the visible mode.
Final confirmation of changed inputs invalidates the old result; staged edits
alone do not. An unchanged-input failed rerun retains and labels the previous
successful result. Late replies must match
source, object, request and submitted inputs. Native/partial/wrong-offset result
envelopes are refused. Query results remain outside native cell-edit, collection
and record-navigation capabilities.

This bounded workflow follows Microsoft's published sequential prompt reference.
Explicit VALUE type and NULL controls retain the typed profile's additional
choices. Native prompt-time validation, ordering and window behavior remain
unqualified, and general query updateability is outside this read workflow.
Microsoft's [parameter prompt guidance](https://support.microsoft.com/en-us/access/use-parameters-to-ask-for-input-when-running-a-query)
and [PARAMETERS contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql)
supply the name/type behavior; the retained Access captures continue to govern
workspace density, grouping and selection. Action/pass-through/unsupported query
declarations remain visible and report errors, without provider or VBA execution.
See the [runtime boundaries](../application/RUNTIME.md#native-saved-queries-and-declared-parameters).

The following macOS observations predate the staged modal presentation. The
native preview was exercised against an independently built raw ACE-shaped
database with two user saved queries. Opening listed the objects
without execution. A declared Long prompt returned the expected two filtered
rows; invalid numeric text was refused. A VALUE prompt required an explicit type,
returned SQL-looking text literally, and retained a separate explicit NULL choice.
Switching between saved and SQL modes preserved their respective input/result
slots. The final rebuilt binary also passed the Long query, and its parameter
field boundary was visibly present. The complete input database remained
byte-for-byte unchanged. The preview requested 1280 by 820 logical content points;
the observed 2560 by 1700 screenshot includes native title chrome at 2x scale.
This qualifies neither Windows behavior nor pixel fidelity
of Access's uncaptured parameter dialogs.

Focused checks cover the shared parameter resolver, independent fixture extraction,
native inventory, runtime parameter admission and stale replies, and the desktop
bridge. The shared UI includes actual rendered prompt/type/NULL accessibility
relationships and same-name replacement focus behavior. Strict selected-package
Clippy, native build, formatting, dependency boundaries and documentation links
pass. The standalone read-only runtime profile also passes; an obsolete binary
display-helper feature gate was removed after that check exposed it. The original
aggregate lint invocation's Cargo feature-closure failure and subsequent valid
explicit-testkit run are distinguished in the
[development guide](../development/DEVELOPMENT.md). No full engine-suite rerun or
native Access comparison is claimed.


## Native mock observation — 5 November 2026

The current debug desktop build was launched on macOS in its resource-free mock
mode through an ignored local application bundle. The requested content size was
1280 by 820 logical points; the observed capture measured 2560 by 1700 including
native title chrome, consistent with 2x capture scale. This is not independent
measurement of the operating system display configuration.

The Home datasheet and open View menu were visibly inspected. Choosing Table
Design displayed the read-only field grid and General property sheet. A native
coordinate click on City changed its description, caption and field size to the
corresponding synthetic metadata. File opened Backstage, Options displayed its
appearance controls, and a coordinate click on Dark visibly changed the theme.
The expanded language menu displayed all eight supported locale choices with CJK
glyphs. These observations used the current binary and did not open or save a
real database.

Automation was intermittent: some accessibility actions and keyboard inputs
produced no observable change, and selecting Japanese was not verified. The
initial loss of a window handle was followed by reattachment to the live process;
it does not establish an application crash. This session therefore does not
qualify native keyboard traversal, screen-reader actions, the complete locale
round-trip or Windows behavior. Offscreen interaction tests remain separate
coverage, and a complete native workflow review is still outstanding.


A follow-up on the same live mock session selected Japanese successfully and
returned from Options to the dark workspace. The localized Ribbon, navigation,
Design headers and property labels were observed while City retained its literal
synthetic metadata. A subsequent Down key moved from City to Balance and changed
the property sheet accordingly. F6 and Tab produced no observable focus change in
that attempt, so full native traversal remains unqualified. These observations
extend the earlier partial record without establishing general accessibility
operation or native translation quality.

At the user's request, further non-headless checks are suspended to avoid
interrupting their work. Continue with headless interaction and offscreen render
checks; do not activate, manipulate or close native application windows as part of
ongoing verification without renewed authorization.


## Regional datasheet and chrome headless gate — 5 November 2026

The current display/read batch passed 141 shared UI tests (including 704
scene/locale/theme draws), 58 runtime tests, 26 desktop tests and the runtime
documentation example. Coverage includes real typed date/Currency facts,
qualified native Format provenance, complete 105-row sorting with no edit
identity, retained sort presentation, automatic first-table opening and captured
path identity. System-theme transitions, viewport requests, status view/zoom,
centered headers/checkboxes, visible typed NULL and table Close are headless checks.

The retained Access Home capture was reopened before these changes. PNG exports
were reviewed from a macOS build with Windows input conventions: light datasheet
and query at 1280×820 logical / 1×, and German dark Options at 900×580 / 1.25×.
The new renderer uses the actual full viewport instead of the harness's eight-point
outer frame. These exports do not verify Windows/Linux window-manager behavior
or constitute a native Access comparison. Native UI checks remain suspended.
The [display contract](DISPLAY_PREFERENCES.md) records deliberate current differences.


## Reviewed row insertion headless gate — 5 November 2026

New row now uses a separate revision/table target and explicit field intent,
without fabricating an existing native row handle. The runtime issues consuming
reviews through the ordinary native INSERT planner. The desktop sends both cell
updates and insertion through one save owner, preserving cancellation, retained
resources, receipts, close veto, acknowledgement and source refresh.

The final selected gate passed 143 UI tests (including 736 scene/locale/theme
draw combinations), 62 runtime tests, 29 desktop tests and the runtime documentation
example. Two adapter original-image tests cover the shared UPDATE/INSERT check;
the optional-feature-free runtime build, selected strict Clippy, formatting,
dependency guard and documentation links also passed. New host tests interact
with real headless field widgets and apply/reopen an independently authored,
fully qualified native fixture. No native application was opened.

Offscreen image review used a macOS build with Windows input conventions:
English light insertion review at 1280×820 logical / 1× and Korean dark new-row
draft at 900×580 / 1.5×. Fields occupy available width and the measured footer
aligns right while retaining keyboard order. No native Access capture exists for
this explicit offline new-row dialog; it remains a provisional workflow.
See [runtime bounds](../application/RUNTIME.md#reviewed-offline-row-insertion).


## Reviewed deletion headless gate — 5 November 2026

The selected gate passed 145 UI tests, 67 runtime tests, 31 desktop tests and the
runtime documentation example. All 48 synthetic scenes render in eight locales
and both explicit themes (768 combinations). Actual widget tests cancel, apply,
retain and acknowledge a normal native deletion, reusing the application-private
host harness shared with insertion tests. Runtime checks cover exact membership,
source drift, stale capabilities and complete-snapshot refresh positions.

Compact German dark Delete review was exported from a macOS build with Windows
input conventions at 900×580 logical / 1.25× and inspected offscreen. The original
Home capture was reopened for its working Delete command. No retained Access
Delete-confirmation capture exists; this explicit offline dialog is provisional.
Cancel receives initial focus; Enter and Escape cancellation are observed in
headless tests. No native app activation or Windows Access comparison occurred.


## Typed instructions and Home navigation gate — 5 November 2026

The combined selected gate passed 149 UI, 67 runtime and 32 desktop tests plus
the runtime documentation example. Typed input hints are mapped exhaustively
across the host and rendered fully visibly in German/Japanese edit and new-row
forms. Decimal instructions explicitly retain invariant `.` syntax. Home Go To
shares the footer input/parser and native target service; Select record uses each
visible result's existing selection and complete single-record clipboard output.
Actual popup and source/modal/EOF/query boundaries are exercised headlessly.
Strict selected Clippy passed; no native application was opened.


## Long-value presentation gate — 5 November 2026

All 153 shared UI tests passed, including the 768 scene/locale/theme combinations.
Three helper cases cover exact boundaries, decomposed Unicode, emoji, combining
sequences and CR/LF/CRLF/Unicode line separators. An actual-cell observation
checks bounded painted/accessibility payloads for large ASCII, CJK and multiline
values while the emitted Copy command retains the complete text. Strict UI
Clippy, formatting and documentation links passed. No native application was
opened and no native clipboard publication is claimed by these headless checks.

## Native Find and complete-value Zoom — 5 November 2026

Home Find and Ctrl+F open a modeless native-table search. The shared runtime
searches the complete admitted table under cumulative limits, checks EOF and
returns a source-bound occurrence. Desktop retrieves the matching page with a
separate native allowance and publishes only an exact revision/table/record/field
match. Cancel, failed retrieval and Not found preserve the previous page.
Field/table scope, directions, wrapping, wildcard mode and explicit case/display
policies follow the [current runtime contract](../application/RUNTIME.md#native-table-find).
At this gate, query-result Find, Replace and substring highlighting were not
implemented. Later work supplies [reviewed replacement and bounded occurrence
paint](FIND_REPLACE.md); query-result Find remains unavailable.
Unformatted regional date and fractional-number searching still refuse explicitly;
formatted qualified dates and Currency share the datasheet formatter.

Home Zoom, Shift+F2 on a cell and the cell context menu open a resizable read-only
viewer. It retains the complete scalar text up to an explicit 8 MiB viewer limit,
lays out only visible bounded rows and copies the full value on request. Native
Edit delegates to the existing reviewed cell workflow; displayed text is never
parsed into an edit, and query results remain read-only. See
[display preferences](DISPLAY_PREFERENCES.md) for current limits.

The selected package gate passed 159 UI, 73 runtime, 34 desktop and five formatter
tests, plus the runtime documentation example. Focused review checks additionally
cover cumulative Find result-copy accounting and dropdown Escape ownership.
The synthetic matrix contains 51 scenes, eight languages and two explicit themes
(816 draw combinations). These are headless checks, not Microsoft Access native
interaction qualification. Native application activation remains suspended.

After review, all seven Zoom and five Find UI tests passed, including a new
dropdown Escape regression, the strict full-galley last-line check for an 8 MiB
value and Unicode request-byte refusal. Both desktop Find acceptance tests and
six runtime Find cases passed again after the relevant corrections. Final strict
Clippy passed for display/UI/runtime/desktop and the Access/query/testkit helper
closure; the runtime build without default features also passed. Dependency
boundaries, their 25 Python tests and documentation links passed.

Six final German/de-DE offscreen exports were rendered and inspected for
Datasheet, Find and Zoom in both themes: macOS build, 1000×680 logical points,
1.25× scale (1250×850 pixels). They are retained privately under
`.cache/find-zoom-review-final/`. Find and Zoom start centered while retaining
movable window positions. These exports do not exercise native window controls,
OS clipboard integration or a Windows application.

## Native selection filtering — 5 November 2026

Home → Selection applies Equals/Does not equal to the actual native cell value;
NULL cells offer Is Null/Is Not Null. Displayed rounding, symbols and localized
dates do not become predicate inputs. A new criterion replaces the existing
criterion for that field; different fields combine with eager AND. Toggle Filter
retains the definition while switching between matching and original records.
Advanced → Clear All Filters removes it after successful unfiltered retrieval.

The runtime returns complete checked membership and an opaque source-bound
handle. Visible pages retain original row ordinals, captions, controls, collection
summaries and calculation provenance. Find searches only the active membership;
paging and Go To use visible positions, while edits/deletion/collections resolve
the original native record. A failed candidate retains the previous view and
handle. After an acknowledged offline save, typed filter preferences bind to the
reopened schema and rescan under a new source identity; old row rights are not
reused. A changed record can consequently disappear from the matching view.

This is bounded, session-local selection filtering for native tables. It does
not persist Access object Filter/FilterOnLoad properties, filter query results,
sort a filtered view, expose partial-value/range menus or implement Filter by Form.
The distinct-field ceiling is sixteen. The existing platform/write limitations
remain in force. See the [runtime contract](../application/RUNTIME.md#native-table-filtering).

The coordinated gate passed 170 UI, 83 runtime, 37 desktop and five formatter
tests, plus the runtime documentation example. Three actual desktop widget tests
cover selection/paging/toggle/clear, filtered Find and a reviewed save that changes
membership after reopen. Native cases include eager errors, complex-field
refusal, sparse collection parents and cached NULL provenance without formula
execution. Follow-up geometry, modal-close and focused-cell copy checks passed;
the forced keyboard-focus tooltip was removed while bounded hover details remain.

Strict selected Clippy and the minimal runtime profile passed. The synthetic
matrix contains 55 scenes, eight locales and two themes (880 draw combinations).
Eight German/de-DE filtered, empty, disabled and error exports were inspected
offscreen on macOS at 1000×680 logical / 1.25× (1250×850 pixels), retained privately
under `.cache/filtering-review-final/`. Empty headers stay aligned with populated
headers, and preview exports settle layout without a synthetic pointer. No native
application window, system clipboard or Windows interaction was exercised.

## Native editable sorting — 5 November 2026

The header menu and Home Ascending/Descending use the runtime's explicit native
SortView service. A complete stable order carries a checked mapping to original
records; this is neither a sort of the visible page nor an SQL result substituted
for the table. Current native edits, deletion and collection targets retain those
physical identities. Equal keys retain source order, NULL sorts first ascending,
and typed values use the strict key service and qualified native text policy.

Clear All Filters keeps the sort. Remove Sort keeps filter criteria. An off-state
filter keeps its definition while displaying sorted source membership. Reopening
a reviewed save rebinds both preferences against the new native source before
publishing another page. Failed or canceled candidates leave the old handle and
page intact. Only admitted records have their sort values compared; explicitly
turning filtering off can therefore fail when previously excluded values enter
the requested order. The old filtered view remains usable after such a failure.

The combined headless batch passed 174 UI, 88 runtime and 38 desktop checks plus
the runtime exporter test. Desktop scenarios cover complete regional value
presentation, reverse physical lookup, filtered descending Find across pages,
clear/remove composition, and a reviewed edit/save/reopen that removes the edited
record from the retained filter. Runtime cases separately cover stable equal
keys, hidden nonfinite keys and fresh source rights. These checks establish local
service/host behavior, not native Windows Access comparison or window-manager
parity. One selected sort field is supported; multi-field ordering, query-result
Ribbon sorting and persisted Access OrderBy properties remain outside this slice.

Final strict seven-package Clippy, the minimal runtime Clippy build, formatting,
19-package dependency boundaries with 25 guard tests, and 7,741 local documentation
target checks passed. Follow-up filtering and uncached-mode Find checks passed
after the final admission/allocation corrections.

The retained private Home capture was reopened for comparison. Sorted synthetic
light/dark exports were inspected at 1000 × 680 logical pixels, 1.25 scale
(1250 × 850 physical pixels), German UI and `de-DE` regional settings, using a
macOS offscreen GPU build with Windows keyboard conventions. Both show the
centered header's descending indicator, enabled Remove Sort, disabled filter
controls when no predicates exist, and the unfiltered count with current sort.
The images are `.cache/ui-native-sort-review/de-light-sorted@1.25x.png` and
`de-dark-sorted@1.25x.png`; they remain untracked. No native application window
was opened. No retained Access capture shows this exact sorted synthetic dataset,
so this is visual approximation and headless interaction evidence only.

## Reviewed single-cell text Paste — 5 November 2026

Home's Paste text command requests the system text clipboard for the selected
native cell. Keyboard Paste on a focused native scalar cell uses the same typed
host action. Both open a fresh source-bound edit draft with the literal text as
the replacement; neither reviews, applies or saves it automatically. Runtime
validation, exact native planning and the existing consuming save worker remain
unchanged. A completed save reopens through the existing filter/sort restoration
path. This is a single-cell text operation, not Access private-format, multi-cell
or record Paste.

Empty text and the literal string `NULL` do not select stored NULL. Multiline
text remains one cell's input and is never split into records. Current invariant
numeric/ISO date entry rules apply; localized currency or date display text is
not silently parsed using the UI locale. The existing edit dialog shows the
replacement and its field-specific input hint before Review. Identical canonical
DATE/float input retains the target's existing carrier; this is not recovery of
precision from the clipboard source. Exact typed transfer remains planned, with
its [transport review](../development/DEPENDENCY_REVIEW.md#typed-clipboard-transport-review--5-november-2026)
recording the required private-format and platform boundaries.

Admission requires a current native scalar cell, current source/publication and
selection, no pending operation or conflicting dialog, and at most one MiB of
UTF-8 input. Oversized text reports an error without truncation. Read-only query
results, selected records, complex/calculated fields and stale native targets do
not acquire an edit action from Paste. Normal text widgets retain their ordinary
editing shortcuts.

The toolbar clipboard request captures its target, publication and focus. Since
egui's clipboard response has no request ID, an invalidated outstanding request
is retained until its response arrives and is discarded before text widgets can
consume it. Home cannot replace an unanswered request with another one. There is
no guessed timeout that could reinterpret a still-late response as a new paste.
An invalid response is reported; an unanswered OS request can keep Home Paste
disabled. This limitation does not claim native platform clipboard qualification.

Local acceptance passed 41 desktop and 178 UI cases in the batch, including the
five new Paste UI cases. Its remaining SQL-editor test initially assumed a single
global scrollbar; the corrected nearest-editor-scope case passed separately with
height, paint clipping, Run visibility and keyboard assertions intact. Desktop
checks cover actual native Paste/Review/Apply/save/reopen, literal localized-input
refusal with unchanged file bytes, and stale/oversized requests leaving the worker
unchanged. Two existing runtime input tests were extended and passed, preserving
exact-target DATE/float behavior and input limits. Strict seven-package Clippy,
formatting, dependency boundaries and documentation-link checks passed.

The retained actual Home screenshot was reopened. Current German/de-DE light and
dark datasheet exports were inspected at 1000 × 680 logical pixels, scale 1.25
(1250 × 850 physical pixels), using the macOS offscreen GPU build. The original
clipboard icon and two-line `Text einfügen` label fit the Clipboard group and
retain the reference's large Paste placement. Images remain untracked at
`.cache/ui-paste-review/de-light-datasheet@1.25x.png` and
`de-dark-datasheet@1.25x.png`. No native window was opened. The exact text-Paste
label and reviewed dialog workflow have no corresponding retained Access capture;
these renders establish local visual approximation, not platform clipboard parity.

## Object loading, retry and shortcuts — 5 November 2026

A table or Design object awaiting its first publication shows an object-specific
loading status while its read is busy. Idle or failed reads expose a Retry
control bound to that exact ObjectTab instance. Complete diagnostics retain their
existing selectable report; a failed fresh filter restore uses that report and
the failed status without creating a second error store. Existing published rows
remain visible during refresh, and a loaded zero-row result keeps its real
headings instead of becoming an unloaded object.

The host admits Retry only for the current unloaded object under the normal
modal, busy and save-owner guards. It dispatches the existing native read or
inert saved-query inspection, never query execution implicitly. A failed or
canceled retained filter/sort reapplication keeps its source-free definition;
only a matching accepted result removes it. Repeated retries cannot silently
become ordinary page reads. New-source post-save plain-page cursor preferences
are recorded separately from retained filtered view ranks, without assigning
row rights to an unloaded offset.

Microsoft's [Access shortcut reference](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
documents Ctrl+F6 / Ctrl+Shift+F6 database-window switching and Ctrl+W closing.
TableCraft maps these to its retained object tabs on Windows/Linux and supports
Ctrl+F4 there using the [general Windows document-window contract](https://support.microsoft.com/en-us/accessibility/windows/keyboard-shortcuts-in-windows).
Ctrl+Tab retains its separate dialog/pane meaning. The macOS Cmd+F6 / Cmd+Shift+F6
and Cmd+W mapping is a TableCraft adaptation; plain macOS Control+W remains text
word deletion. Exact modifiers are required.

Recognized object chords are captured before text widgets. Source/publication,
active instance and readiness are checked again after drawing before emitting
one host action. Held-key repeats do not successively close or cycle newly active
objects in this engineering profile. Refused recognized close chords are consumed
and issue CancelClose, preventing a key event from becoming an application-window
close. A platform that sends only a native menu/CloseRequested event without the
corresponding key cannot be classified by this shared layer. Native OS/menu
behavior remains unqualified; this is not a claim of complete Access shortcut or
window-manager parity.

All 189 UI and 48 desktop tests passed. New checks cover actual controls for
loading/empty/error states, exact Windows/Linux/macOS key conventions with live
TextEdit ownership, guarded close behavior, native cancellation followed by
same-tab recovery, repeated corrupt-page failure, stale/loaded-empty refusal and
repeated filter-declaration failure while an ordinary page would be readable.
These tests reuse existing independent fixtures. The 59-scene × eight-locale ×
two-theme matrix passed (944 combinations); strict seven-package Clippy passed.

Four German/de-DE `object-loading` / `object-error` PNGs were opened and inspected
in light and dark themes at 1000 × 680 logical pixels, 1.25 scale (1250 × 850
physical pixels), using the macOS offscreen GPU renderer. They remain ignored
under `.cache/ui-object-status-review/`. Loading exposes the actual object name,
spinner and existing Cancel; failure exposes the complete diagnosis and Retry.
The retained actual Home capture was reopened for placement. No matching Access
loading/error capture exists, so these status surfaces remain provisional visual
approximations. No native application window was opened. The loading scene is
classified as animated and exports a bounded frame sequence without waiting for
an intentionally perpetual spinner to settle.


## Common field filters — 5 November 2026

Home Filter and the column menu now describe the selected native field before
opening a typed comparison editor. Numeric/date comparisons, inclusive Between
and Not Between (including reversed bounds), explicit NULL tests and text
patterns use runtime services. Text equality and prefix/suffix/contains choices
honor the database's ANSI-89/92 wildcard mode; captured selection equality
continues to compare the literal stored value. Different fields combine with
AND; a replacement on the same field replaces that criterion. Sorting survives
filter changes. Unsupported field kinds refuse without a sequential fallback.

Metadata description does not decode user records. Applying a filter finishes
the complete native scan before publishing a new view. Failure, cancellation,
invalid input and stale replies preserve the previous rows and native rights.
The source-bound dialog blocks conflicting object/edit actions. Each input has
a 64 KiB UTF-8 ceiling; oversized text is rejected whole, including same-frame
cursor movement followed by paste, rather than silently applying a prefix.

Fixed numeric bounds use exact Decimal operands, including fractional bounds
outside the stored field's range. Unrepresentable bounds refuse. Floating
bounds must survive the documented canonical decimal spelling round trip;
this is an engineering policy, not proof of exact IEEE representation or native
Access coercion parity. Input uses invariant typed syntax independently of
regional display. Dynamic date presets, value-list filters, arbitrary SQL
criteria and broader unsupported field types remain unfinished.

The retained Home capture was opened for control placement. No exact common
filter popup capture exists; its layout remains a visual approximation.


## Datasheet presentation and shell follow-up — 5 November 2026

Startup follows the system theme unless explicitly overridden. Opening a database
activates its first local, non-system table. The condensed title shows the database
name, filename and file-compatibility label; hover exposes the captured absolute
path. That label does not identify the Access application that last saved it.
Windows/Linux use the shared title controls without native decorations; macOS
retains its native window frame. Cross-platform window interaction is unqualified.

All Objects navigation retains the taller search field and magnifier. Object tabs
have individual Close actions. The bottom status strip retains the current view
on the left and guarded view switches and zoom on the right. Datasheet cells use
typed NULL markers, centered native checkboxes, centered captions and a column
menu chevron. Stored empty text remains empty; it is not converted to NULL.

The final native page now offers a separate New row action when the existing
insertion workflow is admitted. This is a command, not an appended record or a
new native row identity. Home already exposes supported Copy/Paste, filtering,
sorting, Refresh, New/Delete, Find, navigation and Zoom. The later
[Totals implementation](DATASHEET_TOTALS.md) adds complete-view aggregates.
The later current-occurrence Replace workflow below enters the normal reviewed
edit service. General Cut, Replace All, spelling and text-formatting services
remain unfinished.

Save, Undo and Redo are visible in the title strip with explicit unavailable
explanations. There is no document undo history or inline dirty-record Save
service; reviewed offline edits continue to use their consuming Review/Apply
workflow. Field filters have their own status, and Snapshot browsing wording has
been retired. Original artwork now uses a stronger red database cylinder and a
rounded settings gear; the generator and provenance manifest remain authoritative.

Date and time retain independent System locale defaults, explicit alternatives
and Raw. Options now previews each alternative and the current currency's symbol
and minor units. System locale uses captured host regional CLDR conventions;
custom OS date/time overrides are not read. Interface translation does not select
the regional locale, and display choices do not alter storage, query or edit rules.

## Reviewed current-occurrence Replace — 5 November 2026

Home Replace opens the modeless Find and Replace window. Find Next searches the
complete admitted native view. Replace requires the still-selected successful
match with unchanged search options and view. Runtime rechecks that occurrence
against raw source data and creates a normal offline edit draft; the UI does not
splice displayed strings. The draft then requires Review change and Apply to
original file through the existing journaled save worker. This extra review is a
current workflow difference from Access's immediate Replace command.

Raw Text and unformatted invariant integers are supported subject to native write
admission. Replacement characters are literal. Empty replacement can remove a
matched text span; literal NULL does not select typed NULL. Lookup controls,
stored NULL matches and unsupported regional numeric/date conversions visibly
refuse. Replace All still requires a separate complete-view batch review and
transaction; this command never loops independent saves.

A successful draft hands its request to the ordinary edit owner, so Cancel before
Review closes it without saving. Closed or superseded Find windows cannot accept
a late replacement draft. The original file stays unchanged until explicit
Apply succeeds. The macOS/Linux host advertises this capability; unsupported
platforms retain an explicit boundary. Native Access interaction is unverified.

## Compact presentation follow-up — 5 November 2026

The title strip begins with the red database icon and TableCraft. Quick Access
adds the existing file chooser and session-only visibility customization; general
document Save/Undo/Redo remain unavailable. Document naming currently uses the
source filename stem and literal Database kind label, not a custom Access AppTitle.
Both ACE 16 storage families display Access 2016 as the title compatibility
convention. The precise Ace2016/Ace2019 profile stays available in inspection.

Native grids end with an inert Click to Add placeholder. Its header cannot select,
sort or copy a nonexistent field, and it has no stored column/row identity. Tables
can collapse independently of saved queries. Record controls now use compact
icons, editable position and an honest known/unknown count; the existing paging
controls remain in Page tools. Grid/ruler view icons and a thin zoom track follow
the retained capture. The separate Zoom viewer preserves its read-only/reviewed
editing contract while moving Font and dismissal controls below the body.

Six synthetic Ribbon tabs have grouped disabled command previews. Existing
commands retain their typed services; no database functionality was expanded.
Current visual evidence and headless verification are in
[presentation preview](PRESENTATION_PREVIEW.md#compact-shell-record-strip-and-full-tab-previews--5-november-2026).

## Table Design inspection layout follow-up

The shared inspector now follows the retained field grid, selector gutter and
General/Lookup property layout. A full-width divider supports dragging and
focused Up/Down adjustment. Inert blank grid rows never select a nonexistent
field. Property values support full-value keyboard Copy with a visible bounded
refusal; field/publication changes do not transfer an old focus to new metadata.
Lookup remains uninspected and schema editing remains unavailable. See the
[presentation review](PRESENTATION_PREVIEW.md#table-design-grid-and-property-inspection--5-november-2026).

## Query Design presentation follow-up

The existing bounded visual query editor now uses compact source cards, explicit
join connectors and a resizable source/grid split above ruled QBE cells. Blank
columns remain outside the draft; only the existing Add expression/field actions
create projections. Centered Show controls retain hidden-field criteria. Add
Tables preserves local user-table selection and cancellation, with unsupported
source-kind tabs disabled. Existing explicit build/Run and stale-response guards
are unchanged. See the [presentation review](PRESENTATION_PREVIEW.md#query-design-layout-follow-up--5-november-2026).

## Localized schema presentation labels

The Design grid localizes recognized host-supplied Access presentation names and
separately describes qualified reader carriers. An `Int32` reader does not prove
an AutoNumber declaration; absent presentation metadata uses a descriptive
32-bit-integer label with a reader-only explanation. Unknown labels stay literal.
The UI does not decode native type codes or flags to invent a presentation name.

Known host-authored Field Size summaries localize independently of stored
property text. Numeric sizes, unknown summaries, field names, captions,
descriptions and stored expressions remain literal. Native Details retains exact
carrier identifiers, hexadecimal codes and other diagnostic values. No native
metadata scan, storage behavior or editing capability was added by this change.

German Field Size terms follow Microsoft's [field-size reference](https://support.microsoft.com/de-de/access/set-the-field-size),
which itself uses Byte, Integer, Long Integer, Single and Double. Other bundled
translations remain subject to the existing native-speaker review boundary.
