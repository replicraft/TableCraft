# Saved-query authoring and guarded persistence

## New query Save and Save As — 13 November 2026

For [admitted native sources and declarations](../access/queries/NATIVE_QUERY_CREATION.md),
the desktop connects scratch SQL Save, checked Design Build-to-Save and authored
saved-query Save As to the shared creation review, consuming save worker and
guarded reopen. Save As uses the current authored SQL, including a dirty saved
editor; copying the stored definition remains a separate Runtime request.
Ordinary existing-query Save retains the TCQE expression-replacement route below.

A new query first asks for its name. OK starts the source-bound review and save;
there is no additional mutation confirmation. Cancel issues no native request.
The UI retains exact tab, document, text and Design revisions. Submitted SQL is
shared immutably between the issued request and worker. Unsupported input/source,
failed review and failed reopen preserve authoring text; stale replies cannot
attach to a newer editor. Native resources remain owned through actual recovery,
release and acknowledgement outcomes.

Only a Created-only receipt followed by the same complete-image/native-identity
witness can publish the new query. Scratch Save rebinds the same UI tab to the
fresh saved identity. Saved Save As retains the original dirty source tab and
opens a distinct created tab. This is the current safe ownership rule; native
Access tab-handoff behavior has not been observed.

Saved Design is a genuine object-owned editor, with its own SQL, draft, raw input
and preferences. It survives parking and fresh publication in Design mode. Run
builds current authored SQL rather than executing the stored definition behind
unsaved grid changes. Native saved-SQL review failures remain visible in Design
and its Results view.

Exact typed-draft equality determines Design changes; request revisions still
advance independently for freshness. Unchanged Build/Run and explicit Field Cancel
back to the baseline stay clean. Running an edited draft does not mark it saved.
Importing already-dirty saved SQL does not substitute that proposed text for the
stored baseline, and successful Save As resolves only its actual destination.

Changed Design and nonempty scratch SQL close through exact Save/Discard/Cancel.
Unfinished Field text must first be completed or explicitly cancelled. Discard
drops its exact checked owner without pretending an unbuilt model was saved.
Scratch SQL has no stored definition: Close-Save goes directly to naming, while
Design Close-Save first builds its current model. Name cancellation, refused
admission, failed review and nonreplacement cleanup cancel the original deferred
Open/Exit; a later unrelated Save cannot complete that close.

SQL View owns its current authored text even when an older typed model or cached
Design result remains. The current Results view carries an explicit Design or SQL
owner, established by the accepted Build/Run or selected Datasheet transition;
historical row provenance cannot choose the current authoring model. Design Run
owns its pending Results view before rows arrive. SQL-owned Results keeps current
raw SQL for Save/Close-Save, while Design-owned Results keeps its typed draft.
Saved SQL View uses its existing original/proposed close and TCQE save route.

Created publication preserves exact authored original/proposed strings and their
text epoch. Ordinary edit eligibility comes from a separate real Runtime inspection,
not from the Created receipt or display/canonical formatting. That observation may
retain canonical stored SQL for later source comparison without rewriting authored
text or Design. A later inspection compares against the actual stored observation;
a changed source still refuses. Successful TCQE replacement retires that observation
and reacquires the new stored fact. Stale or failed replies cannot publish it.

**Native scope:** creation currently admits one source-free SELECT ALL signed
Int32 literal with an explicit output alias, without parameters or other clauses.
Qualified unencoded ACE 16 sources may retain protected local tables and known
property storage. When MSysQueries is absent, native construction can initialize
that exact system role under complete source, allocation and byte-preservation
checks. Existing system DATA/index topology, append capacity, active/opaque
payloads and the other [source restrictions](../access/queries/NATIVE_QUERY_CREATION.md)
still govern admission. General SELECT persistence, alias/model changes through
existing-query replacement, QueryDef rename/delete, broader indexed/system storage
and other Jet/ACE write profiles remain unfinished. The separate local-table field/
wildcard serializer is not active. UI Design/read-query coverage does not widen
either native writer.

Initialized system storage is an explicit native-plan effect carried by the
immutable reopen witness. Runtime verifies its actual identity/TDEF and complete
prior table membership against the same full AFTER. UI publication permits only
that exact local system-table addition and the actual new query; the receipt
remains query-only Created, with no user schema or row-count effect.

## Verification status

The [GUI lifecycle record](../../tests/results/queries/querydef-gui-lifecycle-2026-11-13.json)
keeps source/input pins, failed attempts, explicit ignores and reused scopes. At
`93ccf3e6e`, UI/Desktop strict checks pass, the full UI suite passes 797 tests with
no ignores, and the full Desktop suite passes 466 tests with two explicit ignores.
The latter includes actual Created-to-ordinary-TCQE saving of the edited value 101.

