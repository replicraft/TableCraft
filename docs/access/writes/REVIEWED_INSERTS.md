# Fresh qualification of reviewed native INSERTs

This working query/engine/adapter boundary now supplies the runtime and desktop
SQL INSERT VALUES and qualified SELECT-append workflows. Inputs are reviewed
before an exclusive offline lease is acquired. The shared Append query warning requires explicit confirmation,
and save consumes the sealed candidate without reevaluating SQL or host inputs.
Existing datasheet insertion and legacy CLI insertion remain separate consumers;
their current restrictions are unchanged.

## Typed VALUES and original presence

`plan_insert_with_options` and `plan_insert_with_omissions_and_options` accept
complete typed parameter inputs through `MutationOptions`, with independent query
and backend allowances. Declarations, names, carriers and length/precision
constraints are checked before one consuming finish, including unused inputs.
Written VALUES order is preserved even when fields reorder into backend schema
order. A later expression error cannot publish a partial row.

VALUES binding has no input relation or row/identity source. Only explicitly
declared, unqualified parameter names enter the existing closed expression
profile. Conversion constructors remain the existing finite set; this does not
enable Date/Now calls, UDFs, domains, dynamic Eval or subqueries in VALUES.
SELECT append uses a separate retrieval route; it does not widen VALUES binding.
SQL-looking parameter text is literal data. Destination field policy arrays are
refused rather than applied as provenance to fresh literals/parameters; their
comparisons use the explicit default provider.

Supplied NULL, supplied empty values and omitted fields remain distinct. Named
omissions are selected structurally before evaluation, without retrying a failed
complete plan under another policy. Generated companion schemas retain their
explicit field-list requirement. Defaults and generated values remain native
work; no field is guessed from its spelling or position. Existing entrypoints
without typed input contracts still refuse declarations, with their original
NoBindings evaluator and accounting.

The Access `plan_recoverable_insert_with_options` consumer uses actual native
metadata and selects complete or omission intent once. Query work pays shape,
binding, evaluation and output vectors; native work pays metadata and finish.
An optional host clock authorizes only needed source-qualified stored defaults,
not explicit clock expressions. Invalid VALUES cannot reach that finish/provider.

## Complete SELECT staging before native finish

The separate `prepare_native_insert_select` adapter consumes a metadata-only
`NativeJournalInserter` and returns a `PreparedNativeInsertSelect`. Actual target
metadata and field mapping are established before source retrieval. It selects
complete or omission handling once, without fallback; the omission finish can
use the explicit default clock only for needed qualified defaults. See the
[backend-neutral staging contract](../../query/INSERT_SELECT.md).

Runtime supplies the original stable `NativeSource` and its existing query
providers, database text comparison and LIKE policies. Retrieval, named-schema
discovery and stored assignments share one cumulative query allowance. Native
source callbacks borrow the same native allowance used for capture/metadata.
Only after preparation has returned does consuming `finish` borrow that allowance
for one native batch finish. No source scan runs while that mutable borrow is held.

All required original retrieval work completes before assignment/native finish,
including EOF, UNION promotion, sorting and eager errors. Self-append cannot read
an inserted prefix. Supplied NULL remains supplied and omissions remain omitted
for every row. Valid lossless Strings publish only at final stored Text assignment;
non-storable states never collapse to NULL, zero or numeric error codes. A late
failure publishes no appended prefix and never calls native finish. Empty
retrieval still invokes complete native admission and can produce a zero-row plan.
Native source, macro, relationship, index and generation gates remain unchanged.

## Five sealed native families

Every `NativeJournalInsertPlan` variant now supports fresh requalification using
independent native and proof pools, retaining its exact original family:

| Family | Original intent | Retained behavior |
| --- | --- | --- |
| Complete | TCNA1 | Exact complete supplied rows |
| AllocateEof | TCNA3 | Original represented EOF DATA allocation |
| CompleteTyped | TCNA4 | Source-selected fixed or growing complete rows |
| Omitted | TCNA2/TCNA6 | Original supplied/omitted field presence |
| Clock | TCNA5 | Original presence and captured civil clock input |

Each engine wrapper checks complete fresh BEFORE equality and original digest,
then calls only its corresponding normal planner. A shared final proof compares
both complete rebuilt images and the inserted count before returning the original
intent with the rebuilt plan. A failed family is never retried as another family.
No SQL is replayed and no host clock is resampled. Stored deterministic defaults
are reconstructed from the identical bound source, with the retained clock
reading where applicable; they are not replaced by expanded SQL values.

