# Blank Access database creation

File > New > Blank Database now accepts an `.accdb` destination and an explicit
Create action. Browse selects a location; cancelling the picker starts no
operation. Creation requires an absolute `.accdb` destination and never replaces
an existing file. The shared runtime constructs a real unencrypted ACE 16 image
with zero user tables, columns and rows. Its system catalog and allocation maps
are serialized directly; no sample database, imported template or header-only
placeholder supplies the result.

The desktop enables the runtime's optional `ace16-create` feature. macOS and
Linux use the existing Unix required-publication protocol, including file and
captured-directory barriers. Windows uses a separate local-NTFS write-through
handle-relative rename and retained-file flush. Other Windows filesystems and
network locations refuse before staging. This implementation still needs actual
Windows execution; cross-compilation does not establish filesystem behavior.
See [fresh native construction](../access/writes/FRESH_DATABASE_CREATION.md) and
[the runtime creation contract](../application/RUNTIME.md#fresh-blank-database-creation).

## Completion, cancellation and reopening

The creation operation owns its destination and terminal receipt separately from
the replaceable read worker. Another view cannot supersede publication. The UI
retains the operation while cancellation finishes, including when closing the
window, and retains the terminal receipt or failure after acknowledgement.
Cancellation does not assert that publication was rolled back.

Failures distinguish not published, uncertain visibility, published without a
confirmed platform durability step, and published with cleanup failure. The destination
is not automatically deleted or retried after an uncertain or visible outcome.
Only a successful creation report initiates `OpenCreated`. During ordinary
bounded source capture, that command requires the exact canonical blank bytes
before native reader qualification. This binds the captured content, not physical
file identity or continued identity of the publication's captured parent.
Reopen failure preserves the successful publication fact.

## Initial unsaved table

After successful guarded reopening, the desktop opens an unsaved `Table1` draft.
Ordinarily opening a zero-table database does not invent one. The draft has a
separate object/session identity, an ID AutoNumber primary declaration and one
new-record placeholder; it is absent from the persisted navigation catalog.
There are no native row handles, stored records or manufactured query results.

Click to Add offers explicit Short Text (255 UTF-16 units) and Long Integer
fields, followed by field naming. The shared renderer-neutral table builder
retains field identities and a revision; rejected names and exhausted limits do
not partially change the design. Its current authoring ceiling is sixteen
fields, and the initial generated ID is protected. This is a bounded authoring
profile, not the full Access field designer. Record entry and automatic type
inference are not implemented.

Switching tabs parks the actual draft. Closing a pristine draft discards it
without writing. A changed draft requires explicit discard or cancellation.
Database replacement, a new creation request and window close retain their
original requested action while the same confirmation is pending; another
request cannot replace it. Cancellation keeps the draft. The Save action requests
a table name and runs the source-bound persistence operation described below.
macOS/Linux use their retained bundle owner; Windows now uses a separate
local-NTFS flat-sidecar owner. Fresh-file creation alone does not grant same-file
Save capability, and actual Windows execution remains unqualified.

Microsoft's [Create a new database](https://support.microsoft.com/en-us/access/create-a-new-database)
describes the intended complete continuation: Create opens an initial `Table1`
in Datasheet view, with focus in Click to Add; entering data defines fields,
and closing a changed table offers save, discard or cancel. The implementation
now connects the bounded initial schema to a real same-file save. Record entry,
automatic field inference, general schema changes and complete Design view
remain unfinished.

## Creating the first table after Open

Ordinary Open and acknowledged recovery followed by Open leave a zero-table
database without a draft. Create > Table now explicitly opens the same unsaved
Datasheet draft when the opened source is the exact canonical ACE 16 blank.
A rolled-back first-table operation can therefore return to that workflow after
recovery acknowledgement; recovery itself never starts table authoring.

The runtime compares the complete immutable captured source against the canonical
blank under the read operation's budget and cancellation checks. It publishes an
advisory availability fact bound to the opened document revision. The shared UI
accepts that fact only for the matching revision and also requires the existing
Save capability, no user tables and no conflicting draft, modal or operation.
Selecting the command changes only local draft state. It neither acquires a
writable target nor creates a journal. Review still qualifies the original source
and declarations; Save freshly rechecks the retained target before mutation.

A readable noncanonical blank or a database with existing user tables keeps this
command disabled. This does not restrict opening or reading those sources, and
it does not authorize general table addition or schema changes. Table Design
remains disabled. Microsoft's
[Create a table and add fields](https://support.microsoft.com/en-us/access/create-a-table-and-add-fields)
distinguishes Create > Table, which opens Datasheet view, from Create > Table
Design, which opens Design view. The retained Datasheet and Design captures guide
those separate surfaces; they do not capture activation of the Create command.

This route uses the existing macOS/Linux and Windows Save implementations and
retains their platform limits described above. Windows cross-compilation and
headless checks do not establish actual Windows execution or native Access
behavior. The new explicit-open acceptance checks are separate from the
historical test counts below; native Access comparison remains pending.

## Saving the initial table

The shared Save prompt accepts an exact table name and submits
the current immutable draft snapshot. Its supported schema is the initial ID
AutoNumber primary key plus explicitly added Short Text or Long Integer fields,
within the sixteen-field ceiling. No records are authored by this operation.
The UI keeps the draft distinct from the persisted database while review and
saving run; neither accepting the name nor closing the prompt proves a save.

Runtime first qualifies the exact original canonical blank and declarations,
then issues a consuming review. Its existing nonreplaceable SaveWorker acquires
the target, compares all original bytes and rederives the native operation under
that same retained descriptor. A distinct TCNT schema intent and checked V2
journal resize the original file; the operation does not replace the path with a
freshly created image. Unix finalization acknowledges retirement/removal barriers.
Windows acknowledges its write-through retirement rename and retains the intact
retired artifact; the completion report distinguishes those namespace facts from
native commitment and does not claim durable Windows deletion. See
[first-table persistence](../access/writes/FIRST_TABLE_PERSISTENCE.md).

Only an acknowledged committed/verified-replacement receipt with the matching
schema count starts guarded reopening. Reopening compares the complete captured
file against the expected content retained during review before native reading.
The host also matches the original document, draft revision and table identity.
An expected-content witness is not itself a save receipt or physical file identity.
Only a successful reopen promotes the matching draft into the real catalog/table.
A failed reopen preserves the known save fact and displays a separate failure.

Cancellation before durable effects keeps the draft. Once a journal is retained,
the UI retains its outcome and offers only the recovery/release actions supported
by the actual owner. A cancelled or failed commit is not reported as rollback or
permission to retry. If saving was requested by a pending close/open/create
transition, successful completion resumes that original action; cancellation or
failure does not silently replace it.

All 79 desktop checks pass, including the connected create/save/query flow,
same-file identity, cancellation before review dispatch, source drift, host
admission refusal and deferred file opening after successful Save. The shared
batch passes all 123 runtime and ten builder tests; eleven focused Save UI cases
pass, including keyboard confirmation and retained-resource controls. These are
historical local results. Windows fresh creation and same-file Save are now
implemented, but actual Windows execution and native Access save/reopen remain
unqualified.

## Evidence and remaining qualification

Feature-local headless checks cover five shared UI creation cases, four desktop
composition cases and five runtime creation cases. They exercise explicit
submission, cancellation, retained outcomes, no-clobber publication and guarded
reopening. Engine and adapter checks compare complete bytes against the
independently authored testkit blank fixture and enforce resource/cancellation
boundaries. The CLI exposes the same construction through
[`create --blank`](../cli/NATIVE_CREATION.md).

A separate read-only Jackcess 5.0.4 observation of a CLI-created image recognized
V2016, zero user tables, the three expected system tables and seven catalog
objects. Expression evaluation and external resolution were disabled; file
bytes were unchanged afterward. This is independent-reader evidence, not native
Access open/save qualification. Seven draft-specific UI checks and three desktop
transition checks additionally cover source separation, parked drafts, discarded
requests, stale approvals and whole oversized name rejection. Final light/dark
and compact translated frames were reviewed offscreen; see the
[presentation record](PRESENTATION_PREVIEW.md#initial-unsaved-table--7-november-2026).
The new naming surface has its own English light/dark and compact German/Japanese
offscreen review recorded in the [Save presentation evidence](PRESENTATION_PREVIEW.md#initial-table-save-as--7-november-2026).
Native Access comparison remains pending.

The New form is a headless presentation adaptation of the retained Backstage
layout and documented Microsoft workflow. No native Access New-form capture or
cross-platform visual/interaction parity is claimed. See
[visual references](VISUAL_REFERENCES.md).
