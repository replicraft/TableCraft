# Native CRUD, queries and Access parity

Scope accounting, 7 November 2026. This records the user's expanded target,
the inspected implementation and the remaining work; it is not a compatibility
announcement or a record of newly passing native tests.

## Target and completion rules

Jet 4 and ACE 12, 14, 15 and 16 are first-class targets for full native database,
table, column and row CRUD and Access query support. This supersedes the earlier
policy of expanding older formats only when they help ACE 16. ACE 16 remains the
modern application reference. Features must follow each target's actual
capabilities: older formats must not receive incompatible newer field types.
ACE 15 is an application compatibility target, not an invented file-header code.
Use actual stored layouts and Access-2013-produced evidence.

SQLite, native projects, Python and CRDT work remain paused. VBA execution remains
deferred to the standalone `vba-rs` direction; preserving existing modules and
detecting dependencies remain required. Access table data macros are in scope.
These deferrals do not establish full Access application parity.

Track completion per operation **and** profile, with separate engine, adapter,
runtime, CLI, GUI, recovery, platform and native Access evidence. A recognized
header, accepted SQL AST, disabled mock command, independent-reader result or
passing refusal is not a completed positive operation. Existing resource ceilings
are not permanent substitutes for general support. Bounded memory and cancellation
remain requirements even when supported database sizes grow.

## Current native CRUD matrix

All cells describe current implementation, not the requested final state.

| Operation | Jet 4 | ACE 12 | ACE 14 | ACE 15 | ACE 16 |
| --- | --- | --- | --- | --- | --- |
| Read catalog, schema and rows | Bounded | Bounded, including qualified complex reads | Ordinary reads; advanced qualification incomplete | Actual stored-layout coverage; application evidence incomplete | Broad bounded implementation |
| Create a database | Missing creator | Missing | Missing | Missing | Bounded scalar creator |
| Add a table | Missing | Missing | Missing | Missing | Qualified bounded creation ledgers |
| Rename/drop a saved table | Missing | Missing | Missing | Missing | Missing |
| Add/rename/alter/drop a saved column | Missing | Missing | Missing | Missing | Missing |
| Update rows | Restricted fixed non-NULL Int32 copy route | No normal writer admission | No normal writer admission | No independently qualified normal writer | Many qualified carrier/index/allocation profiles |
| Insert/delete rows | No equivalent normal routes established | Missing normal routes | Missing normal routes | Missing normal routes | Qualified normal routes; restrictions remain |
| Recoverable writes | Existing Jet 4 update integration | No CRUD-specific qualification | No CRUD-specific qualification | No CRUD-specific qualification | Original-intent journal/recovery for admitted actions |

Current normal ACE writes admit unencoded subversion-zero ACE 2016/2019 layouts.
Broadening that format match alone would not supply older-profile writers.
Encrypted reads and encryption-copy transformations do not supply encrypted
in-place CRUD. Unsaved Design fields can be added, renamed, changed and removed;
this is declaration authoring before creation, not saved-column alteration.

The actual mutation owners are
[`MutationPlan`](../../crates/backends/tablecraft-access/src/editing/offline/plan.rs),
the [ACE mutation profile](../../crates/engines/tablecraft-ace/src/table/mutation/ace_profile.rs)
and the [runtime save contracts](../application/RUNTIME.md).
Read qualifications remain in [native reading](../access/NATIVE_READING.md);
writer qualifications remain in [native writes](../access/writes/NATIVE_WRITES.md).

## Remaining native implementation

