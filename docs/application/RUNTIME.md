# Application sessions and tasks

`tablecraft-runtime` owns the desktop's working native read session, command
execution and background worker. It has no egui/eframe dependency. The desktop
owns native windows and dialogs, forwards commands and moves owned results into
the shared UI's presentation types. Engine interpretation stays in ACE, Access
adapts it, and query evaluation stays in the query libraries.

## Fresh blank database creation

The optional `ace16-create` feature exposes `CreationRequest::blank`,
`CreationWorker` and owned terminal creation reports independently of
`offline-edit`. The desktop explicitly enables both features. ACE owns bounded
serialization; Access adapts it through `CreationDestination::capture`,
`prepare_blank` and `publish`. Runtime supplies one cumulative native allowance
and cooperative cancellation across those three phases. The result is an
unencrypted ACE 16 database with zero user tables, columns and rows, serialized
from its own system catalog and maps rather than a copied template.

Creation uses a separate nonreplaceable worker. `start` consumes an explicit
destination request, refuses another operation while work or an unacknowledged
report remains, and allocates its report owner before filesystem publication.
Repeated `poll` calls retain the same terminal report; `acknowledge` requires its
exact operation ID. A rejected start returns the unconsumed request. Dropping the
worker is not publication cancellation or final-file cleanup; hosts retain it
until terminal acknowledgement.

Reports distinguish capture, construction and publication failures while owning
the original typed cause. Publication facts preserve not-published (including
possible staging residue), uncertain visibility, published-but-unconfirmed and
published-with-cleanup-failure outcomes. A cancellation request does not imply
rollback or absence of a file. Neither a failure nor cancellation authorizes an
automatic retry or deletion of a visible/uncertain destination.

Required publication uses the Unix implementation on macOS and Linux and a
separate local-NTFS write-through handle-relative rename on Windows. Receipts
expose `guarantee()`; Windows does not claim a separately synced parent directory.
Unsupported locations refuse without a weaker fallback. Actual Windows filesystem
execution remains unverified. Existing files are never overwritten. See [fresh construction and publication](../access/writes/FRESH_DATABASE_CREATION.md).

`Command::OpenCreated` accepts only a successful retained report. It constructs
the expected deterministic blank image and compares it with the bytes obtained
by normal bounded file capture before `Source` qualification. A changed capture
is refused; there is no ordinary path-open fallback. This content check does not
attest physical file identity or revalidate the publication's parent identity.
The read worker retains its usual generation checks. Failure to open the created
file does not erase the host's successful publication receipt.

Five feature-local runtime creation checks cover retained operation/report
ownership, publication and guarded reopen behavior. The corresponding shared UI
and desktop checks remain headless. Independent Jackcess reading qualifies only
the observed generated structure; native Access open/save remains unqualified.
The desktop starts a shared source-free `Table1` draft only after this successful
guarded open. That draft has no runtime native table/row authority; its schema
requires the separate reviewed persistence operation below. Record entry remains
unimplemented. See [database creation](../ui/DATABASE_CREATION.md).

## Reviewed first-table schema save

With both `offline-edit` and `ace16-create`, the macOS/Linux/Windows host path can submit
`Command::ReviewFirstTable` with a document revision and immutable
`TableDraftSnapshot`. Runtime maps the initial AutoNumber primary ID and explicit
Short Text (255 UTF-16 units)/Long Integer declarations into the native definition.
It captures the standard original image and qualifies the complete canonical
blank-to-first-table plan. This admits neither arbitrary imported-file DDL nor
record creation, field alteration or silent type inference.

The consuming `ReviewedFirstTable` retains the original content, exact owned
native definition and the document's invalidation token. It also exposes an
immutable `FirstTableReopen` witness containing the proposed complete AFTER bytes,
table identity and original document/draft revisions. Those bytes are read-only
reopening evidence; the save path never accepts them as a replacement payload.
The witness is issued during review and is not an acknowledged save receipt.