`OfflineWriteSession::stage_reviewed_insert_with_budgets` captures from its retained
exclusive target, checks the whole original and consumes this proof. It creates
no journal before returning the existing `StagedInsert`; explicit apply retains
the existing fixed/resizable journal, recovery, receipt and retirement ownership.
Failures retain that same session, lock and receipts. Hosts must not hold a native
lease while waiting for review or confirmation.

## Runtime and shared Append confirmation

`ReviewQueryInsert` accepts the same open document's immutable `InspectedQuery`
and literal typed parameter inputs. It validates source identity, document
revision, input limits and declaration constraints before native capture. The
parsed shape selects one local VALUES row or qualified SELECT append through
separate routes. Multiple VALUES tuples, external destinations and broader action
contexts do not become supported through classification or parameter binding.

The worker's existing clock is supplied only for source-qualified needed stored
Date/Now defaults during native review finish. SELECT append finishes complete
retrieval first; its explicit query providers retain their read-query context.
Complete supplied values, supplied NULL, empty requests and unneeded defaults do not sample it. The native source/declaration
checks still precede the at-most-once provider call. Its captured civil reading
or exact absence remains in the original Clock-family intent; save and recovery
use that input without another clock call. This does not establish native Access
clock timing or generated/default-clock composition.

The response separates `QueryInsertReview` facts (table, document revision,
inserted rows and review ID) from the opaque `ReviewedQueryInsert` capability.
INSERT has no matched-old-row count. Shared UI captures the expected Insert kind
and confirmed SQL/parameter witness, then accepts only the corresponding typed
`Insert { inserted_rows }` effect. VALUES reports one record; SELECT append can
report zero or multiple records without a presentation-imposed ceiling. A display
DTO cannot construct a native plan or select another save operation.

`SaveWorker::start_query_insert` preserves the exact selected native plan variant
through fresh offline staging. The common action-save owner uses its INSERT
branch, not UPDATE or DELETE decoding. Original presence, native defaults,
checked generated values and captured clock input retain their owning contracts.
Source drift, cancellation or proof failure retains the same real session and
receipts; no review holds an exclusive lease while waiting for confirmation.

Before save acceptance, No/Escape preserves previous query results and changes
no bytes. After acceptance, cancellation is request-only and terminal outcomes
survive view changes until acknowledgment. Successful acknowledgment freshly
opens the database, restores the SQL tab/draft and clears old snapshots without
rerunning INSERT or inventing a generated row identity. See the
[runtime service](../../application/RUNTIME.md#reviewed-sql-insert-actions) and
[shared scratch action workflow](../../ui/SQL_WORKSPACE.md#reviewed-local-action-queries).

Microsoft's [INSERT contract](https://support.microsoft.com/en-us/access/insert-into-statement)
distinguishes a single VALUES record from SELECT append, and requires a complete
positional row when no field list is supplied. Its
[append guide](https://support.microsoft.com/en-us/access/add-records-to-a-table-by-using-an-append-query)
describes preview and no Undo, and its
[action-query confirmation setting](https://support.microsoft.com/en-us/access/customize-default-settings-for-your-databases)
includes append queries. The current warning shows the qualified native inserted
count and no-Undo text. Exact native append-warning wording and layout have not
been captured, so the warning remains an adaptation; a separate append-record
preview is not implemented.

## Evidence and limits

Focused query tests cover strict typed/reordered inputs, original presence,
generated field-list restrictions, eager errors, split pools, cancellation and
hidden-source/policy refusal. Native staging tests cover all five families and
both complete/EOF growth paths. Fixed/default, growth and captured-clock AFTER
images have independent raw fixture oracles in `tablecraft-testkit`; the clock
fixture proves local retained-reading mechanics separately from the still-pending
source18 native/SDK qualification. Growth uses a literal 255-slot native17 source
and represented free EOF page, not a widened admission rule.

Additional checks retain original intent bytes and refuse substituted requests,
source drift, exhausted pools and entry/middle/terminal cancellation. This does
not establish native Access or non-macOS execution parity. Initial rejected
fixture/expectation runs remain in ignored verification logs.

Current normal generated-field restrictions, required-program gates and
[clock-profile limits](NORMAL_CLOCK_DEFAULTS.md) remain explicit. Microsoft’s
[INSERT contract](https://support.microsoft.com/en-us/access/insert-into-statement)
also describes explicit AutoNumber values; those remain outside the current
supplied-generated-value profile. SELECT append now uses actual staged retrieval
and native batch finish, rather than parser acceptance alone. Current generated
field policies, wider clock request grammars and generated/default-clock
composition stay within their existing native admission. Broader append query
designer, saved-action persistence, target/expression contexts and remaining
native-profile qualification remain roadmap work.