| Workstream | Work that still needs implementation or broader qualification |
| --- | --- |
| Profile coverage | Original Jet 4/ACE 12/14/15/16 fixture inventories; actual version/features/encryption provenance; positive and negative write/recovery evidence for each family. |
| Database lifecycle | Creators for the other targets, full supported declarations, database options, encrypted writes and explicit safe file lifecycle. Removing a database file is a host operation with document/resource ownership, not SQL DROP DATABASE. |
| Saved table lifecycle | Reviewed rename/drop, catalog record replacement, namespace/collision rules, dependency handling, native object identity and allocation retirement. |
| Saved column lifecycle | Add/rename/change/drop; existing-row migration, field numbers/null masks/variable offsets, defaults, indexes, properties and dependency updates. Empty-table rewrites must preserve native history and unknown metadata too. |
| Rows and storage | General scalar and supported advanced-field CRUD, all admitted key families/combinations, DATA/INDEX/LVAL growth and contraction, allocation-map growth, page reuse and reclamation. Remove fitting-only/short-ledger restrictions through real storage work. |
| Defaults and identity | Omitted versus NULL versus supplied values, complete supported defaults, Increment/Random generation, reseeding/history and composition with constraints/indexes/allocation. |
| Constraints and relationships | Required/AZL, field/record validation, primary/unique indexes, foreign keys, relationships, cascades and supported NULL/text comparison behavior. Author and preserve their native declarations. |
| Required events | Complete applicable Before/After table-data-macro behavior, ordering and rollback. Current bounded positive BeforeChange support is not general event execution. Never silently skip required programs. |
| Transactions and collaboration | Multi-operation transactions, atomic action queries, user transaction semantics, conflict detection, Access-compatible shared-file locking and mixed-client coordination; retain original-intent recovery and terminal resource ownership. |
| Preservation | Keep VBA, forms/reports, queries, macros, linked-table metadata and unknown payloads intact through unrelated edits; distinguish preservation from executing or editing those objects. |

The first persisted-schema slice should be table rename on freshly qualified
sources, followed by the rest of table/column CRUD. Even that needs a genuine
catalog mutation plan, original intent, fresh reconstruction, dependency checks,
runtime review and reopen. A Navigation Pane rename with no persisted native
operation would not satisfy it. Neither parser nodes nor a “created by TableCraft”
flag confer mutation authority.

## Queries: current capability and missing work

SQL View is **read-only, not literally SELECT-only**. The
[runtime query path](../../crates/application/tablecraft-runtime/src/query.rs)
uses the shared executor for qualified SELECT, UNION and TRANSFORM/crosstab
queries. Native saved retrieval queries also have inspection/execution routes.
The [visual designer](../ui/QUERY_DESIGN.md) supports a narrower SELECT subset.
GUI Run does not execute action queries or DDL, and editing SQL does not persist
a native QueryDef. Query results currently buffer under 16 MiB/10,000-row limits;
table-browser paging does not provide paged query results.

The parser already models much more syntax than hosts can execute. Separate
neutral mutation planners and qualified native/CLI actions exist, but their
closed expression/target profiles do not establish general action SQL.
See [parser scope](../query/SQL_PARSER.md),
[retrieval execution](../query/QUERY_EXECUTION.md) and
[remaining read/query gaps](../testing/READ_QUERY_P0_GAPS.md).

| Query workstream | Required completion |
| --- | --- |
| Runtime context | Shared runtime scratch/saved execution now supplies a lazy local clock, per-Run Basic random state, unsecured session metadata and English reference messages. Numeric/date/week/code-page/casing and omitted-year policies still need explicit host contracts; native timing/RNG/session semantics remain unqualified. |
| Parameters and sources | Declared scratch and saved parameters now use shared runtime execution and staged GUI prompts. Remaining work includes native implicit-parameter discovery, broader named-query parameter scope, and deliberate linked/external/collection source bindings with permission/namespace ownership. Stored paths must not open targets implicitly. |
| Retrieval semantics | Full supported joins/subqueries, grouping/HAVING/TOP, UNION and pivot behavior, declared types, Variant/coercion/NULL/Error semantics, sort orders/LCIDs and database policy precedence. Finish the documented partial profiles rather than adding unrelated modern SQL. |
| Action queries | Shared reviewed UPDATE, append/INSERT and DELETE, parameters, assignments, INSERT SELECT, qualified join/subquery targets, affected counts, warnings, validation, atomicity and recovery. Reuse the same native services as datasheet edits. |
| Make-table and DDL | SELECT INTO/make-table and CREATE/ALTER/DROP TABLE/INDEX through real native schema/index owners, including dependencies and source-preserving failure. |
| Query definitions | Create/save/rename/delete native saved queries and their properties, parameters and dependencies; CREATE/DROP VIEW and supported PROCEDURE/parameterized saved-action/retrieval workflows. Not just temporary SQL editor sessions or reconstructed read-only definitions. |
| Visual query work | Grouping/totals, crosstabs and action-query modes, parameter/property sheets and bounded faithful SQL/Design round trips. SQL-only query kinds must remain SQL-only where Access requires it. |
| Result usability | Cancellable execution, progress, controlled budgets, complete materialization/paging or qualified progressive results, and Access-compatible query updatability/record identity where valid. Never give aggregate/UNION results invented native row handles. |
| Pass-through | Explicit provider connections, credentials/trust and remote execution/result/error contracts. This is part of the full query target, not functionality supplied by the native parser or a mock button. |
| Permissions and application context | WITH OWNERACCESS OPTION is parsed but currently refused by retrieval; implement actual owner-access/security policy rather than ignoring it. Forms/Reports/TempVars references need supplied application bindings. Custom callable contracts or eventual VBA integration must cover dependent queries; the current VBA deferral remains an explicit gap in any unrestricted full-query claim. |
| Built-ins | Complete argument/carrier/coercion, calendar/locale/context and error behavior in the [builtin tracker](../expressions/ACCESS_BUILTINS.md), with native qualification. VBA-only runtime functions remain deferred, not unfinished native SQL work. |

