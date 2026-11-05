# Application sessions and tasks

`tablecraft-runtime` owns the desktop's working native read session, command
execution and background worker. It has no egui/eframe dependency. The desktop
owns native windows and dialogs, forwards commands and moves owned results into
the shared UI's presentation types. Engine interpretation stays in ACE, Access
adapts it, and query evaluation stays in the query libraries.

## Session and task ownership

Open captures an immutable image of a stable/offline file up to 64 MiB, closing
the original handle after the bounded copy. It checks length before and after;
this does not establish a transactional snapshot against same-size concurrent
writes. Close other writers first. Package selection and native password/page
decoding use the existing Access/ACE services. Passwords are consumed into
zeroizing storage, including on failures. Opening a source never executes its
active content or follows stored external paths.

The worker owns the session and one replaceable pending request and response.
New commands supersede older work through generation cancellation; publication
and host acceptance both reject stale generations. Pending replacement and old
result clearing share the mailbox lock; backend work runs outside it. Dropping a
worker requests cancellation without joining on the GUI thread. A blocking OS
file-open/read call must return before a cooperative cancellation check can run.

| Command | Owned response and scope |
| --- | --- |
| Open | Inert catalog, local-storage facts, format and captured absolute source path; a failed replacement clears the old session |
| Page | One buffered table page with typed scalar/stored-field cells |
| Describe | Field declarations, reader facts and inert field-property observations, without user-record scans |
| InspectCollection | Complete stored children of one revision-bound native parent cell |
| Query | Explicit read-query result over the selected source; no implicit provider opens |
| SortTable | Revision-bound table/field identifiers composed into an owned SELECT/ORDER BY model; complete buffered read-only result under the same query budgets |
| FindNext | Revision-bound native table/field search returning an exact source ordinal, field name and consumed UTF-16 span; no rows are filtered or reordered |
| FilterSelection | Capture an exact native selected value, build complete typed filter membership and return its first checked native page |
| FilterPage | Read a visible page from an issued filter handle, preserving original source ordinals; toggling does not discard criteria |
| ReapplyFilter | Explicitly rebind typed filter preferences to a newly opened revision, issuing a fresh map and source handle |

The desktop follows a successful Open with Page for the first non-system local
table, unless restoring an acknowledged save. This is a host workflow, not a
catalog enumeration side effect. Sorting returns no native row handles or source
revision: reordered query positions cannot be used as native edit ordinals.

Describe's reader NULL policy stays separate from its stored Required/default/
validation observations. Unknown field declarations remain visible. Metadata does
not establish whole-table readability or permission to write.

## Read-only Table Design properties

Describe now preflights the actual catalog property carrier, inspects the target's
complete native property map and binds its exact object ID to the table schema.
Those native operations share one cumulative allowance. The same preflight and
binding owner serves ordinary Page reads; it does not open user records or execute
property text.

Each field retains observations for Required, AllowZeroLength, DefaultValue,
ValidationRule, ValidationText, Description and Caption. A missing carrier is
`Uninspected`; a completed field map without a key is `Absent`. These states are
distinct from an explicit false Boolean or empty text. Unknown flags/carriers
are `Unsupported`, rather than fabricated defaults. Qualified Boolean properties
use their one-byte native accessor; a wrong width fails the operation. Text and
Memo declarations with the native DDL flag remain original, inert text.

Field-level scope and original field names govern association. An index property
map with the same name is not a substitute. Descriptor envelopes, each selected
property vector and all copied text consume the descriptor publication budget;
iteration, comparisons and cancellation remain checked. Partial descriptors are
never published after failure.

