# Application sessions and tasks

`tablecraft-runtime` owns the desktop's working native read session, command
execution and background worker. It has no egui/eframe dependency. The desktop
owns native windows and dialogs, forwards commands and moves owned results into
the shared UI's presentation types. Engine interpretation stays in ACE, Access
adapts it, and query evaluation stays in the query libraries.

## Captured read-query resource options

`Command::Query`, `RunQuery` and `RunSavedQuery` capture a validated, immutable
`QueryRunOptions` value for that invocation. Inspection remains inert and does not
execute a query or consume its eventual run allowance. Issued SQL and saved-query
targets retain their existing document/revision checks before supplied inputs are
processed. Action-query reviews and write/save allowances are unchanged.

The options separate snapshot fetch granularity from cumulative native, query,
comparison and presentation resource pools. Defaults impose no extra product
quota: counters use their representation bounds, and individual snapshots use
unsigned 32-bit row ordinals. Fetch batches remain 256 rows. Materialization grows
only for actual source rows; it never reserves the maximum row count up front.
Parameter conversion and final result publication share the run's presentation
pool. Buffered failures publish no partial result.

Hosts can select explicit positive quotas through `QueryPoolLimits::new` and
`QueryRunOptions::with_limits`. Native/query pools carry row allowances;
comparison/presentation pools do not. Requests are not silently clamped to former
4 GiB/10-million-row or smaller query maxima. Budgets remain cumulative accounting,
not a resident-memory guarantee. Cancellation, source/format validation, checked
arithmetic and fallible allocation remain required. Batch limits do not become
database row-count limits, and there is no automatic retry after exhaustion.

Native initialization, classified native-source limits during execution and
presentation failures expose `Error::ReadQueryLimit` with their actual pool and
original `LimitError`. Backend-neutral `QueryError::SourceFailure` also preserves
payload-free I/O kinds, invalid/truncated data, busy resources, invalid requests/
cursor state, constraints, source structural ceilings and allocation failures.
Runtime retains those non-limit categories through `Error::Query`; it does not
retry or replace a source. Unclassified providers can still return the legacy
opaque source failure. Comparison/evaluator limits without pool provenance keep
their original diagnosis rather than a guessed pool. Rejected configurations
perform no I/O.

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
requires the separate reviewed persistence operation below. Its raw pending record
uses the distinct reviewed insertion continuation below. See [database creation](../ui/DATABASE_CREATION.md).

## Reviewed table-creation schema save

The shared UI can retain one raw pending record alongside an unsaved schema.
Its handoff is presentation-owned: original field identities, schema/record
revisions, omitted/NULL/value intent and hidden raw text remain separate from
native values and handles. Runtime continues to receive the existing schema
review command, then a fresh BeginInsert/ReviewInsert after guarded reopening.
These are two distinct saves; schema commitment never implies a stored record.
The UI seeds only an exact current native insertion draft, and the existing
consuming row review/save still determines native admission. Desktop routes
those services and retains requested transitions across their acknowledgments;
it parses no record input and constructs no generated defaults.

Opening a document reports separate `table_creation_available` and
`table_design_available` facts alongside its exact
`document_revision`. With creation and offline editing enabled on a supported
host, runtime captures an internal `First` or `Added` profile from the immutable
source. `First` requires the complete canonical blank. Otherwise `Added` checks
the actual append-table source profile through the engine's bounded qualification.
The caller receives advisory booleans, not a profile selector or native action
capability. Unlike the canonical-blank comparison, append availability captures a
native image and checks existing user rows and primary membership under the same
read operation's budgets. It neither builds a proposed table nor runs active
content, opens a draft, holds a write lease or writes the file.

The original generated-only append lineage admits one through eighteen existing
local user tables: `7 + 3n` pages, `7 + n` catalog records and `20 + 3n`
inline map records for `n` tables. Each original table's single DATA page and
checked Int32 primary root retain their original physical roles. Repeated sources
require ordinary in-row scalar storage without extended flags; unsupported
Memo/OLE LVAL, complex and extended Date/Time schemas refuse. Original opaque
property data and unused gaps remain bytes to preserve. Other object/layout,
indirect-map and ownership profiles remain unavailable.

The largest admitted append produces nineteen tables and 64 pages, at the current
base-zero inline/global FREE-map bitmap boundary. This is an implementation
profile, not an Access format limit; further bitmap/catalog growth remains work.
A readable source at that boundary reports unavailable for another addition.
The mixed-ledger continuation also admits qualified keyless and single manual
Int32/Int64/Currency-primary tables. A keyless target requires two new pages;
a keyed/generated target requires three. The native owner derives all existing
roles from the actual source and independently reports both capacities. Thus a
62-page source can enable fresh Design while leaving Create > Table disabled.
This is still a bounded base-zero append profile, not arbitrary imported DDL.
Proposed names/declarations and append capacity are checked later against the
actual request. An unavailable advisory fact does not classify the database as
corrupt. Structural/profile refusals report false; cancellation, resource
exhaustion, I/O and construction failures remain errors. Read-only sources report
false. The revision-bound fact enables only explicit Create → Table; ordinary and
recovered Open do not themselves create a draft.

With both `offline-edit` and `ace16-create`, the macOS/Linux/Windows host path can
submit `Command::ReviewTableCreation` with a document revision and immutable
`TableDraftSnapshot`. Runtime maps the initial AutoNumber primary ID and explicit
Short Text (1–255 UTF-16 units), Number with an explicit Field Size, Large Integer,
Currency, Yes/No and legacy Date/Time declarations into the native definition.
The immutable snapshot also carries effective capacity, exact optional
Required/AllowZeroLength policies and an explicit default mode. New ordinary
authoring fields start with Required=false; Short Text starts with
AllowZeroLength=true, following the documented UI defaults. Explicit omission
remains available and is never reconstructed as false or true. No system clock,
display format or inferred Number subtype changes these declarations.

Review uses the captured internal profile to qualify exactly the first-table or
append-table operation against the current standard original image. It does not
retry one operation as another after refusal. Document invalidation revokes the
review; Save detects changed file content through complete original-image
comparison. The advisory Open result cannot authorize Save. This admits neither
arbitrary imported-file DDL nor record creation, field alteration or
silent type inference. After each acknowledged Save, a fresh Open qualifies the actual resulting source
before another Create → Table can become available. Retained prior availability
or table count alone cannot enable it.

Canonical blank and qualified populated-source Open publish the separate
revision-bound `table_design_available` fact. It enables an empty manual Design
draft whenever a supported target can fit; it does not depend on the generated
Create > Table fact. Manual snapshots without a generated field lower through
`FirstTableDefinition::capture_manual`, retaining actual optional key position
and all explicit policies/defaults/Descriptions. The captured First or Added
operation reviews the actual target declaration; no source retry or dummy primary
root is used. At keyless-only capacity the existing keyed fact disables the
no-key dialog's Yes choice before adding an ID; No remains available. A manually
selected key still receives a fresh typed capacity refusal at review. The draft,
raw record and correction path remain intact. Actual constraints and complete
BEFORE bytes are rechecked by the consuming SaveWorker and guarded reopen.

Manual first-table requests use TCNT6 and yield a real keyless table or one
supported manual numeric primary. A UI Yes decision that adds a protected
Increment ID instead produces the existing generated declaration and its
applicable TCNT1–5 intent. Authoring-view origin does not redefine native storage.
Pending first-record inputs remain separate, keyed by draft identity; only an
actual generated field requires Omitted. A manual primary does not become a
generated value or suppress the original supplied first field. The actual fresh
insertion draft still checks names, types and generation before adopting inputs.

### Initial-field property lowering

`TableDraftSnapshot` owns the exact applicable field properties and original default
input. Short Text capacity is checked in 1..=255 UTF-16 units. Hidden Text capacity
and AllowZeroLength choices survive editor type switches but do not become
properties of a non-Text snapshot. Required and explicit default choices remain
exact across ordinary type changes; the protected AutoNumber ID refuses all such
property changes. The renderer-neutral builder validates bounds, applicability and
revision changes, without importing core carriers or evaluating expressions.

Runtime lowers `DraftDefaultMode::TypeDefault` to zero in the selected numeric
carrier (Decimal at its declared scale), Boolean false, or an absent
Text/DateTime/GUID default. `Absent` retains absence;
`Null` retains an actual NULL carrier. `Literal` passes the exact raw text through
`scalar_input::parse_declared` with the selected descriptor, then native
`CreateColumn` validation.
Text is literal text, integers and Currency use the shared invariant numeric
syntax, Boolean uses lowercase true/false, and legacy DateTime uses the shared
explicit civil/bit input profile. No SQL parser, VBA evaluator, clock, field lookup
or display-formatted input participates. A text literal `Now()` is quoted text;
a DateTime formula `Now()` is invalid. Unsupported NULL carriers, irreversible
dates, incompatible values and out-of-profile declarations refuse before save
setup rather than silently changing or dropping the default.

