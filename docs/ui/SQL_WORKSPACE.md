# Scratch SQL workspace

The scratch SQL object separates the editable draft from the text that produced
its displayed results. Explicit Run uses runtime-owned statement classification:
retrieval uses the read query service, while qualified local UPDATE, DELETE and
INSERT VALUES or SELECT append enter a separate review and confirmation workflow.
Editing the draft, selecting a cached result view or switching object tabs does
not execute SQL.

Scratch and saved retrieval execution share the runtime's
[query context](../application/RUNTIME.md#read-query-execution-context): lazy local
Date/Time/Now and admitted Timer projections, a per-Run default-seeded Rnd
sequence, unsecured CurrentUser metadata and an explicit English Error catalog.
Those providers do not authorize writes, select database collation or turn
display formatting into SQL conversion policy. Declared scratch parameters use
the [shared runtime parameter service](../application/RUNTIME.md#scratch-query-parameter-service)
and the same staged dialog controls as saved queries. Local UPDATE, DELETE and
qualified INSERT VALUES/SELECT use the reviewed action service described below.
Broader action contexts, DDL and native saved-definition editing
remain unfinished.

## Reviewed local action queries

Run inspects the actual statement, acquires any declared typed parameters, then
requests its source-bound runtime review. Desktop does not parse SQL or
reinterpret a failed read as a write. UPDATE confirmation displays matched
records and records with effective changes. DELETE confirmation displays matched
records and the native plan's deleted-root count. DELETE removes whole records;
it does not clear selected field values or erase retained payload bytes. The
INSERT warning is labelled Append query and displays the native plan's inserted
count, with no matched-old-row count. VALUES admits one record; qualified SELECT
append can report zero or multiple records. The UI does not invent a batch limit.

The shared `QueryActionReview` carries a typed UPDATE, DELETE or INSERT effect.
The presentation owner captures the expected `QueryActionKind` when review begins
and rejects a mismatched effect. Plain display facts never select a native save
operation: desktop retains a distinct consuming runtime capability for each kind.
The shared `query-actions.ftl` section provides common decision/save controls and
operation-specific titles, effects and confirmation text in every UI language.

Yes applies the reviewed operation. No/Escape before acceptance drops the
lease-free review and preserves prior results, SQL and selection. All action
warnings state that database changes have no Undo. Microsoft's
[update-query workflow](https://support.microsoft.com/en-us/access/create-and-run-an-update-query)
documents confirmation and no Undo. Its
[DELETE statement contract](https://support.microsoft.com/en-us/access/delete-statement)
describes whole-record deletion and no Undo, while the
[screen-reader query workflow](https://support.microsoft.com/en-us/accessibility/access/use-a-screen-reader-to-create-a-query-in-access-desktop-databases)
documents confirmation of the number of rows before deletion. The
[delete-query guide](https://support.microsoft.com/en-us/access/create-and-run-a-delete-query)
also describes inspecting records selected by the same criteria. Microsoft's
[append-query guide](https://support.microsoft.com/en-us/access/add-records-to-a-table-by-using-an-append-query)
describes preview and no Undo; the
[confirmation setting](https://support.microsoft.com/en-us/access/customize-default-settings-for-your-databases)
explicitly covers append action queries. Exact append-warning row-count wording
has not been captured here. The current warnings show reviewed counts; they do
not add a separate action-result preview. No corresponding local warning capture
is retained, so these dialog layouts remain adaptations rather than verified
Access replicas.

The original scratch inspection's SQL allocation, edit revision, tab/document
identity, publication witness and fully confirmed parameter inputs transfer into
one shared action session. They are not recaptured from mutable UI state. An
incomplete prompt or altered input envelope cannot enter review. Stale or duplicate
callbacks cannot replace an accepted review. After the save worker accepts the
opaque runtime capability, Cancel/Escape requests cancellation rather than
dropping ownership. Retained failures use the existing Recover/Release controls;
all terminal outcomes require acknowledgment from the same save owner.

After acknowledged success, the host reopens the actual source and restores the
SQL tab/draft, clearing obsolete query snapshots. It never reruns the action to
refresh results and never publishes affected counts as query rows. Query results
do not gain native row handles. Synthetic preview scenes use the same controls
but only canned DTOs and explicit mock outcomes; no database capability is present.

Existing native writer qualifications still apply. This does not enable older
write profiles, encrypted/package editing, joined/external targets, cascades or
additional expression contexts. Unsupported action/DDL kinds stop after
inspection without a write; unsupported action shapes fail before confirmation.
SELECT append uses the database query mode for LIKE where qualified; other action
contexts retain their existing explicit mode restrictions.
See the runtime contracts for
[UPDATE](../application/RUNTIME.md#reviewed-sql-update-actions),
[DELETE](../application/RUNTIME.md#reviewed-sql-delete-actions) and
[INSERT](../application/RUNTIME.md#reviewed-sql-insert-actions).

## INSERT presence, retrieval and defaults

Append accepts a single local `INSERT INTO … VALUES (…)` or a qualified
`INSERT INTO … SELECT …`. VALUES keeps its closed expression profile; SELECT
uses the existing read-query engine and its qualified providers, comparison,
LIKE and named-source contracts. This is bounded append support, not general SQL
or DDL. See [shared SELECT staging](../query/INSERT_SELECT.md).

Named fields may reorder or omit columns; positional appends must cover every
exposed field. Parameters remain literal typed inputs. Actual native target
metadata is captured before retrieval. SELECT reads the complete original source
under shared allowances, including self-append, and finishes required eager work
before assignment and one native batch finish. Late errors cannot append a
prefix; even an empty result must pass native admission.

An omitted field retains omission intent for every row. Supplied NULL and supplied
empty Text do not request a stored default. Native declarations own carriers,
defaults, generation and constraints. Current generated-field restrictions still
reject supplied generated values or NULL; Microsoft's
[INSERT contract](https://support.microsoft.com/en-us/access/insert-into-statement)
permits explicit AutoNumber values, but those are not supplied by these routes.

Needed, qualified Date/Now stored defaults use the runtime worker's clock at
native review finish, after complete SELECT retrieval. The default provider is
read at most once for the batch; the native clock family retains that civil input.
Confirmation, save and recovery never resample it or rerun SQL. Empty requests
and unneeded defaults do not sample it. Explicit SELECT clock expressions keep
their read-query context; explicit clock expressions in VALUES remain outside
its closed profile. Generated/default-clock composition retains existing limits.
This policy does not establish native Access clock timing parity.

## Declared scratch parameters

Explicit Run first inspects the bounded SQL for declared parameters. Inspection
parses declarations without scanning records or consulting execution providers.
Queries with no declarations continue from that same Run intent. Otherwise one
prompt appears per declaration; only final confirmation dispatches execution.

For example, with a database open, Run this SQL and enter `42`:

```sql
PARAMETERS [Minimum] LONG;
SELECT [Minimum] AS [Echo];
```

Input is literal data. VALUE declarations require a chosen concrete carrier;
NULL is separate from text. Type, length and precision validation remains owned
by runtime/query services. Unknown column names do not invent prompts, and this
flow does not add transitive named-query parameter propagation. Qualified local
UPDATE, DELETE and qualified VALUES/SELECT INSERT use these same prompts before
their separate Yes/No confirmation; unsupported action shapes retain their
explicit refusal.

Acquisition captures the exact SQL tab, document, edit revision, text and focus
publication. SQL A→B→A changes, tab/source replacement and stale replies cannot
run the previous inspection. Duplicate metadata or late failure callbacks cannot
dismiss accepted staging. The host retains the immutable runtime target; UI state
contains no native source or runtime capability. Fresh Run intents are rechecked
before clearing any accepted prompt or dispatching another request.

Inspection, staging, Cancel and errors preserve earlier complete results and
selection. They are labelled Previous during acquisition; after Cancel, the usual
SQL-origin status applies again. Only successful accepted execution replaces them.
An executing query can still publish its submitted SQL after later draft edits,
with those results labelled Previous. Modal commands and keyboard handling use
shared admission, including cancellation and conflicting dialogs.

## Shared parameter presentation

`workspace::query::parameters` owns the renderer-neutral parameter DTOs, literal
input admission, typed controls and staged prompt body. The existing saved-query
consumer retains its own target, publication and original-input checks. Scratch
acquisition has its own Inspecting/Prompt/Ready owner and uses the same controls;
desktop routes DTOs and retains the issued runtime target.

Saved prompt declarations, original/draft inputs, target names and pending input
vectors count their allocated capacities; scratch SQL and its staged body use the
same shared 64 MiB retained
presentation allowance. Clone admission happens before construction, with an
actual-capacity recheck. Editing includes the detached modal's memory and an
independently admitted current-value candidate. Rejection retains the previous
staged allocation; successful confirmation admits its action copies before
changing committed inputs or clearing saved results. The desktop checks actual
owned vector capacity before dispatching a worker request.

Ordinary Unicode Undo/Redo is preserved. Under extreme pressure where a current
value cannot be copied within the allowance, input is disabled for that frame
and Cancel remains available. The allowance covers retained presentation and
explicit staging copies, not all renderer caches or process memory. Existing
1 MiB logical parameter-input admission remains separate. No query semantics,
native write eligibility or saved-result success behavior is expanded.

## SQL edit witnesses

Actual scratch editor changes advance a checked presentation-owned revision.
QBE SQL replacements and workspace-layout restoration use the same owner;
identical replacement text is a no-op. The global scratch draft keeps its current
ownership across tabs. Scratch inspection compares exact SQL and revision,
together with its captured tab/document/publication identity, to
reject A→B→A edits. The runtime's issued target independently binds source identity.
Direct host assignment to the public draft still requires the exact-text check;
hosts that need edit history must use `replace_query_sql`.

Revision exhaustion refuses new captures/runs and disables further editor input
without wrapping, replacing text or dropping prior results. QBE and layout
replacement refuse before publishing their replacement state.

This witness does not change submitted execution results: an older completed Run
can still publish its captured SQL and be labelled Previous after draft editing.
An edit witness alone does not authorize execution.

## Result ownership

Before dispatch, shared UI state captures the submitted text under its retained
memory allowance. The pending witness binds that text to a nonzero request ID,
the exact SQL tab and the current document revision. Desktop associates the
runtime reply with that request; publication never reads the then-current draft
to guess what executed.

A successful publication retains result data and its SQL origin together.
Editing the draft during execution can therefore show the submitted query's
completed rows while marking them as previous results. Pending runs, errors and
cancellation retain valid earlier rows. Discarding an unpublished witness does
not discard the last successful result.

The status appears in SQL View and cached Datasheet results:

| Origin | Presentation |
| --- | --- |
| Captured SQL matches the draft, with no pending run or error | Results match the last successful run of this draft. |
| Draft differs, a run is pending, or an error is visible | Previous results; Run explicitly to update them. |
| No submitted-text witness | The SQL that produced these rows is unavailable; no draft equivalence is claimed. |

Query Design has a distinct origin carrying its validated submitted SQL and
existing design-revision witness. A scratch run cannot borrow an older design
revision to claim that its rows match the design. Saved-query results retain
their separate parameter and execution ownership.

## Retention and limits

Pending and successful SQL text count toward the existing 64 MiB shared retained
presentation allowance, including parked objects. The normal GUI preflights the
command copy and pending witness before allocating another draft copy. Result
admission checks text capacity and data both before and after display conversion;
refusal preserves the earlier data/origin pair.

Tab switching parks and restores provenance with its rows. Closing the SQL
object or opening another document clears its result ownership. Preference-only
workspace layout restoration does not restore results or their witnesses. The
allowance governs retained presentation; editor input and runtime SQL limits
remain separate.
Captured SQL is presentation state, not executable authority or tracing payload.

## Scoped larger-table reads

The **Read query limits…** control is available beside scratch/saved Run controls
and in Query Design/results. It edits host-supplied values for the next read run.
Apply validates them through the shared runtime; it does not run a query, retry a
failure, change SQL or discard previous results. Invalid values remain editable
with the supported numeric ranges in their diagnosis. Cancel/Escape discards only
the preference draft. Sectioned `query-limits.ftl` catalogs cover every UI locale.

The host initializes these values from actual `QueryRunOptions::default`, then
captures checked options for each accepted scratch inspection, Design build/run
or saved-query execution. Parameter acquisition cannot change the captured run.
Action-query reviews, native writes and their independent budgets are unchanged.
See the [runtime contract](../application/RUNTIME.md#captured-read-query-resource-options).

| Pool | Unchanged default | Owner |
| --- | --- | --- |
| Complete native scalar snapshot | 16,384 rows, 256-row internal batches | [`TableSnapshotLimits`](../../crates/engines/tablecraft-ace/src/table/page/cursor/limits.rs) |
| Cumulative native reads/allocations, rows and work | 512 MiB / 100,000 rows / 5,000,000 steps | [`QueryRunOptions`](../../crates/application/tablecraft-runtime/src/query/options.rs) |
| Cumulative query allocations, rows and work | 16 MiB / 10,000 rows / 1,000,000 steps | Same per-run options |
| Comparison bytes and work | 16 MiB / 1,000,000 steps | Same per-run options |
| Parameter/result presentation allocations and work | 8 MiB / 100,000 steps | Same per-run options, one cumulative pool |
| Immutable file image | 64 MiB, not changed by this control | [`Document`](../../crates/application/tablecraft-runtime/src/document.rs) |
| Submitted SQL | 64 KiB, not changed by this control | [`query`](../../crates/application/tablecraft-runtime/src/query.rs) |

These are cumulative consumer allowances, not format limits or reserved process
memory. Native rows include metadata; query rows count intermediate work rather
than only final results. Snapshot rows are bounded to 1,000,000; query execution
is bounded to 256 MiB, 1,000,000 rows and 100,000,000 steps. The other adjustable
pools permit at most 4 GiB, 10,000,000 rows where applicable and 1,000,000,000
steps. Comparison/presentation have no source-row allowance. No invalid value is
silently clamped by the runtime. Existing UI retention and protocol output caps
remain independent.

A fresh read-only MCP process over the retained 100,969-row sample refuses its
largest-table count under defaults, then returns 100,969 with explicitly raised
snapshot/native/execution/comparison allowances. The same inspected SQL target is
reused, and the original database SHA-256 is unchanged. This observes the shared
runtime, separately from desktop headless SQL/Design/saved-query cases over
independent synthetic files; it is not a native Access comparison.

Native-boundary and presentation limit errors identify their known resource
pool. Source/expression errors whose shared contracts erase pool provenance keep
their original typed diagnosis; they are not relabelled as a guessed execution
or comparison failure. Full pool-specific provenance remains a follow-up. A
failed run preserves prior complete results and requires explicit Run again;
there is no automatic larger retry or partial result publication.

## Verification and appearance

The 8 November 2026 batch passed 495 shared-UI and 127 desktop tests, followed by
the additional preview-animation regression in a four-case focused group. Six
connected desktop cases include typed retrieval over Jet 4, ACE 2007 and ACE 2016
inputs, with original bytes unchanged. Strict lint, formatting, package boundaries
and five nonnative target compilations pass. Native Access comparison remains
separate. The [offscreen record](PRESENTATION_PREVIEW.md#scratch-parameter-acquisition--8-november-2026)
captures the new scenes and visual limitations.


The 6 November 2026 headless gate passed 51 focused UI cases, all 66 desktop
cases and strict all-target UI/desktop Clippy. Checks cover edited drafts,
request/source guards, memory refusal, mixed SQL/design origins, parking, reset
and the real worker's failure/cancel paths. Original fixture bytes remain
unchanged. These checks do not establish native Access equivalence.

No retained Access screenshot shows SQL View or its result-status messages.
Their placement is a TableCraft adaptation. See the [presentation record](PRESENTATION_PREVIEW.md)
for offscreen review; native window checks remain suspended at the user's request.