Reused checks at `9b63442ce` include strict lint across eleven packages, 2,237
native passes with 263 explicit ignores, six Runtime creation passes, 80 MCP
passes and the renderer build. Earlier unchanged-owner scopes at `5f3b34840` retain
343 Access passes with one ignore, 497 Runtime passes with one ignore, the feature
matrix and Rustdoc. These are explicitly reused checks at their captured sources,
not a single current full suite or a combined passing total across versions.
The latest UI/Desktop checks qualify their current presentation owners; earlier
UI results remain superseded evidence.

These are local release/headless persistence and regression checks for the stated
constant-query profile. The local-table SELECT serializer remains unactivated;
this record does not qualify it, broader query authoring, other native profiles
or Windows/Access behavior.

The retained [visual-only record](../../tests/results/queries/query-save-surfaces-2026-11-13.json)
covers English/German light/dark Name, saved Design and close presentations on
macOS at scale 1 using headless normal-host rendering. It remains separate from
functional checks and native Windows/Access behavior. No corresponding native
Save-name or close capture is available; these dialogs remain visual adaptations.

The runtime owns SQL qualification and native save authority. Shared UI owns
exact text, revisions, parked sessions and save presentation; the desktop binds
these through one consuming review/save/reopen lifecycle. Hosts explicitly opt
in to editing. Unavailable editing never starts a save: retained SQL remains
read-only, while a source without qualified SQL shows the unavailable placeholder.

## Current native boundary

The complete-SQL route reviews existing projection and WHERE expressions in a
qualified local SELECT against actual saved fragments from the same immutable
ACE 16 image. Multiple saved definitions and MSysQueries DATA pages can coexist.
Each edit binds the target's signed native identity, attribute, order and exact
original text. Existing fragment membership/order, aliases, parameters, sources,
flags and all other model fields must remain unchanged. Only changed expression
ASTs are serialized; unchanged stored spelling remains byte-exact. The complete
candidate is reconstructed and must equal the full proposed SQL model.

The native source remains a closed, unencoded ACE 16 code 5/subversion-zero
profile with complete catalog/index/allocation/forwarded-root ownership checks.
Edited Memo bodies must be inline; protected unedited external bodies may remain
unchanged. Qualified replacement can grow or shrink through the existing native
record-placement owner while preserving the file length, logical index roots
and every byte outside its proved scope. Unsupported topology, parameters,
compiled/cache/property payloads and unqualified ownership still refuse.

`ReviewQuerySql` returns a distinct `ReviewedQuerySql` capability for
`SaveWorker::start_query_sql`. Its ordered native request uses `TCQE0002` and the
existing fixed-image checked V1 journal. Request version and journal version are
separate. The low-level literal review/`ReviewedQueryExpression`/TCQE0001 route
keeps its original same-length, one-query, five-fragment language exactly; neither
decoder retries a failed request as the other version. The
[native QueryDef reference](../../tests/reports/queries/NATIVE_QUERYDEF_FITTING_SAVE.md) preserves
that earlier profile and the independent SDK observations.

Canonical SQL visibility remains independent of writer eligibility. A readable
SELECT/UNION/crosstab can remain read-only. Even semantic no-op review performs
the complete native qualification; AST equality alone grants no write authority.
The host neither slices SQL nor encodes native fragments.

The existing-expression replacement route does not create a QueryDef, implement
Save As or initialize MSysQueries; those belong to the separate creation service
above. Neither route qualifies broader query shapes or establishes Access 97–2019
write parity. Independent SDK observations, TableCraft byte preservation and
Microsoft Access behavior remain separate evidence. Native Microsoft Access
save/reopen, DateUpdate/events and Windows execution comparisons remain unrun.
The saved-SQL status/close surfaces have no retained native Access interaction
capture; their presentation is provisional.

## Exact review and save ownership

A UI request binds document revision, native-model object identity/name, exact
original/proposed strings and a monotonically increasing text revision. Missing
original SQL is not fabricated. Runtime review returns a separate native
capability and complete `QueryExpressionReopen` witness. Review holds no exclusive
file lease, creates no journal and executes no saved query.

Cancellation, a stale document revision and read-only source eligibility are
checked before parsing submitted SQL. Native object identity and complete source
qualification are still checked from the actual captured image.

The desktop retains that exact request, runtime review ID and witness through
one nonreplaceable save owner. UI prepares and budgets its complete status
before the actor starts, then binds the actual accepted save ID without another
allocation. Stale replies/actions cannot attach to another object, text revision
or accepted lifecycle. A stale close-Save cannot relabel an existing save as a
request to close its tab.

