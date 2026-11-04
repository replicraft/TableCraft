# Native desktop preview

The first implementation provides native startup, Selawik workspace, table
navigation, bounded scalar datasheets and explicit read-only SQL. Runtime tests
exercise real reads/queries and cancellation; headless rendering covers empty and
populated states. Native visual/platform verification is recorded below. Broader
editing and application designers remain work.

Run with `cargo run -p tablecraft-desktop -- path/to/example.accdb`, or omit the
path and enter it in the window. Password input is masked and is consumed/erased
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

The initial datasheet is read-only. The SQL editor executes admitted reads only;
it is not an unrestricted action-query or scripting console. A later editing UI
must call the same typed command/session services as other hosts, with fresh
source/constraint checks, exact target capability, consuming revision/lease
semantics and durable recovery where required. No button may bypass unsupported
validation, data macros, locking or native-save restrictions.

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