The stored declarations explain the read-only property sheet. They do not
evaluate defaults or validation, enforce Required, confer edit authority or change
reader nullability. Microsoft's [Required workflow](https://support.microsoft.com/en-us/access/require-a-value-in-a-field),
[default-value guidance](https://support.microsoft.com/en-us/access/set-default-values-for-fields-or-controls)
and [validation guidance](https://support.microsoft.com/en-us/access/restrict-data-input-by-using-validation-rules)
describe the corresponding application behaviors; those behaviors remain separate
from inert inspection. See the [native property contract](../access/schema/NATIVE_PROPERTIES.md)
for type, framing and preservation boundaries.

## Table cells and stored collections

Page uses `NativeBrowser::browse_page` once. Metadata selects ordinary scalar
paging or the existing qualified stored-field profile. Failed reads are not
retried as another profile. Unknown, corrupt or unqualified columns refuse the
operation rather than disappearing from the page or becoming empty collections.
Checked page EOF controls the Next action; page size and declared row count do
not substitute for it.

Each cell owns its display text and explicit reader facts. Shared regional
options, validated `ScalarPresentation` facts and the renderer-neutral formatter
belong to `tablecraft-display`, imported directly by runtime, UI and desktop.
Runtime qualifies native facts; formatting does not authorize native edits or
change invariant edit inputs. Display has no GUI or database dependency.

The cell facts remain explicit:

- Scalar cells carry independent NULL and stored-calculation flags. Literal text
  `NULL` does not acquire NULL semantics from its spelling. Cached NULL results
  retain both flags; no formula evaluates and freshness remains unknown.
- Exact Boolean values carry `Some(true/false)` separately from display text;
  text/numeric lookalikes and expression-only states never acquire that fact.
  `DisplayControl` independently distinguishes uninspected metadata, a verified
  absent declaration, the exact native signed control code and an unsupported
  declaration. It does not supply a guessed field default.
- Multivalue, attachment and stored-history cells carry assigned-key presence and
  a complete member count. An absent key differs from an assigned empty list.
  Duplicate and NULL elements count. An attachment whose FileData is NULL remains
  a member. All typed items must validate before the count is published.
- Current Memo text remains a separate scalar from stored version membership.
  Display does not select a latest version or infer timestamps from ordering.

Parent-page cells contain collection summaries. Explicit child inspection returns
a separate complete scalar table; attachment payload bytes are not included. The underlying stored-field
reader is eager: it validates complete parent/child snapshots and retains encoded
attachments before returning a visible page. This is bounded complete-read
behavior, not lazy expansion or constant memory. No attachment decoder, URL
resolver, file export or external application is invoked by browsing.

The shared UI renders values before compact annotations, retains provenance in
tooltips/accessibility context and copies the complete display text without its
badge. Display strings are not lossless exports, edit carriers or a general native
field-formatting engine. Query Empty/Missing/error states remain explicit display
states; they are not silently converted to stored NULL.

Native Page preflights the actual `MSysObjects` schema for its property carrier,
then inspects present property storage under the same cumulative native budget
that is moved into the browser. A successfully inspected catalog without that
carrier yields `Uninspected`; actual metadata/read/framing/cancellation/limit
errors propagate. Property and page object identities must match. Field names
join only column-scoped groups, and exact DDL-flagged Int16 declarations retain
their original codes. Unknown flags, carriers or widths are `Unsupported` rather
than absent. Presentation work charges the name lookups and cell enrichment.
Query results and explicit child contents remain `Uninspected`: aliases or a
parent collection's settings do not establish native field provenance.

Shared UI shows a read-only checkbox only for a non-NULL typed Boolean with an
explicit native checkbox control (106). Text-box, combo-box and unknown controls
retain the stored display text; absent/uninspected controls remain explicit.
Full-value Copy and cached-result provenance are unchanged. See the
[desktop presentation record](../ui/DESKTOP.md).

## Native Boolean field Format

Native pages now apply the qualified predefined `Yes/No`, `True/False` and
`On/Off` Format names to non-NULL typed Boolean cells with an explicit TextBox
DisplayControl (109). The current English presentation policy supplies `Yes`/
`No`, `True`/`False` and `On`/`Off`; neither database LCID nor ambient OS locale
chooses those labels. Other locale policies remain unimplemented.

The original table identity and column-scoped property group must already be
bound by the page reader. Format requires its native DDL flag and Text carrier.
Missing, unknown or unsupported declarations retain the original display value;
actual property read/framing/cancellation/resource errors still fail the page.
CheckBox (106) retains its read-only checkbox and previous full-value copy text;
combo-box, absent and unknown controls do not gain text-box behavior. NULL stays
NULL, and cached-result provenance remains separate from these display labels.
No formatted string is reparsed into a stored value or replacement input.

The shared UI copies the full resulting display text. Native property bytes and
stored values remain unchanged; queries and explicit collection contents do not
inherit a parent field format. Custom format sections, numeric/date formats and
DecimalPlaces remain outside this Format slice; Caption headers are described below. See Microsoft's
[Yes/No formatting contract](https://support.microsoft.com/en-us/access/format-a-yes-no-data-type).
This is a documented presentation policy with synthetic native-property tests,
not a new Microsoft Access differential or locale-compatibility claim.

## Native Caption headers

Result columns carry an exact `name` separately from an optional native
`caption`. Native page captions use the already object-bound Column property
map, explicit DDL flag and native Text or Memo carrier, under the same publication
budget.
Missing or unsupported declarations leave the caption absent; native metadata
read/framing errors still fail the operation. Explicit empty captions remain
`Some("")` metadata. The current header presentation falls back to the true name
for empty captions until native empty-caption behavior is qualified.

Captions are presentation only. Edit, collection and record navigation targets
retain the original field name, including when captions duplicate another field
name or each other. Query aliases and synthetic collection child headings carry
no inherited native caption. Both column envelopes and allocated name/caption
text count against the output allowance.

## Explicit collection inspection

Each executed Open attempt advances a checked worker-local document revision,
including a failed Open. It identifies the captured immutable session, not an
on-disk file version, durable bookmark or write capability. Opened responses expose
it; native table pages carry `source_revision: Some(revision)`. Query results and
child tables carry None, so they cannot be mistaken for native parent ordinals.

InspectCollection supplies that revision, the original table/column names and
global parent ordinal. The service rejects stale revisions before native reads,
then reuses one-parent `browse_page` and its typed collection items. Missing
parents, missing columns, scalar cells and corrupt children are explicit errors.
This rereads complete qualified collection snapshots under the existing budgets;
it is not a lazy child reader. The requested global offset is applied once.

Multivalue contents retain order, duplicates and NULL elements. Attachment rows
retain the signed stored child ID and nullable name, type, URL, timestamp, flags
and encoded-byte count; absent FileData differs from present encoded bytes of
length zero. No payload is decoded, extracted or opened, and metadata URLs remain
plain text. History retains stored order and signed child identities, with nullable
Memo/time values separate from current Memo text. Timestamp columns distinguish
rounded civil display from exact original DATE bits; an unavailable civil display
is labelled explicitly while the raw value remains present.

The shared UI admits only typed collection cells in the current native page.
An explicit context-menu action or plain Enter opens a loading/result/error modal.
The modal owns keyboard input and keeps the underlying page/query/schema intact.
Replies must match its still-open task ID and full source/parent target; closing,
replacing the source or changing the underlying view prevents late reopening.
Closing an old dialog cannot cancel a newer request. Native decoding and revision
checks stay in runtime; desktop moves DTOs and routes results.

## Reviewed offline cell edits

The optional `offline-edit` feature supplies BeginEdit/ReviewEdit on the read
worker and a separate `SaveWorker`. Desktop enables it. Drafts use exact document
revision/table/column/ordinal targets; they retain no offline lease while the
user types. Runtime converts replacement text to typed values and invokes normal
native planning. SQL is never generated from the input. The shared UI's review
is a display DTO; only the runtime can issue its opaque consuming `ReviewedEdit`.

The capture is shared through Bytes between the reader and review precondition.
Apply acquires the existing exclusive offline session and compares the complete
physical image before resolving fresh snapshot-local handles and reconstructing
the native plan. Encrypted/package sources remain read-only for this route.
Backend validation, required programs, write profiles and resource controls still
apply. See [offline session ownership](../access/writes/OFFLINE_WRITE_SESSIONS.md).

Save work is nonreplaceable: a newer read generation cannot discard its terminal
outcome. Reports distinguish known commit/rollback facts, retained resources,
recovery eligibility and acknowledged release. Cancellation requests stopping;
it is not a rollback receipt. Recovery/release consume the retained owner through
explicit operations. Busy/stale starts return the unconsumed capability.
Known committed edits are never replayed as recovery attempts.

The host blocks source replacement and window closure while an accepted save
remains unacknowledged. A successful acknowledged save reopens its source and
refreshes only the table/page bound to that exact reopen generation. Failed or
released uncertain outcomes cannot leave the old snapshot masquerading as current.
Incomplete setup offers explicit resource release while preserving any existing
or partial journal entries for diagnosis. Uncertain release still denies retry;
the current UI has no general restart/recovery browser for that state. Reports
show the current operation outcome separately from the last completed receipt,
including its available row counts. A release failure cannot hide a completed
commit, and a previous receipt cannot be promoted to the current attempt.

The default reserved `.FILENAME.tablecraft-journal` sibling blocks reopening
while present, including a symlink, without following or deleting it. This only
detects that active reserved path. Journal retirement renames bundles, so a crash
after rename can leave an undiscovered retired sibling. Other caller-selected
bundles and path aliases are not a general journal-discovery mechanism. Keep the
explicit offline and outstanding-journal preconditions; this is not Access
multiuser locking or complete crash-restart recovery.

Normal filesystem editing currently works only on macOS/Linux. Other platforms
return an explicit unsupported result. This reviewed-cell workflow is a bounded
offline workflow, not Access autosave, record navigation or query-result
updateability parity.

## Localized input instructions

EditDraft and InsertField publish an inert InputHint tag derived from the actual
schema type. The desktop mirrors that tag exhaustively; the shared UI selects
Fluent instructions in the current language. Original field names remain literal
metadata. These hints neither choose a parser nor grant native write eligibility.

Single, Double, Currency and Decimal instructions explicitly require the invariant
`.` decimal separator. Regional display choices, symbols and translated prose
never become implicit input syntax. Parsing and unchanged DATE/float preservation
continue to use actual schema/native values, independently of the hint tag.

## Reviewed offline row insertion

The same `offline-edit` worker now accepts BeginInsert/ReviewInsert. InsertTarget
contains a document revision and table name, with no invented existing row handle.
The metadata draft supplies original field names, input carrier descriptions and
reader nullability; it does not promise that a complete row will pass native
admission. Each field retains explicit Omitted or Value intent, with NULL separate
from literal text. The UI keeps text while toggling those choices.

Review parses supplied text against actual declared types and runs the normal
presence-bearing native INSERT planner over the captured image. It returns exact
reviewed inputs, a one-row count and an opaque consuming ReviewedInsert capability.
No native file lease is held while the user types or considers the review.
Formatting preferences never alter these inputs or substitute displayed values.

SaveWorker accepts either a cell update or insertion under one nonreplaceable
owner. Insertion apply captures the same exclusively retained descriptor used for
its exact original-image comparison, then reconstructs the normal plan with the
retained omissions/values. It uses the existing journal, receipt, cancellation,
recovery, release and acknowledgement lifecycle. Source drift refuses before a
journal is created; any retained session still requires explicit release.

The desktop's matching save owner routes outcomes back to the insertion dialog.
An acknowledged successful insertion reopens the actual source and refreshes the
same table from its first page. It does not invent an inserted native row identity
or promise that a later-page row is visible on that first page.

Current bounds are one row, at most 256 fields, 1 MiB per supplied input and 8 MiB
of total input text. The normal planner decides actual format/schema/index and
required-program eligibility. Qualified constant defaults, NULL fallback and
Increment AutoNumber use ordinary admission. This service supplies no clock or
random provider, so defaults needing those capabilities remain refused. Encrypted
and packaged sources are read-only here; native saves are currently macOS/Linux
only. This adds no general create-table, multirow, external-source or inline
Access autosave guarantee. See [normal INSERT](../access/writes/NORMAL_ACE_INSERT.md).

## Reviewed offline row deletion

ReviewDelete captures a document revision, table and original row ordinal. The
worker binds that ordinal against its complete native snapshot, preserves the
qualified field captions/control/format facts for confirmation, and runs the
normal single-row DELETE planner. The returned display review cannot authorize a
save: only the separate opaque ReviewedDelete capability can enter SaveWorker.
No native lease is held while the confirmation is visible.

Apply uses the same save owner and durable journal as cell editing and insertion.
It compares the complete reviewed image against the fresh exclusively retained
capture, binds a fresh snapshot-local handle, and reconstructs the normal plan.
Original byte equality, required programs, native constraints, relationships,
indexes and preservation gates remain mandatory. Source drift refuses before
journal creation and retains any actual resource owner for explicit release.
Cancellation does not manufacture rollback or discard a completed receipt.

After successful acknowledgement the host reopens the actual source. The next
view ordinal comes from the complete reviewed membership: the next remaining
record, the previous last record, or zero when empty. It does not use a declared
row count or a browser page prefix, and it is not a native mutation handle.
Query/sorted output, system tables and absent records cannot issue the UI intent.
The existing plain-source, offline and macOS/Linux save limits apply. This route
adds neither bulk deletion nor record-history Undo; a later undo would be a fresh
validated native operation. See [normal DELETE](../access/writes/NORMAL_ACE_DELETE.md).

## Native table filtering

Selection filtering reads the original scalar from the immutable native source,
using the exact revision/table/field/source ordinal supplied by the host. The UI
never reconstructs the predicate from displayed currency, date, checkbox, NULL
marker or rounded text. The initial operations are Equal and NotEqual for an
entire selected value; selected stored NULL becomes IsNull or IsNotNull. Empty
text remains a typed text value, distinct from NULL. Partial text selection,
range menus, common-filter editors, Filter by Form and advanced expressions are
not supplied by this initial service. Calculated/complex/unqualified selected
fields and Binary predicates refuse explicitly.

At most sixteen distinct native fields are retained. Applying a new criterion to
the same exact field replaces that field's previous criterion; different fields
combine with eager AND semantics. Every criterion evaluates for every traversed
source row, even after an earlier false/unknown result. Native field comparison
policies and the existing scalar SQL comparison service govern equality; stored
representation equality is not SQL equality. In particular, signed numeric zero
representations can compare equal. Unknown text policies are never replaced by
binary comparison; NULL predicates do not activate unused collation callbacks.

The membership scan uses one projected native cursor with cumulative work and
cancellation controls and completes checked EOF before returning anything. A
failure or later count mismatch cannot publish an earlier matching prefix. The
retained map contains ordered, unique original ordinals, rather than copied table
rows or filtered display indices. Filter pages separately carry visible offset,
original ordinals, matched/source counts and `has_more`; disabled filtering shows
the source membership while retaining the criterion-match count and definition.
No filtered position can acquire native identity through `offset + displayIndex`.

An opaque cloneable `FilterHandle` shares immutable typed criteria, exact type
constraints and membership through Arc. A private per-Document Arc witness binds
its row rights independently of the public revision counter; another worker with
the same revision number cannot reuse the handle. There is no hidden active
filter or registry that evicts old published handles. A candidate command takes
an explicitly matching base handle or explicitly starts a new definition. The
host keeps its old handle/page until a complete matching replacement is accepted;
canceled, failed and superseded candidates drop normally. Toggle off/on keeps the
issued definition; Clear All drops the host's filter state. A selection made
while filtering is off starts a new definition.

Each visible page reads one original contiguous band covering at most 100 mapped
rows, then presents only those selected indices. The existing normal browse
profile retains native Caption, Format, display controls, stored-calculation
provenance and complete stored collection summaries. Unselected band rows are not
converted into UI cells. The native band allocation is still fully charged;
sparse large bands can exhaust the ordinary allowance rather than silently omit
fields or reset the budget for each selected row. Existing complete collection
snapshot limits remain unchanged. An empty filtered page publishes qualified
metadata headings without fetching an unrelated source row.

Map construction has an 8 MiB retained-work allowance and one million steps,
including typed criterion/name copies, schema-name comparisons, map capacity and
handle/definition storage. Selection capture and its cursor scan share one native
allowance (512 MiB, 100,000 rows, five million steps), with the separate existing
16 MiB/one-million-step native comparison pool. The first page and subsequent
FilterPage calls use a separately named native band-read allowance with the same
bounds, plus the normal cumulative 8 MiB/100,000-step presentation pool. No pool
resets per batch, source row, field or selected output row. These are explicit
phase limits; a valid but larger source can return a resource error.

Saving/reopening changes the document witness, so old filter handles must refuse.
`FilterHandle.definition()` instead returns opaque cloneable preferences with
only table, typed predicates and declared type constraints; it carries no row map
or old source rights. Explicit ReapplyFilter validates the new revision and exact
field declarations, resolves fresh native comparisons, scans again through EOF
and returns a new handle/map/page. It preserves enabled/off state and clamps the
requested visible offset to existing membership (zero when empty). The desktop
uses this only after a matching acknowledged save and fresh Open; it never
reuses old filtered ordinals as new-source identities.

Microsoft's [filtering workflow](https://support.microsoft.com/en-us/access/apply-a-filter-to-view-select-records-in-an-access-database)
and [record-editing guidance](https://support.microsoft.com/en-us/access/ways-to-add-edit-and-delete-records)
provide the selection, replacement, toggle and record-position target. Local
acceptance covers 205 source rows across filtered pages, same-field replacement,
different-field AND, typed NULL/empty and signed zero, eager nonfinite errors,
failed/canceled candidates preserving old handles, cross-worker witness rejection,
EOF failure and explicit fresh-source reapplication. These headless checks do not
establish native Access filtering or cross-platform GUI parity.

## Native table Find

`FindNext` captures the exact document revision, local table, field/table scope,
pattern, match mode, direction, previous source position and regional display
options. It searches the immutable native session through one `NativeBrowser`
projected cursor, including records beyond the currently loaded 100-row page.
It does not construct SQL, filter rows or interpret a query result's position as
an original native record. All scoped declarations, format/control capabilities
and comparison descriptors qualify before user-record traversal. Failure never
silently drops an unsupported field.

The scan reads complete batches through checked EOF before publishing either a
match or a negative result. A declared-count mismatch, later malformed record,
cancellation, missing source anchor or exhausted allowance returns an error,
including when an earlier prefix contained a match. Only candidate positions are
retained; the service does not materialize a complete table. Up chooses a prior
occurrence, Down stops at the end, and All searches forward then wraps. Continued
searches consume further occurrences within the same cell. Positions contain the
original row ordinal and exact field name plus a consumed UTF-16 range; valid
surrogate pairs are not bisected, including zero-width continuation.

Pattern compilation happens once per command. Any/Whole/Start matching uses the
shared bounded wildcard frontier and stored ANSI-89/ANSI-92 query setting.
Successfully inspected absence selects the documented ANSI-89 default; an unknown
setting refuses. Match-case off uses the exact qualified native field/default
weight provider. Match-case on uses a separately admitted provisional exact
UTF-16 literal/class policy for supported General/French descriptors. It does not
reuse insensitive LIKE or claim Windows linguistic case-sensitive parity. The
selected-start rule prefers the shortest nonempty consumed span, falling back to
an empty span only when necessary. Wildcard selection greediness and native
substring highlighting remain unqualified; the desktop currently selects the
matching cell rather than claiming Access-identical substring highlighting.

Formatted date/currency searches use the same renderer-neutral
`tablecraft-display::Engine`, captured `Options` and qualified native facts as the
UI. Native masks, lookup controls and unsupported custom formats refuse; they
never become approximate raw searches. Unformatted date searches require a real
regional typed-input policy and currently refuse. Unformatted Single, Double,
Decimal and Currency likewise refuse instead of guessing locale-dependent decimal
syntax. A table-wide unformatted search therefore refuses if its scope includes
such declarations, even when another field might match. Integer/text searches
retain the current explicit carrier presentation. Documented Find exclusions for
Yes/No and OLE Object are explicit; other unsupported fields refuse. Whole-field,
unformatted `Null`/`Is Null` criteria search typed NULL, and `""` searches actual
zero-length text; neither searches the UI's synthetic NULL badge.

Complex fields are outside this scalar Find profile. Their internal foreign keys
are never treated as searchable displayed values. A current collection cell does
not enable Find; table-wide search refuses a complex scoped field, while ordinary
field-specific searches retain their existing qualification.

The command has a 64 KiB UTF-8 pattern ceiling. Metadata and cursor reads share
one native allowance (512 MiB, 100,000 rows, five million steps), with a separate
cumulative native-comparison pool (16 MiB and one million steps). Pattern/span
work has a separate cumulative 64 MiB/ten-million-step pool; presentation uses
its existing 8 MiB/100,000-step allowance, including the once-captured request
and every retained candidate name. None resets per batch or field. Resource
failure is an error, not a completed negative search. The host retrieves the
matching page with a second explicit command and its own operation allowance;
publication requires the same source revision, ordinal and original field.

Microsoft's [Find dialog contract](https://support.microsoft.com/en-us/access/use-the-find-and-replace-dialog-box-to-change-data),
[FindRecord behavior](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/findrecord-macro-action)
and [wildcard reference](https://support.microsoft.com/en-us/access/access-wildcard-character-reference)
provide the behavioral target. Local tests establish full-cursor traversal,
repeated occurrences, wrap/reverse navigation, typed NULL, captured EUR display,
mandatory EOF validation and failure boundaries. These are headless implementation
checks, not native Access or cross-platform GUI qualification.

## Native saved queries and declared parameters

Open now publishes table and saved-query declarations from one inert
`NativeBrowser::inventory` capture. The native adapter retains all catalog kinds
and original identities; the current UI presents Tables and Queries. Opening
reconstructs no query body and executes no query or user code. Both catalog-to-DTO
passes charge/check every entry, including kinds omitted from these two lists.

`InspectSavedQuery` supplies a worker-local document revision, full scoped object
ID and original name. It reconstructs the supported owned read syntax through
`NativeSource::read_saved_query` and returns declared parameter specifications.
Revision is checked before native loading, then both ID and exact name are
checked against the reconstructed definition. Inspection does not bind sources,
scan user records or establish execution readiness. Unsupported action,
pass-through or malformed definitions remain visible catalog declarations and
return explicit errors when inspected.

`RunSavedQuery` is a separate request with literal named inputs. The query-owned
`resolve_parameter_type` supplies carrier and length/precision constraints, so
prompts do not duplicate SQL type lowering. Declared scalar kinds stay fixed;
VALUE requires an explicit concrete kind. NULL is an independent flag. Missing,
duplicate, unknown or mismatched inputs fail; unknown column errors do not invent
parameter prompts. Text is literal data, with no SQL interpolation or VBA call.
The same shared scalar conversion service supports editing and these inputs;
editing retains its exact unchanged DATE/float-carrier rule separately.

Input admission is at most 256 parameters and 1 MiB of cumulative UTF-8 names,
kinds and text, including retained text beside NULL. The UI checks before cloning
an action; runtime checks independently. Metadata/input conversion has its own
bounded scratch allowance, separate from query execution and final display
publication. A single native source and cumulative native allowance cover saved
reconstruction and subsequent execution/schema discovery. Query and display
limits remain the existing controls below.

Execution uses the existing SELECT/UNION/crosstab profile and schema-discovery
service. No external source registry, stored connection, provider, clock, random
state or VBA authority is added. Required unsupported policies remain explicit
errors. Text comparison uses the stored policies described below; the host does
not guess unsupported policies, a locale or LIKE mode. General Access query
updateability remains separate work.

Saved results are complete buffered query results with no native source revision,
zero offset and no progressive suffix. The shared UI rejects a native/partial or
wrong-offset response instead of removing its provenance. Success/error replies
must still match the selected query, request and exact submitted inputs. Dirty
inputs clear their old result; a failed rerun with unchanged inputs retains and
labels the previous successful output. Saved and ad-hoc SQL text/results retain
separate selection slots. None of these results grants native edit, record or
collection-target capability.

## Native query text policies

Both ad-hoc SQL and explicit saved-query runs configure the Access adapter's
deferred database-default resolver before schema discovery. Available Text fields
retain their exact native policy identities; literal/default expressions use the
actual stored database descriptor. The existing General legacy/2010 and
comparison-only French legacy providers remain provisional native-weight models,
not Windows Access SQL qualification.

An unsupported default does not block numeric execution or plain projection.
Supported field policies remain usable independently. An operation that requires
an unsupported field/default policy fails explicitly, with no ordinal, system
locale or another-family fallback. Corrupt metadata, cancellation and resource
errors still fail during configuration. The same source/native allowance covers
saved reconstruction and execution; inspection alone configures no text policy.

One separate cumulative comparison pool allows 16 MiB and 1,000,000 steps per
query, shared by all resolved families and nested execution. It is independent
of native I/O, query execution and display quotas. The host supplies no omitted
VBA module Option Compare setting; explicit binary and supported database string
comparison modes retain their own contracts.

LIKE grammar remains a separate setting. Explicit query runs read the native
database property map through its checked Databases/MSysDb catalog roles, under
the same cumulative native allowance. Qualified `ANSI Query Mode` LONG/DDL values
0 and 1 select ANSI-89 and ANSI-92 respectively. The actual example supplies 0;
the 1 mapping has reference-supported synthetic checks, not a native Access
execution comparison. Missing or unsupported observations leave LIKE unavailable
without blocking numeric execution. Malformed metadata, role, cancellation and
resource failures propagate rather than becoming a guessed default.

Microsoft documents the
[per-database ANSI-89/ANSI-92 choice](https://support.microsoft.com/en-us/access/access-wildcard-character-reference).
This does not implement every SQL-mode difference. Database sort order,
application version and operating-system locale do not establish the option.
See [database properties](../access/schema/NATIVE_PROPERTIES.md) and
[native comparison admission](../access/queries/NATIVE_QUERIES.md).

## Limits and verification

Each native operation currently receives 512 MiB of charged bytes, 100,000 rows
and 5,000,000 work steps. Visible pages request 100 records. Stored-field pages
also retain the existing default complete parent/child snapshot ceiling of
16,384 rows; a small visible page does not bypass that ceiling. Ordinary scalar
pages keep their paged profile instead of taking that complete snapshot.

Query execution independently uses 16 MiB, 10,000 rows and 1,000,000 steps, with a
64 KiB SQL input ceiling. Display publication has an 8 MiB budget and 100,000
steps, including typed row/cell envelopes and collection iteration. These are
consumer controls, not format limits or an exact process-heap cap. Partial
pages/counts are not published after failure. Generation checks remain necessary
even when cancellation is requested.

Feature-local runtime tests use the public Worker and existing testkit inputs.
They cover immutable reads, failed opens/password retry, real queries, metadata,
stored caches, collection presence/counts, corrupt child refusal and mailbox
ordering/cancellation. Shared UI tests exercise copy, focus, provenance and
virtualization; these do not qualify native screen readers or Access behavior.
Native observations and current platform limits are recorded in the
[desktop guide](../ui/DESKTOP.md). Payload-free diagnostics remain governed by
[the tracing contract](../development/TRACING.md).