`SaveWorker::start_first_table` uses the same nonreplaceable retained-resource
owner as row saves. It checks the invalidation token, opens one exclusive target,
requires all reviewed original bytes and reconstructs the native schema operation
against that descriptor. The adapter's distinct TCNT original-schema intent and
V2 journal perform checked same-file resizing, recovery, retirement and cleanup.
Cancellation/invalidation participates in bounded native work; neither grants an
implicit rollback, journal deletion or retry. Windows uses the separate
local-NTFS flat-sidecar owner rather than the Unix directory-bundle cleanup
contract; actual Windows execution remains unrun.

Successful receipts carry `SaveSchemaChange::FirstTableCreated { column_count }`
and `None` for both row counts. Receipts separately carry optional
`SaveRetirementGuarantee` and `SaveCleanupDisposition` facts mapped from the actual
adapter receipt. Unix removal and Windows retained artifacts remain distinct;
neither is inferred from a native committed outcome. Desktop reports a retained
private recovery journal in each locale and keeps that fact in revision-bound
status after normal refresh or the first-table automatic acknowledgment/reopen.
A different document cannot inherit it. The macOS runtime suite passes 125 tests and the desktop suite passes 81 tests
in the current batch, including receipt/status handling. The new retained-journal
preview passes its focused eight-locale/two-theme test. Windows x64 and ARM64
desktop/service cross-checks pass; actual filesystem execution remains unrun.
Recovery of a committed replacement retains the
schema fact; an original-image rollback does not. Terminal phase, known outcome,
resource state and available recovery/release actions remain independent of
whether the display classification is cancellation, resource exhaustion or a
native refusal. Typed classification preserves those facts through transparent
native/intent/journal error layers.

After an acknowledged matching schema receipt, the desktop explicitly submits
`Command::OpenFirstTableSaved`. Normal bounded capture must equal the complete
witness content before native qualification; mismatch refuses without an ordinary
open fallback. This establishes fresh content, not inode identity or continued
parent-namespace identity. The host checks the request's document/draft identities
and replaces the draft only after successful reopening; a reopen failure does not
erase the acknowledged save. Pending user transitions remain bound to their
original request while save/recovery/reopen completes.

The Access adapter's five focused first-table tests pass locally on macOS.
The current runtime and desktop suites above exercise connected integration and
typed-classifier behavior; historical creation results remain separate evidence. Native Access comparison, Windows
same-file filesystem-journal behavior and power-loss qualification remain unrun.
See [the adapter contract](../access/writes/FIRST_TABLE_PERSISTENCE.md).

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
| ReviewFirstTable | Source-bound consuming initial-schema review; no lease is held while presenting it |
| OpenFirstTableSaved | Explicit expected-content capture after an acknowledged schema save; no mutation authority from the witness |
| Open | Inert catalog, local-storage facts, format and captured absolute source path; a failed replacement clears the old session |
| Page | One buffered table page with typed scalar/stored-field cells |
| Describe | Field declarations, reader facts and inert field-property observations, without user-record scans |
| InspectCollection | Complete stored children of one revision-bound native parent cell |
| Query | Explicit read-query result over the selected source; no implicit provider opens |
| SortTable | Revision-bound table/field identifiers composed into an owned SELECT/ORDER BY model; complete buffered read-only result under the same query budgets |
| FindNext | Revision-bound native table/field search returning an exact source ordinal, field name and consumed UTF-16 span; no rows are filtered or reordered |
| FilterSelection | Capture an exact native selected value, build complete typed filter membership and return its first checked native page |
| FilterPage | Read a visible page from an issued native view handle, preserving original source ordinals; toggling retains criteria and sort |
| SortView | Order the complete active native membership using strict stored keys, returning a checked editable source map |
| ClearFilter | Drop predicates while retaining the native sort and source binding |
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
Each 100-row display page requests at most 101 native rows under the same work
budget and cancellation checks. A validated extra row enables Next without being
published; otherwise checked EOF disables Next, including when the final display
page has exactly 100 rows. Failure while reading the lookahead refuses the whole
buffered page. The extra parent and its selected stored fields/collection work
consume the existing native allowances; no limits are raised. If a reader has
not reached checked EOF, Next remains available. Declared row counts do not
substitute for checked traversal.

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