At this audit the builtin tracker has 144 Partial rows, one Full row and 16
Not planned rows. Rows can group aliases; these are not individual function counts
or a percentage-complete estimate. The remaining work is primarily semantic
completion/composition and evidence, not a few missing function names.

Microsoft distinguishes retrieval, action and SQL-specific queries in
[Introduction to queries](https://support.microsoft.com/en-us/access/introduction-to-queries)
and [Run a query](https://support.microsoft.com/en-us/access/run-a-query).
Every action query needs the corresponding warning/confirmation/result workflow,
not an unchecked execution path attached to the read-only Run button.

## A good desktop v0.1.0

The existing [desktop checklist](../releases/DESKTOP_0_1_0_CHECKLIST.md) describes
a bounded preview. It does not already promise full CRUD/query across all five
targets. The expanded compatibility program above is now active; a release must
say exactly which positive operations and profiles it actually completes.
The existing CLI version number is not proof of desktop readiness.

Recommended product milestone: a dependable native table/query workbench, with
database/table/column/row authoring and the corresponding native SQL workflows,
before attempting to ship the forms/report application builder. If v0.1.0 is
advertised as **full CRUD and full query support** for all requested targets,
every corresponding matrix cell and query workstream above is a release gate,
including pass-through. A smaller preview can ship with explicit exclusions, but
must not close this target or call its exclusions full support.

Beyond the implementation work, release accounting still includes:

- A frozen supported-operation/profile matrix, known limitations and desktop
  release notes matching the built artifact; visible refusal for unavailable
  operations rather than silent omission or discarded data.
- Real-file end-to-end journeys over registered original MDB/ACCDB inputs, not
  only generated fixtures: browse, parameters, queries, edits, schema changes,
  save/reopen, cancellation, corruption and recovery, with source/output hashes.
- Connected preservation, constraint, macro, encryption, stale-revision and
  failure tests for every advertised write path. Read success does not qualify writes.
- Native parameter-discovery and broader named/external source workflows beyond
  the connected declared scratch/saved prompts. Existing stale scope, Cancel,
  retained-memory and previous-result protections must compose with them.
- Usable save/dirty-record/undo workflows, source-bound confirmations and errors,
  keyboard/focus/clipboard/IME/accessibility, large-data responsiveness,
  persistent Options, light/dark themes and all eight sectioned Fluent catalogs.
- Desktop CI builds and complete artifacts, dependency/font notices, native
  icons/metadata, minimum OS/runtime-library qualification, signing/notarization
  policy and verified install/update/uninstall behavior. Existing archive tooling
  and CLI release CI are separate evidence; desktop publishing is not wired yet.
- Working installed file-open receivers before registering MDB/ACCDB handlers;
  cold/running-app opens, Unicode paths, encrypted/error inputs and coexistence.
- Execution on every claimed Windows/macOS/Linux architecture and filesystem;
  native Access open/edit/save comparisons and physical durability qualification
  for the claims they support. Headless/cross-compilation evidence remains distinct.

Native graphical checks remain suspended until the user resumes them. That does
not block implementation/headless progress or turn unrun platform checks into
passing evidence. Preserve the user's desktop packaging working files.

## Access application parity after the data workbench

| Area | Remaining product scope |
| --- | --- |
| Forms | Decode/preserve native bodies; layout and controls, designers, properties, binding, validation, focus/navigation, subforms, events and faithful imported layouts. Current Form metadata is not a working form renderer/designer. |
| Reports | Native sections, grouping, calculations, page layout, pagination, preview, printing/export, designers and report wizards. |
| Macros and startup | Native UI/embedded macros, non-VBA application actions, event dispatch, startup/options and custom Ribbon behavior, distinct from table data macros. |
| External Data | Native links and credentials, provider capabilities, import/export formats and saved tasks, connection management, relinking and corresponding wizards. CLI TSV output already exists; broad interchange does not. |
| Database tools | Relationship design, indexes/property sheets, analysis/dependency tools, compact/repair, maintenance and migration/split workflows with native preservation. |
| Security and distribution | Trust and execution policy, legacy security where applicable, credential handling, ACCDC signature verification, templates/packages, runtime/compiled application behavior and deployment. Encryption alone is not security parity. |
| Multi-user operation | Native Access shared-file locking, concurrent changes, conflicts, recovery and mixed-client behavior. This is not the paused native-project CRDT work. |
| Legacy Jet 4 replication | Identify/preserve replica metadata and qualify replica writes, synchronization and conflict behavior separately from ordinary shared-file locking. Native MDB replication is not the paused native-project CRDT feature. |
| VBA applications | Preserve and detect now; execution, VBA-specific DAO/ADO, editor/debugger and COM/Windows integration remain deferred. Full application parity cannot be claimed while these required application behaviors are absent. |
| Complete UI | Finish contextual commands, native defaults/restrictions, wizards, property editing, shortcuts and dialogs from actual Access references, with cross-platform accessibility and performance. |

## What “better than LibreOffice Base” must mean

Base already documents [tables/queries/forms/reports](https://help.libreoffice.org/latest/en-US/text/sdatabase/main.html),
[form authoring](https://help.libreoffice.org/latest/en-US/text/shared/guide/data_forms.html),
[report authoring](https://help.libreoffice.org/latest/en-US/text/shared/guide/data_reports.html)
and [interchange](https://help.libreoffice.org/latest/en-US/text/shared/guide/data_im_export.html).
TableCraft does not currently establish overall feature, reliability or performance
superiority over it.

Microsoft's [format comparison](https://support.microsoft.com/en-us/access/which-access-file-format-should-i-use)
distinguishes legacy MDB replication and user-level/workgroup security from
ACCDB. Full Jet 4 scope must account for them rather than applying newer-format
assumptions or treating a replication GUID field as replica-system support.

The differentiation target is faithful native MDB/ACCDB work across platforms:
open without a conversion detour, retain application objects, reproduce Access
SQL/validation and familiar workflows, and save files Access can continue using.
Prove that advantage with the same original corpus and tasks in Access, Base and
TableCraft: successful reads/edits/reopens, retained objects/bytes, query values and
types, failure/recovery outcomes, task completion, latency, memory, accessibility
and installation. Record versions/platforms and all failures; appearance and
synthetic benchmark wins alone do not establish product superiority.

## Delivery order and acceptance

1. Establish the per-profile operation/fixture ledger and repair shared GUI query
   context/parameters. Keep existing read/write regressions intact.
2. Build shared reviewed action-query dispatch and native persisted schema services,
   starting with table rename; carry older-profile row writers and creators as
   parallel workstreams rather than waiting for ACE 16 to become perfect.
3. Complete allocation/index/LVAL lifecycle and behavior composition so CRUD
   works beyond the current narrow layouts. Bring every requested profile through
   the same operation acceptance, with its actual format-specific behavior.
4. Finish native SQL/QueryDef persistence, GUI action/DDL results and QBE modes;
   close retrieval/builtin semantic gaps and result paging/ownership.
5. Close the release gates for the declared v0.1.0 scope; then expand the forms,
   reports, automation, external-data and application-parity workstreams.

For each delivered operation: independently specify before/after inputs; prepare
without writing; exercise success/cancel/refusal/stale input; verify native data,
schema and identities plus the opaque complement; cold-recover original intents;
reopen in a fresh host; run the actual GUI/CLI path; record per-profile and platform
evidence. Preserve native Access qualification separately until it is executed.
Progress means closing these operation/profile/host acceptance cells, not adding
test lines, parser variants or optimistic support labels.
