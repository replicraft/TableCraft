# Crate ownership and shared-host parity review

Reviewed 9 November 2026 against the working source tree. This source/API inventory includes local
regression verification for the implemented MCP and table-DROP routes. It does
not establish native Access compatibility or Windows execution; supported source
profiles remain bounded and separate from the long-term target.

## Ownership conclusions

| Owner | Working responsibility | Keep out |
| --- | --- | --- |
| `tablecraft-core`, `schema`, `model`, `db` | Values/budgets; validated declared types; inert object metadata; snapshot/edit contracts | GUI, native page layout, host sessions |
| `tablecraft-io`, `crypto`, `formats` | Generic bounded I/O and durable publication; checked primitives; inert package/template codecs | ACE policy, query execution, application permissions |
| `tablecraft-ace` | Jet/ACE interpretation, physical ownership, preservation, encryption policy, native plans and transactions | Desktop, protocol state, presentation DTOs |
| `tablecraft-access` | Native browser/query source and checked edit/creation adaptation | Widgets or independent protocol permissions |
| `tablecraft-sql`, `expressions`, `query` | Owned syntax; scalar semantics; binding and execution against supplied contracts | Concrete adapter opens, filesystem ownership, GUI |
| `tablecraft-builders` | Headless table/query drafts, stable draft field IDs, checked syntax construction | Native source qualification or persistence |
| `tablecraft-display` | Renderer-neutral scalar display facts and regional formatting | Input parsing as an edit side effect, native handles, widgets |
| `tablecraft-runtime` | One native document session, bounded application reads, literal input conversion, consuming reviews, workers and terminal persistence facts | egui, desktop dialogs or a second protocol-only write engine |
| `tablecraft-ui` | Shared widgets, presentation drafts, exact interaction ownership, accessibility and Fluent catalogs | File opens, native capability construction, durable saves |
| `tablecraft-desktop` | Window/dialog/platform integration; action dispatch; host-owned review/save/reopen lifetime; presentation conversion | Native planning, scalar parsing, transaction policy |
| `tablecraft-mcp` | Explicit local stdio protocol; scoped sessions; JSON DTOs and opaque capability handles; runtime worker composition | GUI dependencies, native planning, JSON-created consuming capabilities |
| `tablecraft-testkit` | Reusable independent fixtures and feature-gated observations | Production dependency edges and private subsystem glue |

The existing runtime is the useful boundary for an additional MCP host. Its normal
dependencies contain neither egui nor eframe. Desktop already uses it for cell,
row, table, field, query and database-options operations. No new edit-core crate
is necessary simply to host the same services twice. Keep protocol JSON schemas
and session registries in the new host, not in runtime or the engine.

Desktop's `editing/field_save.rs` holds real actor/report and presentation-reopen
composition shared by several UI operations. Its filter restoration, tab reload
and UI identity checks are desktop concerns. Copying that module into MCP or
moving its UI state into runtime would cross the boundary. MCP instead retains
actual runtime capabilities and witnesses and uses the same `SaveWorker` API.
Both hosts currently classify table-rename/table-creation schema receipts in
small private helpers. Sharing those operation-specific outcome-facts methods
inside runtime is a concrete follow-up now that two real consumers exist. It
needs no new crate, protocol state in runtime or generic changeset framework.
Field/query/options paths already consume runtime-owned outcome-facts methods.

The CLI intentionally consumes lower-level Access/query/creation services directly
and does not depend on runtime. This is not evidence that its advanced commands
are available through desktop/runtime. Encryption management, explicit external
source mapping, progressive TSV queries and detailed native diagnostics need
separate shared service work before claiming MCP parity with those CLI features.
SQLite, native projects, scripting and VBA execution remain paused/deferred.

## Required MCP service surface

The listed operation families are now implemented in the local MCP adapter,
including Find/Replace, filter/sort/totals and headless query design. Names below
are adapter operation names, not native format guarantees. Every
write still uses the runtime's admitted source/profile, actual review and exact
current revision. A listed service is not full Access feature compatibility.