The builder/editor's 4,096-byte UTF-8 bound retains input; it does not enlarge the
engine's separate 255-UTF-16-unit constant-expression ceiling or the declared
field capacity. Added surrounding quotes and doubled embedded quotes count toward
that expression limit. Native admission rejects an oversized expression without
truncation or a journal. Required/AllowZeroLength use exact optional Boolean
presence, independently from NULL, an empty string and an omitted record field.

Present Description selects TCNT0005/profile 5. The snapshot and native
declaration retain exact optional text, including present-empty and AutoNumber
descriptions; they do not evaluate that text. Authoring currently bounds each
description to 255 UTF-16 units, independently of aggregate native capacity.
Without Description, wider-target default definitions use TCNT0004/profile 4, including a NULL default
whose target is Byte, Int16, Single, Double, Decimal or GUID. Otherwise explicit
policies use TCNT0003/profile 3. Existing policy-free TCNT0001/profile 1 and
legacy-family default-bearing TCNT0002/profile 2 remain reconstructable under
their original rules; their decoders do not gain wider default carriers or
inferred UI properties.
The saved original intent and rederived native definition carry these properties
through the existing first-table save and recovery path. Subsequent INSERT still
uses its actual captured schema, original field presence and consuming review.
This is working initial-schema authoring, not imported-field alteration, arbitrary
default evaluation, Windows runtime evidence or native Access qualification.

### Number Field Size lowering

`DraftFieldType::Number` retains `DraftNumberSize` independently from pending row
text. Byte, Integer, LongInteger, Single, Double, Decimal and ReplicationId map to
Byte, Int16, Int32, Single, Double, Decimal and Guid carriers respectively. Large
Number remains Int64; the initial generated ID remains an Int32 Increment primary.
Number initially selects LongInteger. Decimal constraints begin at precision 18,
scale 0, bounded by precision 1..=28 and scale 0..=precision. These are explicit
authoring-profile choices, without importing Access application settings.

`scalar_input::parse_declared` uses exact numeric parsing for a Decimal descriptor,
then `rescale_exact` and the actual precision/scale validation. It permits
`1.2`→`1.20` or `1.2300`→`1.23` at scale 2, but rejects `1.234`, coefficient overflow
or a discarded nonzero digit. First-table defaults, insertion and declared edits
use that same path. The field's exact stored scale is separate from Decimal
Places and regional display formatting. A CDec rounding result is not accepted as
a substitute for the original exact input. The typed native Decimal constructor
still requires the exact declared scale and precision; it does not perform this
text-input rescaling itself.

The native default serializer accepts finite Single/Double values through a
bounded scientific formatter and exact bit reparse, including signed zero.
Producer and independent default proof preserve IEEE width. Nonfinite defaults
and generated-row float values refuse rather than acquiring numeric zero. Manual
GUID cells, NULL and qualified no-default omission are admitted ordinary values;
non-NULL GUID stored defaults remain unsupported. A Number/ReplicationId choice
sets no generation flag and supplies neither random GUIDs nor replication.

