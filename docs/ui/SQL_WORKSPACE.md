# Scratch SQL workspace

The scratch SQL object separates the editable draft from the text that produced
its displayed results. Explicit Run uses the existing read-only runtime query
service. Editing the draft, selecting a cached result view or switching object
tabs does not execute SQL.

Scratch and saved retrieval execution share the runtime's
[query context](../application/RUNTIME.md#read-query-execution-context): lazy local
Date/Time/Now and admitted Timer projections, a per-Run default-seeded Rnd
sequence, unsecured CurrentUser metadata and an explicit English Error catalog.
Those providers do not authorize writes, select database collation or turn
display formatting into SQL conversion policy. Scratch parameter prompts,
action-query execution and native saved-definition editing remain unfinished.
The [shared runtime parameter service](../application/RUNTIME.md#scratch-query-parameter-service)
now provides inert declaration inspection and source-bound typed execution;
that API alone does not connect a desktop prompt.

## Shared parameter presentation

`workspace::query::parameters` owns the renderer-neutral parameter DTOs, literal
input admission, typed controls and staged prompt body. The existing saved-query
consumer retains its own target, publication and original-input checks. Scratch
prompt ownership and desktop routing remain the next integration step.

Saved prompt declarations, original/draft inputs, target names and pending input
vectors now count their allocated capacities toward the shared 64 MiB retained
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
ownership across tabs. A future inspection/prompt consumer can compare exact SQL
and revision, together with its captured tab/document/publication identity, to
reject A→B→A edits. The runtime's issued target independently binds source identity.
Direct host assignment to the public draft still requires the exact-text check;
hosts that need edit history must use `replace_query_sql`.

Revision exhaustion refuses new captures/runs and disables further editor input
without wrapping, replacing text or dropping prior results. QBE and layout
replacement refuse before publishing their replacement state.

This witness does not change submitted execution results: an older completed Run
can still publish its captured SQL and be labelled Previous after draft editing.
It does not connect a parameter prompt or authorize execution on its own.

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

## Verification and appearance

The 6 November 2026 headless gate passed 51 focused UI cases, all 66 desktop
cases and strict all-target UI/desktop Clippy. Checks cover edited drafts,
request/source guards, memory refusal, mixed SQL/design origins, parking, reset
and the real worker's failure/cancel paths. Original fixture bytes remain
unchanged. These checks do not establish native Access equivalence.

No retained Access screenshot shows SQL View or its result-status messages.
Their placement is a TableCraft adaptation. See the [presentation record](PRESENTATION_PREVIEW.md)
for offscreen review; native window checks remain suspended at the user's request.