Changed native saves use the existing adapter, checked original intent and
journal. Every apply rechecks the complete original under its retained exclusive
target. Query completion facts are separate from row counts and table/settings
changes. Failed retained owners offer only their actual recover/release choices;
no edit is silently retried. Status remains query-owned even when an outcome
message cannot fit: an allocation-free fallback retains outcome kind and resource
controls.

Native acknowledgement alone never marks visible SQL saved. A known replacement
requires the SAME accepted full AFTER witness to reopen successfully first.
`SaveOutcome::query_expression_reopen_facts` rejects conflicting receipt scopes;
current original/rollback facts override old replacement history. Original,
no-write and failed outcomes release presentation review while preserving dirty
text. They never acquire unchanged-review authority.

Formatting-only Save is a separate qualified semantic no-op. An actually
unchanged native review drops its write capability and performs only its fresh
BEFORE-equal-AFTER witness read. Only that successful check can resolve the exact
submitted spelling. It invents neither a native actor ID nor a commit receipt.
Saved-query execution is always a later explicit Run action.

## Fresh revision and parked drafts

After the witness read, the shared object owner atomically validates the old
accepted request and fresh catalog, then publishes the new document revision.
It moves retained original/proposed SQL buffers for all active and parked saved
queries, preserving other dirty text, revisions and errors. Only the saved
target's baseline resolves. Object IDs/order/active selection, scratch SQL and
source-neutral parameter/design/totals preferences remain associated with their
owners. Old results, pending callbacks and source-bound native handles are
invalidated and reloaded separately; the source-neutral filter definitions use
the existing checked new-revision reapply route.

A layout-only preference restore cannot substitute for this owned transfer.
Canonical SQL inspection is not repeated over an already retained same-revision
SQL draft, so a successful Save keeps the submitted spelling. Publication/memory
refusal leaves all original sessions and text intact.

## Cancellation, failure and document transitions

Cancel requests stop only the matching unlocked review, actual save actor or
read-only reopen. Cancellation never implies rollback or clears a retained
resource owner. A failed witness read keeps every old session, baseline and
proposed string. Explicit Retry reopens through the same witness without replaying
the edit; Return to draft preserves dirty text and the historical known native
save warning plus any actual cleanup notice. An unchanged-review failure reports
only its read error. Returning to the old dirty view never claims the current
file is original or identical to the expected AFTER.

Open, Create, tab close and window close respect active and parked dirty saved
SQL through exact Save/Discard/Cancel decisions. Deferred host transitions remain
owned until their drafts are resolved, and stale decisions cannot clear newer
text. Close-Save resumes only after acknowledged/witnessed publication. A failed
or original result cancels the deferred transition while preserving the draft.
Unrelated row/schema/settings writes stay blocked while saved SQL is dirty;
the query-only save uses the explicit full-witness owned-transfer route.

## Connected expression expansion evidence

The [coordinated workflow record](../../tests/results/queries/full-sql-expression-workflows-2026-11-08.json)
retains 94 selected passing checks across the Access bridge/staging, runtime,
shared UI and headless desktop checkpoints. Eleven full-SQL runtime cases cover
multi-projection save/reopen/explicit Run, projection+WHERE review, inert syntax,
source invalidation and complete-witness tamper refusal. Five unchanged
literal-service tests retain the separate TCQE0001 boundary. The record preserves
initial compile/fixture/assertion failures separately from corrected checks.
Strict affected-library/desktop lint passes; no native Access or Windows runtime
execution is claimed. General QueryDef CRUD remains unfinished.

## Earlier bounded-route evidence

The [initial connected headless batch](../../tests/results/queries/query-save-rebind-2026-11-08.json)
passes 93 tests, including seven real desktop query-save cases and nine owned
transfer cases within the broader object filter. The UI interaction case drives
the editor, Command-S, the actual query-owned report modal, acknowledgement,
witness reopen and explicit Run returning 43. Formatting-only Save retains the
original file and creates no journal. Parked system-table/scratch sessions and
stale original/reopen cases are separate checks.

The subsequent stale-close guard and historical warning/cleanup correction add
two focused cases and strengthen the failed-AFTER presentation check. Their
focused rerun now passes all 37 selected checks: 21 object/transfer cases,
nine query-save controller cases and seven saved-query reading cases. Across
the retained service, integration and final gates, 109 workflow checks and four
localization checks pass;
repeated executions and the renamed opening case are not counted as new coverage.
A substituted
terminal cleanup envelope tests presentation only; it does not establish a real
retained-artifact filesystem outcome.
