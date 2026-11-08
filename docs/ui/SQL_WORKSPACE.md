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
display formatting into SQL conversion policy. Declared scratch parameters use
the [shared runtime parameter service](../application/RUNTIME.md#scratch-query-parameter-service)
and the same staged dialog controls as saved queries. Action-query execution and
native saved-definition editing remain unfinished.

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
flow does not add transitive named-query parameter propagation or action SQL.

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
