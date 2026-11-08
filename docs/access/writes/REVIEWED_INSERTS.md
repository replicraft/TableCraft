# Fresh qualification of reviewed native INSERTs

This working query/engine/adapter boundary prepares INSERT VALUES before taking
an exclusive offline lease. It is not yet connected to a runtime SQL action
capability or the desktop SQL editor. Existing datasheet insertion and legacy CLI
insertion remain separate consumers; their current restrictions are unchanged.

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
enable Date/Now calls, UDFs, domains, dynamic Eval, subqueries or INSERT SELECT.
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
also describes explicit AutoNumber values and INSERT SELECT; this bounded route
does not claim either merely because the syntax can be parsed. The next consumer
is the shared runtime/GUI action workflow, with a real inserted-row count and no
invented matched-old-row count.