Literal clipboard text can seed the same replacement input with `is_null=false`;
review still uses the target field's invariant/ISO syntax and normal native
admission. It does not infer typed NULL from `NULL` or an empty string, recover
clipboard source precision, or parse regional display formatting. An exact match
with this target's initial DATE/Single/Double edit text retains the target's exact
stored carrier, including residual DATE bits and NaN payloads. This unchanged-edit
rule is target preservation, not a lossless typed clipboard transfer. The 1 MiB
UTF-8 input ceiling is checked before that preservation rule; cumulative work and
publication limits still apply.

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

Normal filesystem editing has macOS/Linux and Windows host paths; other platforms
return an explicit unsupported result. Windows activation uses local NTFS,
write-through retirement and intentionally retained private recovery artifacts.
It does not claim Unix parent-directory barriers or durable artifact deletion;
actual Windows filesystem execution remains unrun. This reviewed-cell workflow is a bounded
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
The metadata draft supplies original field names, input carrier descriptions,
reader nullability and `InsertField::auto_number`. The marker comes from the
actual inert native `ColumnFeatures` metadata, aligned against the insertion
schema by complete field count and exact names. It is not inferred from an ID
name, scalar type, last-issued counter or displayed value, and does not select
Increment versus Random/replication-ID generation semantics. Metadata inspection
does not promise that a complete row will pass native admission.

Ordinary fields retain explicit Omitted or Value intent, with NULL separate from
literal text; their UI text survives choice changes. AutoNumber-marked fields
remain omitted in this reviewed application-input contract. Hosts can show them
as read-only `(New)` rather than offer supplied/NULL input. Review independently
rejects both supplied values and explicit NULL for these fields, even if a caller
bypasses the renderer. The native planner still owns generator-mode, counter,
constraint and index admission. This application-input policy does not change or
ban the separate explicit SQL/engine append contracts, which retain their own
native qualification.

Review rechecks the actual native AutoNumber markers, requires their omission,
then parses ordinary supplied text against actual declared types and runs the
normal presence-bearing native INSERT planner over the captured image. It returns exact
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
and packaged sources are read-only here. This adds no general create-table, multirow, external-source or inline
Access autosave guarantee. See [normal INSERT](../access/writes/NORMAL_ACE_INSERT.md).

New connected runtime scenarios save the initial schema into the actual canonical
blank, reopen it through its content witness, then use this existing insertion
review/save path for ID-only, ID-plus-Long-Integer and ID-plus-Short-Text tables.
They check original omission through review, native-generated first ID, stored
companion values, one-row rather than schema completion, journal cleanup and
ordinary reopening. A separate refusal case keeps the file unchanged when app
input supplies an ID or NULL. All six focused runtime insertion tests passed
locally on macOS on 7 November 2026, including these two new connected cases and
the three initial schema variants. This is not a full-runtime regression run,
Windows filesystem-journal observation or native Access comparison.

The initial-table route still authors no companion defaults or AllowZeroLength
policy. Increment insertion currently requires explicit companion values or NULL;
an ordinary companion left omitted is not silently replaced by an invented
value. Empty Short Text remains subject to the native explicit-AllowZeroLength
coverage gate, and this runtime metadata addition does not weaken it. Direct
editing of the unsaved draft's first record, inference from entered data and
inline Access autosave remain separate work.


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
Query output, system tables and absent records cannot issue the UI intent.
Native filtered/sorted views retain checked original ordinals instead of treating
a displayed position as a source ordinal.
The existing plain-source and offline save limits apply. This route
adds neither bulk deletion nor record-history Undo; a later undo would be a fresh
validated native operation. See [normal DELETE](../access/writes/NORMAL_ACE_DELETE.md).

## Native table filtering

Selection filtering reads the original scalar from the immutable native source,
using the exact revision/table/field/source ordinal supplied by the host. The UI
never reconstructs the predicate from displayed currency, date, checkbox, NULL
marker or rounded text. The initial operations are Equal and NotEqual for an
entire selected value; selected stored NULL becomes IsNull or IsNotNull. Empty
text remains a typed text value, distinct from NULL. Partial text selection,
Filter by Form and advanced expressions are not supplied by this service.
Calculated/complex/unqualified selected fields and Binary predicates refuse
explicitly.