| Workflow | Runtime request / authority | Implemented MCP surface |
| --- | --- | --- |
| Session status | Host-owned published revision and actual retained save/creation IDs; consumes no authority | `session_status`, including recovery after a lost operation-start response |
| Open and catalog | `Command::Open`, `Response::Opened` | `open_database`, `list_tables`, list saved query identities/capabilities |
| Rows and schema | `Page`, `Describe`, `InspectCollection` | `read_rows`, `describe_table`, `inspect_collection` |
| SQL and parameters | `InspectQuery`, retained `Arc<InspectedQuery>`, `RunQuery` | `inspect_query`, `run_query` |
| Saved reads | `InspectSavedQuery`, `RunSavedQuery`, exact `SavedQueryTarget` | `inspect_saved_query`, `run_saved_query` |
| Cell update | `BeginEdit`, `ReviewEdit`, sealed `ReviewedEdit` | `begin_edit`, `review_edit`, `apply_edit` |
| Row insertion | `BeginInsert`, `ReviewInsert`, sealed `ReviewedInsert` | `begin_insert`, `review_insert`, `apply_insert` |
| Row deletion | `ReviewDelete`, sealed `ReviewedDelete` | `review_delete`, `apply_delete` |
| Action queries | `ReviewQueryInsert/Delete/Update`, corresponding sealed capability | `review_query_insert/delete/update`, `apply_query_insert/delete/update` |
| Table rename | `ReviewTableRename`, `ReviewedTableRename`, `OpenTableRenamed` | `review_table_rename`, `apply_table_rename` |
| Table creation | Builder snapshot, `ReviewTableCreation`, `ReviewedTableCreation`, `OpenTableCreated` | `review_table_creation`, `apply_table_creation` |
| Field addition | `InspectFieldAdditionAvailability`, `ReviewAddField`, `ReviewedFieldAddition`, `OpenColumnAdded` | `inspect_field_addition`, `review_field_addition`, `apply_field_addition` |
| Field rename | Exact column name/native number inspection/review, `OpenFieldRenamed` | `inspect_field_rename`, `review_field_rename`, `apply_field_rename` |
| Field deletion | Exact column name/native number inspection/review, `OpenFieldDropped` | `inspect_field_drop`, `review_field_drop`, `apply_field_drop` |
| Saved SQL editor | `InspectQuerySqlDraft`, `ReviewQuerySql`, `ReviewedQuerySql`, complete expected-image witness | `inspect_query_sql`, `review_query_sql`, `apply_query_sql` |
| Restricted expression editor | `ReviewQueryLiteralExpression`, `ReviewedQueryExpression` | `review_query_expression`, `apply_query_expression` |
| Database options | `InspectDatabaseOptions`, `ReviewDisableNameAutoCorrect`, `OpenDatabaseOptionsSaved` | `inspect_database_options`, `review_disable_name_auto_correct`, `apply_database_options` |
| Blank database | `CreationRequest::blank`, `CreationWorker`, `OpenCreated` | `create_database`, creation status/cancel/acknowledge |
| Save lifecycle | `SaveWorker::poll/cancel/recover/release/acknowledge` | `save_status`, `cancel_save`, `recover_save`, `release_save`, `acknowledge_save` |
| Restart recovery | `SaveWorker::start_reserved_recovery` | Explicit recovery-start operation for a host-authorized source |
| Find/replace | `FindNext/ReplaceFindMatch`; retained actual occurrence; replacement returns ordinary edit draft | `find_next`, `continue_find`, `replace_found`, then normal review/apply; retained filtered/sorted views and explicit formatted policy |
| Filter/sort/totals | Actual native `FilterHandle`, inert `FilterDefinition`, metadata and complete view totals | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter`, `reapply_filter`, `sort_view`, `describe_totals`, `calculate_totals` |
| Query designer | `DescribeQueryDesign/BuildQueryDesign/ImportQueryDesign` and checked shared builder model | `describe_query_design`, `import_query_design`, `build_query_design`, `rebuild_query_design`; structured drafts, actual metadata and explicit SQL execution remain separate |
| Table deletion | `InspectTableDropSource`, `ReviewTableDrop`, sealed capability and `OpenTableDropped` | `inspect_table_drop`, `review_table_drop`, `apply_table_drop`; require exact eligible inspection and recheck native identity/count in the consuming review |

MCP must not expose disabled UI commands as apparently successful operations.
Unavailable source profiles, unsupported action-query syntax, unimplemented DDL,
forms/reports and deferred VBA remain explicit refusals or absent tools. Current
services do not establish arbitrary table/column creation, arbitrary ALTER or
full query authoring solely because SELECT and some mutations are supported.

## Local verification and remaining qualification

The consolidated local MCP batch passed 30 unit cases and one actual stdio
integration case after the wire-shape and cancellation follow-ups. The real-file cases cover
cell update/row insertion/deletion, table rename and saved-field deletion,
complete saved-SQL expected-image failure/retry, blank creation/table creation,
consumed/stale/refused capabilities, Find/Replace, filters/sorting/totals, inert
filter reapplication after a write, query-design import/build/rebuild and explicit
read execution. Host authorization/schema checks and the actual stdio process are
separate from native writer qualification. Test counts are a snapshot, not a claim
that every operation/profile combination received a new protocol fixture.

Filter handles and design IDs are session-owned and revision-bound. Successful
view replacement retires the prior opaque filter handle. A save retires native
handles while preserving the inert definition for explicit fresh-source
reapplication; ordinary document opening clears it. Design rebuild shares the
actual checked draft; neither JSON shape nor a builder draft supplies save
permission. Nullable optional protocol fields follow their typed optional/default
meaning, while required revisions, identities and literal values stay required.
Contradictory field-default or Decimal precision/scale inputs refuse.

Actual Windows filesystem/native execution and Microsoft Access behavioral
comparisons remain unverified here. Existing operation-specific source/profile
restrictions still apply on every host; local MCP parity does not complete full
Jet/ACE CRUD, arbitrary SQL DDL, forms/reports, VBA execution or
other deferred product features. Save/creation cancellation remains explicit. SDK request cancellation is now
scoped to one serialized call: queued cancelled requests do not execute, and the
session actor cooperatively invokes only `Worker::cancel` during read polling.
A revoked read publication retires document/review/view authority until explicit
reopening, without cancelling an accepted native save or asserting rollback.
`session_status` reveals retained save/creation IDs when a cancelled or lost
response did not deliver them. Dropping a client request does not erase outcomes
or checked AFTER witnesses. The additional cancellation checks remain separate
from the earlier recorded verification batch. The report introduces no new crate extraction
or additional backend permission.

## Worker usage and ownership

A session owns one `Worker`, one `SaveWorker`, and (when creation is enabled) one
`CreationWorker`. Construct them with a notification callback, not a GUI context.
The protocol actor may run synchronous session methods on its own thread; native
and expensive operations remain on runtime-owned workers. Do not run blocking
poll loops on an async transport executor.

1. Issue `worker.request(Command::Open { path, password })`, retain the returned
   read ID, and accept only its matching `worker.poll()` reply. Use the returned
   document revision and exact native object identities in later requests.
2. Read requests are **replaceable**: issuing another request supersedes the
   pending request/reply. Serialize session calls; this API is not a concurrent
   RPC queue. `publication_is_current(id)` and `cancel()` govern read publication,
   not persistence. A disconnected worker reports its real actor error.
3. Send `BeginEdit`, then `ReviewEdit` with the real draft ID and literal raw
   input. A successful response contains both review DTO and `ReviewedEdit`.
   Retain the sealed capability behind a session-owned opaque review ID. JSON
   returns facts, never the capability or a serializable substitute.
4. Explicit `apply_edit(review_id)` consumes that retained capability through
   `save_worker.start(capability, journal_path(source_path))`. Other mutations use
   their distinct `start_insert`, `start_field_drop`, `start_table_creation`, etc.
   A refused start returns the actual capability through `into_reviewed()`.
   Neither a client-supplied ID nor a DTO can construct a replacement owner.
5. Start returns an actor ID, not completion. `poll()` repeatedly returns the same
   retained terminal `SaveReport`; deduplicate by ID/report. Keep pending status,
   cleanup, retained recovery permissions and `Uncertain` separate. Cancellation
   only requests cancellation; it does not certify rollback or absent output.
6. `recover(id)` / `release(id)` consume only actual retained permissions and
   return a **new actor ID**. Rebind the host's retained owner to it. Do not retry
   the original write to implement recovery. `acknowledge(id)` may refuse while
   native resources remain retained.
7. Capture a schema operation's actual `reopen_witness()` before consuming its
   review. After real known-AFTER acknowledgement and release, issue its exact
   `Open*` witness command. Do not replace a failed expected-image read with
   `Command::Open` or publish a new catalog from a matching display DTO.
   Known-original/no-write, known replacement and uncertainty remain distinct.
8. For blank creation, call `creation.start(CreationRequest::blank(destination))`.
   Poll and retain the actual `Arc<CreationReport>`, check `CreationOutcome`, and
   explicitly acknowledge its exact ID. Use `Command::OpenCreated { report }` for
   guarded opening; a normal path-open fallback does not verify created bytes.
   Failed/uncertain publication never grants deletion or automatic retry.

Opaque session handles must be bounded, scoped to their session/current revision,
removed on consumption and rejected after replacement. Opening another document
must not strand an active save, retained recovery owner or creation report.
Follow [shared editing](../decisions/0002_SHARED_APPLICATION_EDITING.md),
[the runtime contract](../application/RUNTIME.md),
[dependency boundaries](../architecture/DEPENDENCY_BOUNDARIES.md) and
[the MCP direction](../interfaces/MCP.md). Authorization, parameter disclosure and
export scope remain host responsibilities; source content grants none of them.