For the original source family, supplied Byte, Int16, Single, Double, Guid or
Decimal values use the closed TCNA0006/profile 6 omission-preserving intent. Legacy supplied carriers and requests
with only omitted or NULL wider fields retain TCNA0002/profile 2. The selected
intent follows the original carrier payload, not a host permission flag or the
field's display label. Independent reconstruction rereads original source metadata
and retains all existing source/index/constraint and resource admission gates.
Normal source support for these wider ordinary values is not a grant for wider
secondary keys, Random AutoNumber or arbitrary imported schemas. See
[normal Increment insertion](../access/writes/NORMAL_INCREMENTING_AUTONUMBER.md)
and the [initial Number authoring contract](../ui/DATABASE_CREATION.md#number-field-size-and-decimal-declarations).
No native Windows/Access or signed-zero comparison is established by these paths.

The consuming `ReviewedTableCreation` retains the original content, exact owned
native definition and the document's invalidation token. It also exposes an
immutable `CreatedTableReopen` witness containing the proposed complete AFTER bytes,
table identity, expected schema-change kind and original document/draft revisions.
Those bytes are read-only
reopening evidence; the save path never accepts them as a replacement payload.
The witness is issued during review and is not an acknowledged save receipt.

`SaveWorker::start_table_creation` uses the same nonreplaceable retained-resource
owner as row saves. It checks the invalidation token, opens one exclusive target,
requires all reviewed original bytes and reconstructs the native schema operation
against that descriptor using the same captured `First` or `Added` profile.
The adapter retains separate `StagedFirstTable` and `StagedAddTable` capabilities.
TCNT remains canonical-blank initialization; TCAT binds the actual populated
original. TCAT1/profile 1 stays exactly ten pages; TCAT2/profile 2 admits only
thirteen through 61 pages in three-page steps. Both retain the pinned TCNT 1–4
declaration-template grammar.
Description-bearing definitions use separate TCAT3/profile 3 and TCAT4/profile 4
actions for those same respective source partitions, each pinning TCNT5 only.
TCAT5/profile 5 separately carries mixed-ledger additions with closed TCNT1–6
templates, including manual/keyless TCNT6. Its source framing checks two-page
versus three-page target capacity before native reconstruction. The old grammar
and frame ceilings remain unchanged; an outer/profile/template
mismatch refuses instead of retrying another decoder.
The template's canonical header is not the append operation's source binding.
Action dispatch chooses its decoder once, with no retry as another schema or row
action. Each route uses checked V2 same-file resizing, complete native
reconstruction, recovery, retirement and cleanup.
Cancellation/invalidation participates in bounded native work; neither grants an
implicit rollback, journal deletion or retry. Windows uses the separate
local-NTFS flat-sidecar owner rather than the Unix directory-bundle cleanup
contract; actual Windows execution remains unrun.

Successful receipts retain the selected action as
`SaveSchemaChange::FirstTableCreated { column_count }` or
`SaveSchemaChange::TableAdded { column_count }`, with `None` for both row counts.
The new append fact does not relabel old first-table receipts or make a schema
change into a record insertion. Receipts separately carry optional
`SaveRetirementGuarantee` and `SaveCleanupDisposition` facts mapped from the actual
adapter receipt. Unix removal and Windows retained artifacts remain distinct;
neither is inferred from a native committed outcome. Desktop reports a retained
private recovery journal in each locale and keeps that fact in revision-bound
status after normal refresh or matching schema acknowledgment/reopen.
A different document cannot inherit it. The historical first-table batch recorded
125 passing macOS runtime tests and 81 desktop tests, including receipt/status
handling, plus the focused eight-locale/two-theme retained-journal preview. Its
Windows x64 and ARM64 desktop/service cross-checks were compilation evidence;
actual filesystem execution remained unrun. Those counts do not qualify the
later generic table-creation runtime changes.
Recovery of a committed replacement retains the
schema fact; an original-image rollback does not. Terminal phase, known outcome,
resource state and available recovery/release actions remain independent of
whether the display classification is cancellation, resource exhaustion or a
native refusal. Typed classification preserves those facts through transparent
native/intent/journal error layers.

After an acknowledged matching schema receipt, the desktop explicitly submits
`Command::OpenTableCreated`. Normal bounded capture must equal the complete
witness content before native qualification; mismatch refuses without an ordinary
open fallback. This establishes fresh content, not inode identity or continued
parent-namespace identity. The host checks the request's document/draft identities
and replaces the draft only after successful reopening; a reopen failure does not
erase the acknowledged save. Pending user transitions remain bound to their
original request while save/recovery/reopen completes.

The previously recorded five focused first-table adapter tests remain historical
macOS evidence. The initial AddTable checkpoint recorded seven candidate tests,
three TCAT codec tests and four native journal integration tests. The accepted
repeated-addition follow-up records nineteen engine candidate/advisory checks,
five TCAT checks, six native V2 recovery checks, nineteen offline adapter checks,
149 runtime tests and 111 desktop tests. Strict all-feature library/test lint
passes for the six affected packages. These boundary-specific counts are not
added together as a full-workspace qualification.

The connected headless desktop test creates Table1 with Score=17, invokes the
actual Create → Table command for Table2, refuses a duplicate name without
writing, then accepts the corrected name and completes reviewed same-file schema
Save. Its exact `  Ada  ` input survives reopening and is inserted only by a
separate reviewed record Save. The same interaction creates and populates Table3
and Table4, preserving unsaved SQL text and old rows. A fresh desktop host then
executes SELECTs against all four saved tables and checks their original values.
The current eighteen-source-table ceiling is checked separately; Create → Table
Design remained disabled in that recorded generated-only checkpoint. The later
mixed-ledger capability supersedes that UI restriction without changing the
checkpoint's test claims. This is headless interaction over real local files, not
native-window or Microsoft Access comparison evidence.

Windows x64 and ARM64 runtime/desktop all-target/all-feature compilation checks
pass with their dependencies. They are not native Windows execution or a complete
twenty-package Windows/CLI/SQLite build. Earlier six-architecture audit results
remain their recorded checkpoints and do not substitute for repeated-profile
native execution. Native Access comparison, Windows same-file filesystem-journal
behavior and power-loss qualification remain unrun. See the separate
[first-table](../access/writes/FIRST_TABLE_PERSISTENCE.md) and
[append-table](../access/writes/ADD_TABLE_PERSISTENCE.md) adapter contracts. The
[repeated-fixture independent-reader receipt](../../tests/results/reading/ace16-repeated-table-independent-reader-2026-11-07.json)
records five distinct images across six exported files, retaining both original
rows and empty added-table metadata; it is not insertion, recovery or native
Access save qualification.

## Read-query execution context

Explicit scratch and saved retrieval queries now share one runtime-owned context.
`Worker::new` supplies a lazy local Jiff clock; `Worker::with_query_clock` accepts
a caller-owned expression `Clock` for deterministic hosts and tests. Open,
catalog inspection, saved-query inspection and failed binding do not read it.
Clock expressions read on execution and may obtain fresh readings; unavailable
local timezone/time conversion reports a typed error without substituting UTC.
This does not supply a native write-default clock or execute stored programs.

Each explicit Run owns one sequential `RndState` initialized with its reviewed
default seed, shared by nested/named reads within that run. The next Run resets
it, matching the existing CLI policy. This is not native Access application-session
Randomize/Rnd lifecycle qualification and uses no entropy or process-global RNG.
The same context supplies unsecured `Admin` expression metadata for CurrentUser
and the existing explicit-number English reference error-message catalog. Admin
is not an OS account, MDW authentication or an authorization grant; Error's
omitted-number VBA runtime state remains unavailable. Error text does not follow
the Fluent interface language implicitly.

Database collation and ANSI LIKE mode retain their source-bound policies.
Numeric/date/week/code-page/casing and omitted-year contexts remain explicit:
display preferences and the current clock do not silently select them. Missing
required policies still refuse. Query results retain their existing buffered
budgets and no native edit identity; action SQL and DDL remain separate work.
See [the SQL workspace](../ui/SQL_WORKSPACE.md).

## Session and task ownership

Open validates the native header/file envelope (up to the native 2 GiB bound)
or the supported deployment-container geometry before capturing immutable bytes.
Capture grows fallibly from actual reads, without an invented 64 MiB opening cap
or a reservation for the maximum permitted file size. It closes the original
handle after capture and checks length before and after;
this does not establish a transactional snapshot against same-size concurrent
writes. Close other writers first. Package selection and native password/page
decoding use the existing Access/ACE services. Passwords are consumed into
zeroizing storage, including on failures. Opening a source never executes its
active content or follows stored external paths.

The standard native writer's existing complete-image qualification envelope is
separate from opening. Larger readable sources expose unavailable edit/design
capabilities successfully; they do not become failed read sessions. This read
change does not expand mutation source profiles or establish general large-file
writes. Capture still retains the actual immutable file in memory; it is not a
disk-backed snapshot implementation.

The captured plain source transfers its complete vector into the engine's
[`NativeImageSource`](../access/NATIVE_READING.md#shared-immutable-native-image-ownership).
The browser cursor, original-source witness and native review captures share that
immutable `bytes::Bytes` payload. Each review still charges its full retained
capacity and control allowance, then decodes and qualifies native metadata under
its own cumulative operation budgets. Mutable AFTER candidates remain separate.
The owner accepts only consumed vectors; arbitrary/sliced `Bytes` cannot establish
a whole-image witness. Plain-source sharing does not enable encoded/package edits
or extend a native write profile. Save still captures and checks the real
exclusive target afresh; cached bytes never authorize persistence or recovery.

The worker owns the session and one replaceable pending request and response.
New commands supersede older work through generation cancellation; publication
and host acceptance both reject stale generations. Pending replacement and old
result clearing share the mailbox lock; backend work runs outside it. Dropping a
worker requests cancellation without joining on the GUI thread. A blocking OS
file-open/read call must return before a cooperative cancellation check can run.

Read request admission returns a checked identity or `ReadRequestExhausted`.
Zero is never issued; the maximum integer is reserved solely for cancellation.
Exhaustion does not replace the preceding accepted command or consume another
identity. Cancellation still revokes the final accepted publication, and repeated
requests never wrap or reuse an identity. The desktop reports permanent exhaustion
and requires restarting TableCraft and explicitly reopening the database.

Read and save completion notifications run after publishing the owned result and
outside mailbox locks. A panicking host repaint callback is contained at that
notification boundary: the worker remains available, and a save retains its real
recovery resources until explicit release and acknowledgement. Notification
failure never retries a read or mutation or changes its recorded outcome.

An unexpected native save actor exit is different from a notification failure.
The lifetime channel becomes disconnected after native owners have unwound and
the captured source is invalidated. An accepted Apply, Recover or Release then
retains an exact-ID `Uncertain` report until acknowledgement. Any included previous
outcome or receipt is historical: it grants no current-image, no-write, recovery,
release, original-retry or replacement-reopen authority. Genuine completed results
without a lost retained owner remain genuine. A failed actor accepts no more work.

Desktop acknowledgement handles uncertainty before the normal original/AFTER
branches. Local schema/SQL drafts retain their proposed values, and the source
revision stays blocked for mutation even after dismissal or old read callbacks.
Restart TableCraft and explicitly reopen the source to establish fresh authority;
retained on-disk journals still need the normal diagnosis/recovery workflow.
This protocol does not cover process abort, a double panic during destruction,
or recovery of poisoned internal linked state.

| Command | Owned response and scope |
| --- | --- |
| ReviewTableCreation | Source-bound consuming schema review using the source-captured First/Added profile; no lease is held while presenting it |
| OpenTableCreated | Explicit expected-content capture after an acknowledged matching schema save; no mutation authority from the witness |
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
native planning. [Retained-table cell edits](../access/writes/RETAINED_CELL_UPDATES.md)
now use metadata-selected capture and a separate original-request/V3 source family,
including edits after table replacement and insertion. Native operation limits
remain explicit. SQL is never generated from the input. The shared UI's review
is a display DTO; only the runtime can issue its opaque consuming `ReviewedEdit`.

Literal clipboard text can seed the same replacement input with `is_null=false`;
review still uses the target field's invariant/ISO syntax and normal native
admission. It does not infer typed NULL from `NULL` or an empty string, recover
clipboard source precision, or parse regional display formatting. An exact match
with this target's initial DATE/Single/Double edit text retains the target's exact
stored carrier, including residual DATE bits and NaN payloads. This unchanged-edit
rule is target preservation, not a lossless typed clipboard transfer. Shared
input conversion has no separate 1 MiB UTF-8 quota; native field/type validation,
checked allocation, cancellation and explicit caller budgets still apply.

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
or partial journal entries for diagnosis. Uncertain release still denies retry. The desktop now has an explicit
[reserved active-journal recovery flow](../ui/RECOVERY.md), but no general
restart/recovery browser for other or ambiguous artifacts. Reports
show the current operation outcome separately from the last completed receipt,
including its available row counts. A release failure cannot hide a completed
commit, and a previous receipt cannot be promoted to the current attempt.

The default reserved `.FILENAME.tablecraft-journal` sibling blocks reopening
while present, including a symlink, without following or deleting it. This only
detects that active reserved path. Journal retirement renames bundles, so a crash
after rename can leave an undiscovered retired sibling. Other caller-selected
bundles and path aliases are not a general journal-discovery mechanism. Keep the
explicit offline and outstanding-journal preconditions.
`SaveWorker::start_reserved_recovery(source)` explicitly derives that reserved
path, opens the existing offline owner and calls its checked recovery route; it
never runs as a side effect of Open. It retains terminal reports and actual
resources through the same Recover/Release/Acknowledge protocol as saves. This
is not Access multiuser locking or complete crash-restart discovery.

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

## Reviewed SQL UPDATE actions

One platform selection in `editing/mod.rs` chooses
`editing/platform/native/` on macOS, Linux and Windows or
`editing/platform/unsupported/` elsewhere. Each branch keeps its implementation
sliced by feature; common review facts, capability getters and inspected-source
checks remain in the typed `editing/query_update.rs`, `editing/query_delete.rs`
and `editing/query_insert.rs` services. Shared native preparation lives in
`editing/platform/native/query_action.rs`; each operation retains its own sealed
plan. Consumers import the selected implementation owner directly, without
compatibility forwarding modules. Unsupported services retain explicit refusals
and cannot issue native save authority. This consolidates repeated editing guards
without expanding filesystem support. A FreeBSD library compile checks the
unsupported branch; it does not qualify FreeBSD native editing.

With `offline-edit`, `ReviewQueryUpdate` accepts an opaque `InspectedQuery` from
this exact open document and explicit literal `ParameterInput` envelopes. It
rechecks both the private source identity and the document revision before parsing
the immutable SQL. The current local batch UPDATE shape uses the existing native journal editor and original-row query
planner: every SET expression is evaluated before one consuming native finish.
Declared parameters use the shared strict input service before native image
capture or source construction. Envelope checks, typed conversion and review
presentation share one cumulative presentation allowance. Joined/external targets
and unsupported expression contexts remain refused rather than being substituted
or ignored. Evaluated LIKE still requires a supplied wildcard mode and is refused
in this action route; it does not silently assume ANSI-89 for an ANSI-92 database.

Review returns `QueryUpdateReview` display facts (table, document revision,
matched and effectively changed rows, review identifier) plus a separate opaque
`ReviewedQueryUpdate`. No exclusive native lease or journal is retained while a
host considers confirmation. Plain native sources alone can enter this route;
encrypted/package sources retain the read-only editing boundary. Database text
comparison uses the existing deferred database collation resolver and its
independent comparison allowance. The adapter derives stored text policy identities
from actual native declarations, aligns them to the projected scalar snapshot and
carries them through binding and original-row evaluation. Field policy governs
comparisons involving that field; unknown/conflicting policies refuse through
the existing query contract. No ASCII or system-locale fallback is added.

`SaveWorker::start_query_update` consumes that exact capability through the same
nonreplaceable mailbox as cell edits, insertion and deletion. It checks document
validity, opens the target exclusively and stages the whole sealed query/native
plan against fresh bytes from the retained descriptor. Complete BEFORE equality,
original-intent verification and complete independently rebuilt AFTER/count
checks precede journal creation. SQL is not evaluated again. Source drift retains
the actual session for explicit release, and later failures use the existing
journal recovery/terminal receipt lifecycle. Busy/stale admission returns the
original capability; cancellation requests never fabricate rollback.

Action phases retain separate native/query/reconstruction/persistence pools.
Default work counters add no product quota. Complete BEFORE/AFTER lengths and
framing are still validated before source binding or journal operations. Fresh
proof work uses checked length-derived allowances for complete comparisons and
hashes; these are not added to an already-unbounded default counter.

The existing standard writer image envelope remains a mutation qualification
restriction, separate from the native read-opening envelope. Restart recovery
uses that known writer envelope before trusting journal lengths. Every phase
still enforces an explicitly selected budget and cancellation. Neither an image
size within the writer envelope nor a larger read allowance grants UPDATE
permission or bypasses source/program/constraint/preservation checks.
Matched counts include unchanged assignments; changed counts describe the native
plan. A successful save is followed by an explicit fresh open; the old read
snapshot is not silently relabeled as updated.

Desktop Run now dispatches classified UPDATE to this review service and renders
the shared Yes/No confirmation. It retains the original inspected target and
consuming capability privately; the shared UI owns display facts and exact
SQL/tab/document/edit/publication witnesses. Only explicit Yes starts the same
save worker. Before acceptance, No/Escape releases staging while preserving
prior results. After acceptance, cancellation is request-only and terminal
reports survive view changes until acknowledgment. Successful fresh-open restores
the SQL tab/draft and clears obsolete result snapshots without rerunning UPDATE.
The same shared presentation owner also handles the distinct DELETE and INSERT
services below. Broader action contexts, native QueryDef persistence
and DDL remain roadmap work; disabled or unsupported routes never gain authority
by classification.

## Reviewed SQL DELETE actions

With `offline-edit`, `ReviewQueryDelete` accepts the exact issued `InspectedQuery`
and explicit literal parameter inputs. It rechecks private source identity and
document revision before processing inputs or native records. Declarations and
typed values use the same strict parameter service as UPDATE; SQL-looking text
remains literal. Closed local DELETE shape admission and the existing native
writer gates still apply. This is whole-record deletion, separate from clearing
field values with UPDATE and from the single-row browser deletion review.

The Access adapter aligns actual stored-field policies to the original scalar
snapshot and supplies its native resolver to predicate evaluation. Query work
pays complete typed binding, original-row predicates and selected handles; native
work pays capture, schema/alignment and the consuming native finish. Every
predicate must succeed before that finish, including for an empty selection.
Unsupported targets, expressions, comparison policies and required programs fail
explicitly. No source path stored in SQL authorizes an external open or edit.

Review returns `QueryDeleteReview` facts (table, document revision, matched rows,
deleted roots and review identifier) plus a separate opaque `ReviewedQueryDelete`.
Review retains no exclusive file lease or journal. The shared GUI
`QueryActionReview` wraps its display effect as DELETE, while UPDATE carries its
own effect. The expected operation is captured before review; a wrong-kind reply
cannot replace the current decision. UI facts are not native save authority.

`SaveWorker::start_query_delete` consumes only the DELETE capability through the
same nonreplaceable save owner as other edits. Native save orchestration is
shared in `editing/platform/native/save/apply_query_action.rs`, with distinct typed UPDATE,
DELETE and INSERT plans and staging methods. It checks source validity, opens the target
exclusively and invokes
`stage_reviewed_delete_with_budgets` against that retained descriptor. Fresh
complete BEFORE, original TCND binding, independent normal native reconstruction
and complete AFTER/count equality precede journal setup. Save does not replay
SQL, rebind parameters or recapture a UI ordinal. The existing retained session
owns failures, recovery, release, receipts and terminal acknowledgment. A source
change cannot silently turn into a newly evaluated DELETE.

The phase allowances described for [UPDATE](#reviewed-sql-update-actions) apply
unchanged: query, native review, fresh proof and persistence are distinct finite
pools, and retained/restart recovery uses its explicit finite envelope. Empty
operations retain source/action qualification rather than bypassing admission.
This does not expand native formats, index families, macro/cascade execution,
encrypted editing or package editing.

Desktop Run uses `QueryOperation::Delete`, acquires all confirmed parameters,
then requests this review and shows the shared operation-specific Yes/No warning.
Before acceptance, No/Escape preserves the prior results and original SQL. After
acceptance, cancellation is request-only; actual resource owners and terminal
outcomes survive view changes until acknowledgment. Successful acknowledgment
freshly opens the database, restores the SQL draft/tab and discards obsolete
snapshots without executing DELETE again. Native saved action definitions and
DELETE query-designer mode remain separate unfinished work. See the
[sealed native contract](../access/writes/REVIEWED_DELETES.md) and
[shared scratch workflow](../ui/SQL_WORKSPACE.md#reviewed-local-action-queries).

## Reviewed SQL INSERT actions

With `offline-edit`, `ReviewQueryInsert` accepts the exact issued `InspectedQuery`
and literal `ParameterInput` envelopes. Source identity, document revision, input
limits and declared constraints are checked before native capture. The parsed
body selects a separate single-record VALUES or SELECT-append route; failures do
not retry another shape. Destinations remain local. Multiple VALUES tuples,
external destinations and general action/DDL support are not admitted by
statement classification.

The VALUES route retains its closed expression profile and written evaluation
order. The SELECT route constructs its actual native metadata-only inserter
before retrieval, then uses the original `NativeSource`, database comparison and
LIKE policies and existing read-query providers. Qualified SELECT/UNION shapes,
including their named/schema-discovery contracts, use the
[shared append staging service](../query/INSERT_SELECT.md). This does not grant
new query or native write profiles.

Destination names, duplicate fields and positional mapping bind against actual
native metadata before source scans. Positional appends must supply every exposed
field. Complete versus omitted intent is selected structurally from the field
list and real schema. Omission, supplied NULL and supplied empty values remain
distinct for every row. Native declarations own defaults, generation and final
constraints; current supplied-generated-value refusals remain explicit.

SELECT retrieval and assignment consume one cumulative query allowance. Source
callbacks use the same shared native allowance as capture and metadata; no mutable
native-budget borrow is held across those callbacks. The complete original read
reaches its required EOF and finishes sorting, UNION promotion and eager work
before assignment and the single consuming native batch finish. Self-append
never reads inserted rows. A failed read or assignment cannot produce an appended
prefix. An empty result still performs native admission and can return zero;
a successful batch reports its actual inserted count without a UI-imposed ceiling.

At native finish, the worker's clock authorizes source-qualified needed stored
Date/Now defaults. For SELECT append that finish follows complete
retrieval; defaults sample at most once for the batch, and empty requests,
complete supplied values and unneeded defaults do not sample it. SELECT's explicit
query providers retain their separate read-query context. The captured default
civil reading or exact absence remains in the original TCNA5 intent; confirmation,
saving and recovery do not resample it. Explicit clock calls in VALUES remain
outside its closed profile. Generated/default-clock composition and native Access
statement/row timing are not established by this review policy.

Review returns `QueryInsertReview` facts (table, document revision, inserted rows
and review identifier) plus a separate opaque `ReviewedQueryInsert`. There is no
matched-old-row count. The shared GUI effect is `Insert { inserted_rows }`, with
zero, one or multiple records. The original SQL/tab/document/publication and
confirmed inputs transfer into the review owner; the expected action kind must
match the effect. Review holds no exclusive native lease or journal.

`SaveWorker::start_query_insert` consumes this distinct capability through the
same nonreplaceable action-save owner. Its private INSERT plan retains the exact
`NativeJournalInsertPlan` family: Complete, AllocateEof, CompleteTyped, Omitted or
Clock. Fresh `stage_reviewed_insert_with_budgets` checks the complete reviewed
source, rederives native admission and deterministic defaults from original
requests and retained inputs, and requires complete rebuilt images and
inserted-count agreement before journal creation. SQL, parameters and host
providers are not replayed. The original TCNA intent is not converted into
expanded rows or another family. Fixed and growing candidates retain their
original journal geometry.

The action query/native/comparison/proof/persistence allowances remain separate
and cumulative. No/Escape before acceptance preserves previous query results and
changes no bytes. After acceptance, cancellation is request-only and the real
save/recovery owner and terminal outcomes survive until acknowledgment. Successful
acknowledgment freshly opens the source, restores the SQL draft/tab and clears
obsolete snapshots without rerunning INSERT or inferring generated record IDs.
Native saved-action persistence, append query-designer workflows, broader target
and expression contexts, required-program execution and remaining native profile
qualification stay unfinished. See the
[sealed INSERT contract](../access/writes/REVIEWED_INSERTS.md) and
[scratch action workflow](../ui/SQL_WORKSPACE.md#reviewed-local-action-queries).

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

Within the existing qualified Increment source profile, ordinary omitted
companions first resolve a supported stored default. Only complete inspected
absence of a DefaultValue declaration allows NULL for an admitted nullable
carrier. Required=true then rejects that resolved NULL through the normal native
field check; AllowZeroLength does not turn omission into empty Text. Explicit
supplied NULL is never replaced by a default, and supplied values keep their exact
carriers. A present empty, malformed, unknown or executable default remains an
error rather than falling through to NULL. Boolean NULL remains unsupported, so
an omitted Boolean companion without a default still refuses.

This resolution happens in the engine, independently in producer binding and
proof over original metadata. Runtime and UI retain the original Omitted intent
and hidden raw text; they do not manufacture a supplied NULL or amend a default
property. The selected TCNA2/TCNA6 intent retains original presence and the complete
BEFORE binding for its existing source family.
Source admission, supported carrier families, ownership/index profiles, Required,
validation/data-macro eligibility, resource limits and fresh save reconstruction
remain unchanged. The existing present-Int32 restriction on indexed companions in the multiple-owner
route remains in force. The non-generated resolver's existing NULL-absence behavior is separate
and unchanged. See [normal Increment insertion](../access/writes/NORMAL_INCREMENTING_AUTONUMBER.md)
and Microsoft's [omitted-column contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql).

Qualified current sparse histories select TCNA7 from original metadata before
consuming source/default admission. This separate source preserves unused property
dictionary names and retired native/variable slots while validating new records
through its own current-INSERT purpose. Existing all-LONG sparse generation stays
on TCNA2/6; a refused source is never retried under a different family. Original
presence, generated counter, physical slot/index references and exact BEFORE/AFTER
reconstruction remain mandatory. `NativeJournalInserter::new` stays metadata-only;
new source qualification occurs during review, with no implicit clock sampling.
See [current sparse INSERT](../access/writes/NORMAL_ACE_INSERT.md#current-sparse-metadata-and-original-presence).

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

The current operation supplies one row, with fields selected from the admitted
native insertion schema. There is no separate host field-count or per-cell/total
UTF-8 quota. The normal planner decides actual format/schema/index and
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

The initial-table route authors checked constant defaults and explicit field
policies through the property-lowering contract above. Existing policy-free
TCNT0001/TCNT0002 schemas and absent defaults remain distinct from TCNT0003
policy-bearing and TCNT0004 wider-default declarations. Omitted companions use the insertion service's
qualified stored policies and inspected absent-default NULL resolution described
above; the UI never supplies a guessed record value. New
Short Text authoring records an explicit AllowZeroLength=true UI default, with
No remaining a deliberate UI alternative. The lower-level native API also retains
absent declarations. Empty Text still requires the native insertion route's actual
explicit policy; an absent declaration does not bypass that coverage gate.
The unsaved schema retains a raw first-record buffer,
followed by explicit row review after schema saving. Inference from entered data
and implicit Access autosave remain separate work.


### INSERT input retention and guarded retry

`ReviewedInsert::retry_witness()` exposes an immutable `InsertRetryWitness`
issued during actual row review. It retains the exact original source bytes,
path and target through shared immutable ownership, without another full-image
copy. It grants no rollback, replay or save authority.

An explicit `Command::OpenInsertOriginal` captures the source under normal read
limits and compares every original byte, charging comparison work and checking
cancellation. Only a match publishes a fresh document revision. Drift returns
`InsertOriginalSourceChanged`; it never falls back to ordinary Open, begins an
insert or reuses an old review. This establishes content equality rather than
physical file identity or a continued namespace lease.

`InsertRetryEvidence` accumulates only the matching insertion operation chain.
It captures `stopped_before_target_writes()` from the original Apply and current
known outcomes from later recovery/release. A historical last-completed receipt
does not establish the current result. Release alone is not undo. Observing a
committed/replacement outcome prevents treating the same attempt as retryable.
Even no-write/original-image evidence requires resource acknowledgment, explicit
user retry, guarded opening, actual fresh field metadata and a new consuming
Review/Apply. Display diagnostics cannot establish eligibility.

The desktop retains exact input independently of native capabilities, including
text hidden under omission/NULL. Failed/uncertain saves offer read-only copying
or explicit input disposal; copying and disposal never mutate the database.
Eligible retry returns to editing only after the guarded read and fresh binding.
A recovered original image is not an inserted row, and source-drift refusal
keeps the complete input available.

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
The shared adapter also selects qualified retained-schema ACE16 sources after
DROP → ADD → rename and current INSERT. It binds resolved original row witnesses
to TCND0002 and preserves that family during review reconstruction and cold
recovery; ordinary TCND0001 sources keep their own admission. Deleting the final
live row preserves generated-counter history for a subsequent New row operation.
Current source geometry remains an implementation qualification boundary, not an
Access row or file-size limit.

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

Distinct retained fields are bounded by the observed native schema. Applying a criterion to
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

Map construction charges actual criterion/name copies, schema-name comparisons,
map capacity and handle storage. It no longer has a fixed implicit byte/work
quota. Entered operands retain exact operator arity without a separate 64 KiB
UTF-8 input cutoff. Native,
comparison, pattern and presentation phases retain distinct cumulative counters
and cancellation. No counter refills per source row, field or page. Explicit
caller quotas can refuse work; failure never substitutes an incomplete row map.

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

Match preparation and ordinary BeginEdit are separate snapshot passes, with no
offline lease held while the user considers the result. Native, comparison,
match and presentation counters remain independent without implicit product-size
quotas. Literal replacement/final edit input has no additional host byte quota;
native Review and Apply retain their actual qualification and reconstruction
contracts. Counters do not reset per row or substring.
Replace All is not supplied by this single-occurrence operation:
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

`BuildQueryDesign` owns an immutable `Arc<QueryDesignDraft>` shared with the
submitted UI witness. The worker borrows that payload while building; it does
not copy the submitted draft at dispatch. The mutable editor stays independent,
full content/revision checks remain, and UI retention charges complete logical
payload plus shared-control/out-of-line storage. This changes ownership, not
native admission or query execution rights.

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
counter matches. Requested distinct fields must belong to the observed native
schema; omission means no
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

Sorting has separate cumulative key-work, native cursor and membership/presentation
counters, without implicit product-size quotas. Retained permutations/ranks are
charged to the view's actual allocation accounting;
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

Find has no additional 64 KiB UTF-8 pattern quota. Metadata, cursor, comparison,
pattern/span and presentation counters have no implicit product-size quotas.
GUI textbox presentation policy is separate from this shared runtime service. Explicit
resource failure is an error, not a completed negative search. Counters do not
reset per field or batch. Retrieving a matching page is a second explicit command,
and publication requires the same source revision, ordinal and original field.

Microsoft's [Find dialog contract](https://support.microsoft.com/en-us/access/use-the-find-and-replace-dialog-box-to-change-data),
[FindRecord behavior](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/findrecord-macro-action)
and [wildcard reference](https://support.microsoft.com/en-us/access/access-wildcard-character-reference)
provide the behavioral target. Local tests establish full-cursor traversal,
repeated occurrences, wrap/reverse navigation, typed NULL, captured EUR display,
mandatory EOF validation and failure boundaries. These are headless implementation
checks, not native Access or cross-platform GUI qualification.

## Scratch query parameter service

`InspectQuery` accepts a worker-local document revision and bounded SQL text.
It parses syntax and resolves declared parameter specifications without opening a
native query source, binding tables, scanning records or consulting the clock or
random state. Inspection does not establish that the query can execute.

The returned immutable `InspectedQuery` owns the exact SQL and retains private
source identity as well as its document revision. It has no public constructor,
mutation API or automatic Debug/Serde output. `RunQuery` accepts that issued
target plus explicit literal inputs, rechecks both identity and revision before
input conversion, then uses the ordinary shared query executor and context.
A same-numbered revision from another worker is insufficient. Reopening a
source invalidates its earlier target.

Inspection also retains `QueryOperation::{Read, Update, Delete, Insert, Unsupported}`
from the actual parsed statement. SELECT/UNION and crosstab syntax classify as
Read; UPDATE, DELETE and INSERT have distinct classes. Classification still does
not admit unsupported action body shapes; other action/definition kinds
remain Unsupported. Classification
is inert metadata, not binding success or write permission. A string literal
containing UPDATE stays a read query. Hosts must dispatch by this fact rather
than parsing SQL themselves or retrying failed reads as writes. The connected
classification test uses unreadable table DATA and a counting clock to verify
that inspection scans no records, consults no clock and changes no bytes.

Scratch and saved queries share the actual declaration/input owner at
`query::parameters`: type constraints come from `tablecraft-query`, with at most
256 parameters under the existing parameter profile, without an aggregate UTF-8
byte quota. VALUE requires a chosen
concrete kind; SQL-looking text stays literal, and NULL remains a separate flag.
Input validation finishes before native source construction or context use.
The desktop's explicit scratch Run now uses this service to acquire declared
parameters through shared UI staging, then execute the exact retained target.
Parameterless SQL continues from the same explicit Run after inert inspection.
Desktop owns DTO/request routing and the runtime target's lifetime; query parsing,
parameter conversion and execution remain in runtime/query services. The
classified UPDATE, DELETE and qualified VALUES/SELECT INSERT routes use the same confirmed
parameter acquisition before reviewed action confirmation; only the sealed native
plan survives into save. Other action
kinds and persisted query definitions remain unsupported. See the
[scratch workspace contract](../ui/SQL_WORKSPACE.md#declared-scratch-parameters).

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

Input admission retains the existing 256-parameter profile guard; it adds no
aggregate UTF-8 quota for names, kinds or text, including text retained beside
NULL. Actual allocation/work accounting and cancellation apply independently of
query execution and final display publication. GUI textbox policy and retention
accounting do not become native parameter-value limits. A single native source and cumulative native allowance cover saved
reconstruction and subsequent execution/schema discovery. Query and display
limits remain the existing controls below.

Execution uses the existing SELECT/UNION/crosstab profile and schema-discovery
service and the [shared query context](#read-query-execution-context). No external
source registry, stored connection or VBA authority is added. Required unsupported
policies remain explicit errors. Text comparison uses the stored policies below;
the host does not guess unsupported policies, a locale or LIKE mode. General Access query
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

Native, query, comparison and presentation work no longer receive fixed implicit
byte/row/step quotas. Explicit caller controls remain cumulative and independent.
Visible pages request 100 records; complete parent/child retention uses actual
source rows under the snapshot representation and any selected quota. A fetch
batch is not a database-size limit. Default page publication has no separate
8 MiB product ceiling.

Partial pages/counts are not published after failure. Generation checks remain
necessary even when cancellation is requested. Some host-entry and supported
operation restrictions still require individual review; removing invented data
quotas does not establish complete Access functionality.

Feature-local runtime tests use the public Worker and existing testkit inputs.
They cover immutable reads, failed opens/password retry, real queries, metadata,
stored caches, collection presence/counts, corrupt child refusal and mailbox
ordering/cancellation. Shared UI tests exercise copy, focus, provenance and
virtualization; these do not qualify native screen readers or Access behavior.
Native observations and current platform limits are recorded in the
[desktop guide](../ui/DESKTOP.md). Payload-free diagnostics remain governed by
[the tracing contract](../development/TRACING.md).

## Reviewed saved-field rename

`InspectFieldRenameSource` binds the document revision, true table/field names
and actual native column number. It observes the complete qualified source
without a proposed replacement, AFTER image or save lease. Direct native table
headers now carry their actual optional native number; SQL aliases and synthetic
headings do not acquire one from their displayed position or caption.

`ReviewFieldRename` issues exact presentation facts and a separate consuming
`ReviewedFieldRename` from the sealed native plan. The normal runtime now selects
current-metadata profile4 or retained-storage profile5 from original native
metadata before inspection and consuming review, independently of row count.
The retained route handles deleted native ordinals and appended live-property
snapshots without moving prior OFF/history rows. The original empty
same-width profile1, empty fitted profile2 and dense populated Number replay
profile3 remain unchanged for explicit native callers and old intent recovery;
they are not fallback paths from a refused current source. See
[the native contract](../access/writes/FIELD_RENAME.md).

Profile4 qualifies the current unencoded ACE16/subversion-zero eleven-page image:
the actual single-user-table role graph, explicit Name AutoCorrect OFF, ordinary
row/index/constraint and NoRequired evidence, target metadata and exclusive
property ownership must all agree. NameMap/expression dependencies, calculated
fields, nonempty validation rules and nonliteral defaults refuse. Existing
record deletions, counter history and inactive name/row bytes do not themselves
require a creator-history replay or disqualify the source. Actual parser-bound
definition/property capacity still limits each proposal, and every byte outside
the proved metadata scope remains protected. This is not general imported-file,
arbitrary field-type, allocation-growth or native-program support.

Inspection returns the explicit shared fitted-ASCII policy only after this source
proof; equal-width, case-only, unchanged and differently sized proposals share
that same current family. Fitted shape is not a physical capacity guarantee.
Only clean unsupported-source failures become unavailable. Corruption,
cancellation, resource, conflict and preservation failures retain their typed
errors. No displayed boolean or review DTO constructs save authority.
`SaveWorker::start_field_rename` uses the
same retained offline worker and separate native/proof/persistence pools as the
other schema operations. Failed saves retain their actual resource owners.

Only ColumnRenamed-only receipts qualify the corresponding outcome facts;
row, query or options effects refuse. `OpenFieldRenamed` requires the accepted
capability's complete expected-image witness before a fresh document can publish.
Acknowledged persistence alone does not prove that the reopened file still
matches. An unchanged request still performs native review, then consumes to a
read-only witness through `into_unchanged_reopen`; it receives no save ID or
journal and must not be reported as a changed save. This also applies to
qualified populated sources. The desktop retains that read-only owner and blocks
conflicting edits until its exact witness reopens; the broader host editing guard
does not imply a running native save actor.

The [coordinated service record](../../tests/results/schema/field-schema-services-2026-11-09.json)
includes actual runtime save/witness, unchanged, stale and retained-failure cases.
The [shared field-name UI](../ui/FIELD_RENAME.md) now connects Datasheet/Design
proposals, exact close decisions and distinct checked replacement publication.
Normal-host interaction checks now pass, including real Design save/reopen,
unchanged review, close, duplicate rejection and retained sort retry. The
[coordinated record](../../tests/results/schema/field-workbench-workflows-2026-11-09.json)
retains failures and final evidence separately. The separate
source-free status-panel matrix has 22 successful renders and direct review of every image without clipping; compact
German/Japanese layouts retain the complete Ribbon through horizontal overflow. No-op review consumes
only its readonly
witness, and its presentation never claims a changed save. Scratch SQL remains
byte-exact; native filter/sort intent is freshly revalidated without rewriting.
No Microsoft Access or Windows rename-save comparison is claimed.

Current-profile acceptance now passes repeated grow/shrink/regrow and
row-edit/insert/delete-then-rename cases with independent complete images.
Normal desktop widgets and fresh MCP sessions retain nullable rows, tombstones,
generated history and checked reopening. These are separate executed checks,
not inferred from earlier profile3 or single-rename results. See the current
[verification accounting](../IMPLEMENTATION.md#repeated-field-renames-and-current-metadata--9-november-2026).

## Reviewed existing-table field additions

`InspectFieldAdditionAvailability` returns separate Number and Short Text source
facts bound to the exact document revision and table. One immutable image capture
and one candidate-owned native source proof publish both current kind facts. The
UI still requires the selected kind's exact positive fact before opening its
naming editor; named review freshly qualifies the complete declaration. Inspection
constructs neither a speculative named candidate nor a write capability. Known
typed Source refusals produce unavailable facts; malformed, cancelled and
resource-limited inspections remain errors. Older Jet/ACE formats gain no
admission from an ACE16 proof.

The typed candidate uses that same ACE source owner. For an original Number
source, it reuses the complete canonical proof and original source-policy check.
For repeated Number history, it retains the actual checked source, declarations
and proven dictionary policy from the existing history owner. Generated-field
restrictions, derived dictionary order, row-history bounds, allocation roles and
complete source comparison remain required. Mixed sources keep their separate
complete replay. This shares proof-bearing native state rather than inferring
eligibility from an old availability Boolean, and avoids reconstructing a large
Number history twice under the unchanged cumulative native allowance.

`ReviewAddField` captures the complete source-free
`tablecraft_builders::table::FieldAdditionDeclaration`: `NumberLongZero`, or
`ShortText` with an explicit nonzero 1–255 UTF-16-unit width and Unicode
compression permission. Number fixes nullable LONG, stored Required(false) and
typed Int32 zero. Short Text fixes nullable TEXT, stored Required(false),
AllowZeroLength(true) and absent DefaultValue. The current saved-table gallery
requests Short Text with capacity 255 and compression enabled; this does not change the initial
creator's existing compression default or enable arbitrary field properties.

The consuming `ReviewedFieldAddition` derives its `captured_declaration()` from
the sealed native plan. `FieldAdditionReview.declaration` is separate data. The
host checks both against the exact submitted declaration, including width and
compression, before starting the nonreplaceable `SaveWorker::start_field_addition`.
A matching display DTO cannot substitute for the capability's declaration.

Populated current-metadata ACE16 tables now use separate TCAC0008 Number and
TCAC0009 Short Text requests. Both retain the existing review, nonreplaceable Save
worker and guarded reopen. The captured native plan reports its actual declaration;
it never labels a Short Text request as Number. Short Text keeps old records
unchanged/NULL, with explicit width and compression and no default. Its separate
TCNI0015 operation updates present or absent historical Text fields using actual
stored field counts and checked expected values. It supports replacement, empty
Text, NULL and refill through the same typed cell-edit service as other supported
edits. The shared row capability also supports ID-only tables with an eligible
Text field. Desktop and MCP gain no independent native write path. Broader source
layouts, indexed targets and bulk historical Text updates remain unqualified.

Actual inert metadata selects one source purpose before native qualification.
Number retains TCAC0003/0004 for their supported creator prefixes and property
carriers; mixed fields or policies requiring the typed profile select TCAC0005.
Short Text selects TCAC0005. A failed qualifier or decoder never retries another
profile. The typed route reconstructs the canonical inert prefix and every
closed historical field-addition tuple before complete source comparison. Old
Number service requests and recovery languages remain separate supported
consumers; the unified gallery does not narrow their existing source admission.

On sources with the actual native database containment role, the separate normal
selector chooses the explicit-OFF purpose before any proof. The owned settled
source must qualify; no failed TCAC3–5 plan retries TCAC6. This fitted eleven-page
profile offers Number or TEXT(255)/compression enabled, and requires an ASCII
comparison namespace. Its readonly source inspection measures genuine encoder and
allocator capacity: the booleans mean some valid name can fit, while exact named
review remains authoritative. Low-level old Number services and TCAC1–5 requests
retain their original source languages. The same generic service consumes the
sealed TCAC6 plan, so there is no extra host-specific save endpoint or automatic
settings change. The [separate service record](../../tests/results/schema/field-schema-services-2026-11-09.json)
covers actual runtime and headless GUI saves, complete reopened images and the
historical later-event refusal; it does not inherit the historical passing count below.
The separate metadata-selected `AfterExplicitOffHistory` route now consumes a
sealed chronological replay plan and TCAC7 staged owner through these same
services. Runtime and actual GUI saves match four independently observed second
additions; a further raw-only case stays separate. Complete-image reconstruction,
per-kind measured capacity, one actor and the checked reopen witness remain
mandatory. See the [current coordinated record](../../tests/results/schema/field-workbench-workflows-2026-11-09.json).

One private plan composition and renamed `StagedFieldAddition` owner reuse fresh
original-image requalification, the exclusive offline session and checked V2
journal. Cold recovery and settled confirmation reconstruct the exact declaration
under one cumulative allowance. Schema-only ColumnAdded receipts carry no row,
query or database-options counts. `field_addition_reopen_facts()` classifies only
that receipt scope; it grants no native authority. The same accepted capability's
complete `AddedColumnReopen` witness is required before fresh catalog publication.

The shared UI retains one naming/review/save/close controller for both kinds.
Save/Discard/Cancel binds the exact owner, draft revision and declaration; native
failure keeps the proposed name and deferred transition. Known saved AFTER plus
failed checked reopen retains its outcome and stale-source mutation guards.
Successful publication preserves tab identities/order, scratch SQL and preferences
while invalidating old native row/results handles; unrelated dirty drafts still
block the schema write. The bounded empty canonical ACE16 source, sixteen-field
ceiling, actual property-page ledger and single-rollover limits remain explicit
in the [native contract](../access/writes/ADD_COLUMN.md).

The [field workflow record](../../tests/results/schema/field-addition-gui-workflows-2026-11-08.json)
passes 1,505 unique selected checks, including all 248 runtime, 606 shared UI and
173 desktop cases. Five existing ignored cases are excluded. Actual mixed-field
saves and row inputs, declaration substitution refusal and N13/N14 budget/rollover
workflows pass. Strict lint and feature-configuration checks pass. Native Microsoft
Access interaction and actual Windows execution remain unqualified; earlier
Number records below retain their original scope.

## Reviewed existing-table Number additions

`ReviewAddNumberColumn` retains the same consuming `ReviewedNumberColumn` and
`SaveWorker::start_number_column` across the original first ADD and repeated ADD.
The engine observes the selected table's actual LvProp carrier through native
logical-root resolution; runtime calls one matching TCAC0003 or TCAC0004 planner,
without retry on refusal. Each planner rechecks its complete source and exact
Required(false)/typed-zero declaration. Read-only availability uses the same
selection and source proof, grants no capability and distinguishes known Source
refusals from failed/cancelled/resource-limited inspections. Older formats remain
unavailable before incompatible carrier inspection.

A private plan variant retains the sealed purpose through the nonreplaceable
worker and adapter. Both stages reuse schema-only ColumnAdded receipts, retained
failure/recovery ownership and the same complete-image AddedColumnReopen witness.
The host publishes no new revision from receipt alone. The previously qualified
Number Click-to-Add/name/Save flow repeats after successful checked reopening; their
saved facts, stale-source safeguards and other drafts retain the existing lifecycle.
The [native contract](../access/writes/ADD_COLUMN.md#repeated-number-addition-with-existing-external-properties)
defines the exact empty-table/property-history/page-capacity limits. TCAC0004
now permits one property-page rollover to immediate EOF page 11 and later fitted
page-11 bodies, within the existing sixteen-field ceiling; it retains the same
original declaration/framing and checked V2 lifecycle. A full page 11 still
refuses. Native proof, comparison and persistence use their existing independent
allowances, including during whole-history reconstruction.

The [rollover workflow record](../../tests/results/schema/number-column-rollover-workflows-2026-11-08.json)
includes a normal N13→N14 save, actual actor acknowledgement and full-image
reopen, followed by ordinary insertion with every Number field omitted. Fresh
reads verify generated ID1 and fourteen non-NULL zeros. The desktop exercises
those same native services through its ordinary widgets. No general column
alteration or older-format write admission follows from service reuse.

## Reviewed explicit database options

With `offline-edit` and `ace16-create`, the separate
`ReviewDisableNameAutoCorrect { document_revision }` request reviews only the
explicit source-bound disable-both operation. Its current creator source profile
and preserved bytes are defined by the [native options contract](../access/writes/DATABASE_OPTIONS.md).
The normal desktop now composes this service through its shared File Options
dialog. `SaveWorker::supports_database_options()` reports native save-platform
support independently of the current image's positive source eligibility. The
corrected composed functional workflow and scoped offscreen review pass.

`InspectDatabaseOptions { document_revision }` returns the same document revision,
`NameAutoCorrectState::{Missing, Absent, ExplicitOff, Other}` and a separate
`disable_available` fact. Stored-state inspection works on supported native read
hosts independently of save support. Positive eligibility reuses the complete
source preflight without allocating an AFTER candidate or retaining an exclusive
file lease. UI Unknown means unobserved, never OFF. Neither observation nor
eligibility grants native save authority; actual review requalifies the source.

A successful unlocked review returns `DatabaseOptionsReview` and a consuming
`ReviewedDatabaseOptions`; review never holds the native exclusive file owner,
creates a journal or changes defaults. `SaveWorker::start_disable_name_auto_correct`
uses the existing nonreplaceable worker and adapter resizable pair. It retains
owners and terminal failures across cancellation, returns unconsumed capabilities
for stale/busy start refusals, and rechecks the complete original before staging.
Committed or verified replacement receipts report only
`SaveDatabaseOptionsChange::NameAutoCorrectDisabled`; table/query changes and
matched/changed row counts remain absent. `database_options_reopen_facts` keeps
these scope checks in runtime rather than duplicating classification in the host.

The capability's `DatabaseOptionsReopen` witness is a read-only full-AFTER guard.
Ordinary successful Apply acknowledges internally before this read; retained
failure/cancellation and resource resolution retain explicit controls. After
actual save acknowledgement, `OpenDatabaseOptionsSaved` creates a fresh
read session only when every expected byte matches; a changed file refuses
without replaying the option action. A later table rename requires its own
explicit review/save. Unsupported save hosts retain explicit refusals.

Before review, the UI requires a separate exact close-objects confirmation bound
to the current revision, graph order/owners, active tab and confirmation identity.
Dirty stored-object drafts block this flow. On known replacement acknowledgement
plus successful full witness, a distinct options publication closes the confirmed
persisted tabs and invalidates native result/row handles. Source-free scratch SQL
text, IDs and preferences remain a TableCraft tool boundary, not a claim that
Access retains unsaved native Query objects. The neutral replacement graph owner
also serves field additions with their existing KeepAll policy; it supplies no
shared native write authority and never performs a generic Open/layout reset.

Failed/original outcomes retain the current graph. Failed full-witness reads keep
the accepted witness, saved outcome and cleanup state for retry. Explicit Return
preserves that history and blocks mutations against the stale source until a fresh
document open, while read/navigation/file/close transitions remain available.
Readonly cancellation retires the exact host state immediately because the worker
revokes reply publication; SaveWorker cancellation retains its terminal owner.
No implicit Name AutoCorrect change or Rename hook is introduced.

The same explicit service now also saves OFF after qualified empty creator
field histories using the distinct `TCDO0002`/profile 2. Source purpose is chosen
once from actual catalog-column LvProp map ownership, then exactly that source
proof and constructor run. The original `TCDO0001` source/decoder is unchanged;
no failed old qualification or decoder is retried as the new profile. The private
selected plan remains attached to the same consuming review, SaveWorker,
options-only outcome and full-image reopen witness.

Eligibility proves both source and actual fitted storage without allocating the
target AFTER or holding a lease. The current FieldHistory purpose requires an
11-page image, missing MSysDb, a sole latest user-property owner and one existing
page 10 with enough room for the OFF body and catalog append. N12's observed 596
free bytes qualify; N13's 71 refuse before review. The inert four-state observation
continues to work on ineligible catalogs; missing or unknown never means OFF.
No allowance was raised or reset. Equal-length V2 recovery accepts qualified
pending original/replacement mixtures; a foreign shortened or grown target refuses
without writes rather than inventing a tail.

The [field-history composition record](../../tests/results/schema/field-history-options-workflows-2026-11-09.json)
passes 991 unique selected checks with five existing ignored cases, including all
257 runtime and 179 desktop cases. It also retains the strict all-feature and
minimal ACE create+journal lint results. These checks exercise five independent
field-history sources and actual Number/mixed-history GUI saves, with complete
AFTER and scratch-text preservation. Prior UI suite/focused checks and renders
are not repeated or counted in that total. The later
[ADD-after-OFF service](../access/writes/ADD_COLUMN.md#number-and-short-text-after-explicit-off)
has its own source and verification record. Repeated settings changes and broader
storage/older native profiles remain work; the existing creator-OFF→table-rename
source stays separate.

The [coordinated Options record](../../tests/results/general/database-options-workflows-2026-11-08.json)
includes all 253 passing runtime cases and one existing ignored case. The broader
selected-package run and focused corrections also cover actual host cancellation
of acknowledged replacement reads, exact-witness Retry and failed-read Return.
These results are not a full-workspace or native Windows/Access qualification.
Nineteen offscreen frames pass layout assertions and direct visual review;
the final Return frames distinguish Hide details from closing Options.

## Guarded saved-query SQL edits

The [saved-query editing contract](SAVED_QUERY_EDITING.md) defines complete SQL
review, consuming save/status ownership, semantic no-op witness reads and atomic
new-revision publication retaining other active/parked drafts. Query facts remain
separate from row/schema/settings outcomes. Only acknowledged known AFTER or an
explicitly unchanged review followed by the same successful full-image witness
can resolve exact visible text. Failed/original outcomes preserve dirty drafts;
Open/Create/close and unrelated writes cannot bypass them. The distinct `ReviewedQuerySql`/`start_query_sql` route now targets qualified
existing SELECT projection/WHERE fragments through TCQE0002; the low-level
literal TCQE0001 service remains unchanged. The [coordinated record](../../tests/results/queries/full-sql-expression-workflows-2026-11-08.json)
retains its selected passing checks and corrected failures. This route does not establish general
QueryDef creation/Save As,
Access 97–2019 write parity or native Microsoft Access save behavior.

## Reviewed saved-field deletion

`InspectFieldDropSource` binds the exact document revision, true table/field
names and actual native number. `FieldDropSourceInspection.source_eligible` is
an eligibility-only observation, with no candidate AFTER, lease, save support or
physical-fit promise. Populated source inspection validates actual rows/indexes
under the same explicit cumulative native budget. Only explicit unsupported source/target leaves become false; malformed,
cancelled and resource-limited reads retain typed errors. Unsupported builds
retain ordinary metadata and refuse unsupported editing explicitly.

`ReviewFieldDrop` creates `FieldDropReview` presentation facts and a separate
consuming `ReviewedFieldDrop` from the actual sealed native plan. Metadata selects
[the original empty-source or separate populated-tail ACE16 language](../access/writes/FIELD_DROP.md)
before complete source/target qualification. There is no error-driven fallback or
unchanged/no-op branch. Populated deletion retains all DATA bytes and row counts,
removes only the qualified native tail field and its live property group, and
uses a distinct TCFD0002 original request. Older TCFD0001 remains empty-source-only.
No write lease is held during confirmation.
`SaveWorker::start_field_drop` consumes the capability through the real adapter
and the existing nonreplaceable retained worker, with separate native/proof/
persistence allowances. Failures retain actual resources and typed causes.

Only a ColumnDropped-only receipt qualifies `field_drop_reopen_facts`; ADD,
rename, rows, queries and options effects are distinct. The exact accepted
`FieldDroppedReopen` must match the complete expected image in `OpenFieldDropped`.
A terminal save and its ACK leave the original read session/revision/schema
intact; publication is allowed only after this full-image read succeeds.
Cancellation, changed source and failed reopen do not become deletion success.

The [checkpoint record](../../tests/results/schema/saved-field-drop-services-2026-11-09.json)
retains corrected native/recovery, actual adapter-file and runtime service cases,
plus source-free shared model/regression suites. The subsequent
[normal GUI workflow](../ui/FIELD_DROP.md) connects Datasheet Delete Field and
saved Design Delete Rows/Delete to these exact services. Metadata inspection
precedes local Yes confirmation; explicit Save repeats target admission through
the consuming review. Only full-witness publication changes the visible stored
schema. The [GUI record](../../tests/results/schema/saved-field-drop-gui-2026-11-09.json)
separates real headless saves from source-free renders and incomplete native
interaction. Sparse-generated INSERT was outside the earlier service checkpoint;
the later GUI record separately checks omitted INSERT after witnessed Number DROP.