`DescribeFieldFilter` inspects an exact revision/table/column without requiring
any user record, including for empty tables. Its returned `InputHint`, nullable
reader declaration, eligible operators and optional ANSI pattern mode are
informational. `ApplyFieldFilter` independently checks that source, declaration,
operator, operands and optional opaque base handle again; metadata is neither an
edit permission nor a retained operation capability. An explicit enabled/off
witness accompanies the exact base filter ID.

Entered numeric/date common filters support Equal/NotEqual, ordered comparisons,
inclusive Between/NotBetween, and IsNull/IsNotNull. Between accepts reversed
bounds, retaining the shared eager comparison service; it does not normalize
NULL into an ordinary value. Ordinary negated comparisons also exclude NULL.
Typed date operands use the existing invariant ISO entry syntax, rather than
reparsing regional display text. Boolean, Guid, Binary, calculated and complex
fields are outside this common-filter profile. Existing qualified Boolean
selection filtering remains separate.

Numeric bounds are query operands rather than values to be written into a
field. Fixed numeric fields accept exact Decimal operands, including fractional
and out-of-storage-range thresholds; Currency bounds are not rounded through
CCur and integer bounds are not narrowed to their storage widths. The shared
exact parser reuses the conversion grammar but refuses discarded nonzero digits
outside the 96-bit coefficient/scale-at-most-28 envelope. Existing CDec rounding
behavior is unchanged. Single/Double operands additionally require their parsed
carrier's shortest decimal spelling to represent the same exact decimal input:
`0.1` and `0.1000` are accepted, whereas Single `1.00000001` refuses precision
loss rather than silently becoming `1`. This canonical decimal round-trip guard
is an explicit engineering profile, not mathematical IEEE exactness or native
Access numeric type-promotion qualification. Localized numeric spelling and
unsupported precision refuse while retaining the previous view.

Entered Text Equal/NotEqual use native LIKE/NOT LIKE patterns, unlike literal
selected-value equality. Begins/Ends/Contains and their negations add the
mode-appropriate outer wildcards without escaping or rewriting the user's
pattern. The successfully inspected absence of the database query-mode setting
selects ANSI-89; known ANSI-92 selects its grammar; unknown mode refuses.
Qualified native weighted comparison providers are required, with no binary or
system-locale fallback. Pattern negation excludes stored NULL; present-empty
Text remains distinct. Reapplication resolves the fresh source's pattern mode
and field policy instead of retaining an old compiled matcher. Arbitrary SQL
filter expressions and dynamic date presets are not supplied.

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
retained membership map contains ascending, unique original ordinals, rather
than copied table rows or filtered display indices. A separate checked display
permutation records the requested native sort. Filter pages separately carry visible offset,
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
issued definition; Clear All drops its predicates while retaining the sort. A
selection made while filtering is off starts fresh predicates but retains the
existing sort. The selection target explicitly witnesses that activation mode.

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
handle/definition storage. Entered operands have a 64 KiB UTF-8 ceiling each and
exact operator arity (zero, one or two); parsing uses the existing cumulative
8 MiB/100,000-step scalar presentation allowance. Compiled native patterns share
one separate 64 MiB/ten-million-step pool across preparation and all source
rows. Pattern, scalar and comparison work never resets for each candidate.
A smaller work allowance may refuse an input below the byte ceiling. Selection
capture and its cursor scan share one native allowance (512 MiB, 100,000 rows, five million steps), with the separate existing
16 MiB/one-million-step native comparison pool. The first page and subsequent
FilterPage calls use a separately named native band-read allowance with the same
bounds, plus the normal cumulative 8 MiB/100,000-step presentation pool. No pool
resets per batch, source row, field or selected output row. These are explicit
phase limits; a valid but larger source can return a resource error.

Saving/reopening changes the document witness, so old filter handles must refuse.
`FilterHandle.definition()` instead returns opaque cloneable preferences with
only table, typed predicates, sort and declared type constraints; it carries no row map
or old source rights. Explicit ReapplyFilter validates the new revision and exact
field declarations, resolves fresh native comparisons, scans again through EOF
and returns a new handle/map/page. It preserves enabled/off state and clamps the
requested visible offset to existing membership (zero when empty). The desktop
uses this only after a matching acknowledged save and fresh Open; it never
reuses old filtered ordinals as new-source identities.

Microsoft's [filtering workflow](https://support.microsoft.com/en-us/access/apply-a-filter-to-view-select-records-in-an-access-database)
and [record-editing guidance](https://support.microsoft.com/en-us/access/ways-to-add-edit-and-delete-records)
provide the selection, replacement, toggle and record-position target.
Microsoft's [Between contract](https://support.microsoft.com/en-us/access/between-and-operator)
and [Access/ANSI comparison](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql)
specify inclusive bounds, NULL behavior and Access's reversed-bound distinction. Local
acceptance covers 205 source rows across filtered pages, same-field replacement,
different-field AND, typed NULL/empty and signed zero, eager nonfinite errors,
failed/canceled candidates preserving old handles, cross-worker witness rejection,
EOF failure and explicit fresh-source reapplication. These headless checks do not
establish native Access filtering or cross-platform GUI parity.

## Reviewed replacement of a Find occurrence

With offline editing enabled, `ReplaceFindMatch` accepts the exact successful
Find request and found record/field/UTF-16 span, literal replacement text and an
optional issued filter handle/mode. It checks the current document revision,
source witness, visible membership, scope, native field/control policy and exact
match boundaries against the immutable raw source. It revalidates that occurrence
in place; it does not run Find Next again, advance to a different match or replay
an old search after reopening. Input masks, lookup/non-textbox controls and
unqualified fields refuse rather than bypassing normal native admission.

Current supported carriers are raw Text and unformatted Byte/Int16/Int32/Int64.
Text replacement splices precisely the checked occurrence into the full original
text without normalizing it. UTF-16 positions are mapped to valid UTF-8 character
boundaries; a span inside a surrogate pair refuses. Find patterns retain their
native mode/case policy; replacement wildcards are literal characters, without
capture expansion. Integer replacement operates on its exact canonical integer
spelling and passes the existing invariant typed parser. Formatted numeric/date
replacement requires a qualified regional conversion contract and currently
refuses; displayed currency/date text is never reparsed as invariant input.

An empty replacement deletes only the matched substring. The resulting edit
input is explicitly non-NULL Text, including a present-empty value when the
entire text is removed. Stored NULL matches refuse and require an explicit typed
edit; native automatic Whole Field blank-to-NULL versus empty behavior remains
unqualified. Existing review controls expose the NULL distinction rather than
silently equating NULL, empty Text and the literal word `NULL`.

Replacement returns an ordinary `EditDraft` containing the runtime-computed full
value. It neither reviews nor saves automatically. Explicit Review uses the
existing native planner, field constraints, exact original image and consuming
`ReviewedEdit`; Apply uses the same retained SaveWorker outcomes/recovery and
fresh leased source checks. This reviewed workflow is a deliberate current
interaction difference from Access's immediate Replace button. Refused candidates
leave the earlier valid edit draft intact.

Match preparation and ordinary BeginEdit are separate bounded snapshot passes,
with no offline lease held while the user considers the result. The first pass
shares the ordinary native allowance across metadata and normal-editor snapshot,
a 16 MiB/one-million-step native comparison pool, a cumulative
64 MiB/ten-million-step match pool and the normal 8 MiB presentation pool. The
second BeginEdit retains its existing native/presentation allowances. Literal
replacement and final edit input have the existing 1 MiB input ceiling; normal
Review and Apply retain their own existing bounds. Pools do not reset per row
or substring. Replace All is not supplied by this single-occurrence operation:
a complete batch requires its own one-image native admission and consuming
batch save capability, rather than a loop of independent cell commits.

Microsoft's [Find/Replace contract](https://support.microsoft.com/en-us/access/use-the-find-and-replace-dialog-box-to-change-data)
documents partial-field search, literal replacement characters, blank replacement
and lookup restrictions. Current wildcard match-span selection and native
case-sensitive parity retain the Find qualification limits; successful headless
review is not a Windows Access replacement comparison.

## Visual SELECT design services

`tablecraft-builders::query` owns the working headless visual draft, native QBE
criteria composition, typed owned-AST construction and conservative SQL import.
It depends on the owned SQL/schema contracts, without GUI, runtime, engine or
file dependencies. Runtime commands only qualify current-document metadata and
compose this service. Shared UI imports that actual model rather than defining
a second draft or interpolating source identifiers into SQL.

`DescribeQueryDesign` returns exact native table/field names and independently
qualified scalar-reader facts. It reads metadata without fetching user records;
unknown/complex declarations remain visible without acquiring execution rights.
`BuildQueryDesign` and `ImportQueryDesign` check the document revision, capture
fresh source descriptors under one cumulative native allowance and return both
the owned draft/canonical SQL and those descriptors. Build and import do not
execute SQL, open stored linked targets or modify saved queries or the database.
A successful design is syntax and source metadata, not a promise that every
expression or source is executable by the current query profile.

The initial editor permits eight source instances with optional explicit aliases,
seven explicit field-equality join edges, 32 grid columns and eight alternative
criteria rows. Fields selected from metadata become typed source-ID/column
references; native identifiers never become SQL prefix/suffix fragments. The
owned formatter supplies canonical quoting. Projection, optional alias, Show and
left-to-right sort choices remain distinct. Hidden columns can still filter or
sort. Grid cells on the same criteria row combine with AND; nonempty rows combine
with OR, while blank rows add no unconditional branch. Contextual field criteria
support comparison shorthand, compound comparisons, Between, In, Like, Is Null
and quoted or simple bare Text values. Computed operands require explicit
predicate syntax or scalar operands; bare wildcard patterns require explicit
Like. Join orientation and Inner/Left/Right kinds are explicit, never guessed
from names. Cyclic and compound-edge join graphs remain outside this initial
profile.

SQL import accepts only a SELECT representable by this model. It rejects
parameters, UNION, TOP/DISTINCT, GROUP/HAVING/aggregates, external/derived sources,
subqueries, unsupported join conditions and other omitted clauses. A canonical
round-trip proof preserves the represented projection, aliases, join tree,
criteria grouping/order and sorting; only transparent relation parentheses are
normalized. Unsupported criteria decompositions and changes to evaluation order
refuse, leaving the caller's existing SQL and draft intact. SQL execution remains
available independently and is always an explicit host action.

Combined editable text is limited to 64 KiB. The public parser additionally
retains its byte/token/depth ceilings; generated SQL must pass those same bounds.
Metadata has separate eight-table/1,024-fields-per-table/1 MiB name envelopes and
rejects duplicate or invalid names. Hosts use the owning text-accounting and
metadata validators for admission. Runtime retains ordinary native/presentation
pools and checks cancellation before/after bounded pure construction. No mutable
runtime design registry, native lock while editing or implicit preview execution
is introduced.

## Native datasheet totals

`DescribeTotals` returns native headings and their eligible aggregate operations
without reading user records. This metadata is informational. `Totals` accepts an
exact document revision, table, optional issued view ID, enabled/off mode and
opaque base handle. Another worker's handle refuses even if its public revision
counter matches. At most 32 distinct fields may be requested; omission means no
aggregate, rather than an invented count or default operation.

The service reads one projected native cursor through checked EOF, using the
active filter membership or the complete source when filtering is off. It checks
source cardinality and every retained selected ordinal before publication,
including for a request with no aggregate fields. Sorting does not turn the
operation into visible-page arithmetic. A failed, canceled or superseded request
publishes no partial total and leaves prior view handles valid.

One typed SELECT aggregate model is composed directly, without SQL interpolation,
and evaluated through the existing query-owned kernels. Count ignores NULL but
counts present-empty Text; there is no Count(*) or distinct-count substitution.
Numbers support Count, Sum, Average, Minimum, Maximum, sample Standard Deviation
and Variance. Legacy Date/Time supports Count, Average, Minimum and Maximum.
Date/Time Extended currently supports Count, Minimum and Maximum; its mean is
not qualified by the existing kernel. Ordinary Text/Boolean/Binary/Guid fields
are Count-only. Complex, calculated and unqualified physical layouts have no
advertised operations. Empty or all-NULL inputs yield zero for Count and NULL
for the other aggregates; sample statistics with fewer than two values yield
NULL. These are current implemented boundaries, not a complete ACE 16 claim.

Derived cells have no native row or edit identity. Native date/Currency Format
lineage remains explicit. Legacy date means deliberately project the query's
Automation-serial mean into a validated derived date display. Currency means
retain the query's Double result in a separate `CurrencyAverage` display fact;
ICU formats its shortest decimal spelling directly to the selected currency's
minor units. There is no intermediate CCur/four-place write conversion or double
rounding. Raw retains that Double's shortest spelling. Stored Currency and its
Sum/Minimum/Maximum retain the original exact scaled-integer representation.

Each operation keeps the existing native allowance (512 MiB, 100,000 rows and
five million steps) separate from retained projected input (16 MiB/one million
steps), query execution (16 MiB, 10,000 rows and one million steps), native
comparisons (16 MiB/one million steps) and normal presentation output
(8 MiB/100,000 steps). The projection is retained once for all requested fields,
not separately for each column. Copies, names, vector growth and traversal are
charged; none of these pools resets per batch or field. Native admission of
100,000 rows does not override the smaller query-row ceiling: larger complete
inputs explicitly refuse rather than returning a prefix total.

Microsoft's [datasheet Totals-row contract](https://support.microsoft.com/en-us/access/display-column-totals-in-a-datasheet-using-a-totals-row)
provides the operation/type target. Headless acceptance does not establish native
Access formatting, numeric promotion or platform interaction parity.

## Native table sorting

Following Microsoft's [native sorting workflow](https://support.microsoft.com/en-us/access/sort-records-on-text-numeric-or-date-values),
SortView replaces the selected single native sort key or removes it with an
explicit absent SortSpec. It works with a plain table, a filtered view or retained
off-state criteria. It never constructs an SQL query or replaces native identity
with a read-only query result. ClearFilter removes criteria while retaining sort;
removing sort preserves criteria. Zero-predicate views are canonically unfiltered.
Multiple sort keys, custom list ordering and sorting complex/OLE/Binary/GUID fields
are not implemented by this initial native view service.

A complete projected cursor binds the selected field's exact declaration, counts
all source records and reaches checked EOF before publishing an order. Only
records admitted by the active filter have their sort keys validated, copied and
compared. Excluded nonfinite values therefore cannot fail an active filtered sort;
explicitly toggling the filter off can fail when those values enter the order.
Such failure retains the prior published handle and page. Strict relational keys
provide NULL-first ascending order, exact numeric widening and native weighted
text comparison without parsing displayed text. Descending reverses comparison;
equal keys keep their original source order. Text containing numeric characters
remains Text. Native text parity still requires the recorded Windows qualification.

Each immutable handle caches the requested mode's display permutation and checked
source-ordinal ranks. Its ascending predicate membership remains separate for
complete native traversal. Requesting the opposite mode computes a fresh bounded
order and page before returning a replacement handle with the same semantic
revision/table/view identifier; the previous handle is never mutated. A rank
request for an uncached mode refuses instead of guessing physical order. Native
Find uses the current display rank while retaining physical source ordinals.
After a successful save, explicit definition reapplication rebinds the selected
key and predicates against the newly opened source and issues fresh row rights.

Sorting has its own cumulative 64 MiB/ten-million-step key-work pool and one
ordinary native cursor allowance, separate from membership construction and page
publication. Retained permutations/ranks are charged to the 8 MiB view allowance;
text comparisons use the existing independent native comparison pool. Full cursor
work, admitted key copies, fallible stable merge comparisons and rank construction
remain cancellable. A mode change uses a fresh explicit sorting phase, never a
per-record budget reset. Local checks cover complete 105-record descending paging,
stable equal keys, filtering/sort/clear interactions, exclusion of hidden NaN,
failed off-toggle preservation, fresh-source reapplication and native stored
format/control/calculation provenance. They do not establish native Access GUI or
cross-platform parity.

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

FindFiltered additionally requires the exact issued native view and an already
computed display mode. It checks that admission before scanning, including an
empty membership or absent pattern. Filtered searches exclude nonmembers while
still completing the physical cursor; sorted searches choose occurrences by
checked display rank, original field order and UTF-16 span, preserving the
original source ordinal for the matching page and later guarded editing.

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
