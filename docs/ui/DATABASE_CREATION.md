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

Click to Add offers six explicit field declarations, followed by field naming.
The shared renderer-neutral table builder retains field identities and a revision;
rejected names and exhausted limits do not partially change the design. Its
current authoring ceiling is sixteen fields, and the initial generated ID is
protected. This is a bounded authoring profile, not the full Access field
designer. Automatic type inference remains unimplemented.

Choosing a type selects its temporary field name in an inline header editor.
Double-clicking an ordinary header reopens that editor. Enter, Tab or leaving
the editor attempts to commit the exact name; Escape cancels that name edit.
Rejected or duplicate names remain editable and block schema Save until
corrected or cancelled. Save also processes the current pending name before
capturing the declaration, including same-frame typing and the Save shortcut.
The protected generated ID never becomes a name editor.

The new-record row accepts raw input before schema Save. It keeps Omitted,
explicit NULL and supplied text distinct, keyed by draft field identity across
renaming and added fields. Choosing Omitted retains previously typed text as a
presentation cache; it does not supply that text to the backend. The generated
ID stays omitted and displays `(New)`. Input is bounded to eight MiB total raw
text and the existing per-field editor limits. Oversized insertion is rejected
whole, visibly blocks Save and requires an accepted edit or mode correction.
No row is stored while editing this buffer.

| Gallery choice | Native declaration | Authored constant default |
| --- | --- | --- |
| Short Text (255) | Short Text, 255 UTF-16 units | Absent |
| Number (Long Integer) | Signed 32-bit integer | `0` |
| Large Number | Signed 64-bit integer | `0` |
| Currency | Fixed-point Currency | `0.0000` |
| Date & Time | Legacy Date/Time | Absent; no implicit current date/time |
| Yes/No | Boolean | `False` |

These defaults are persisted metadata, retained in the reviewed declaration;
they are not placeholder row values or display formatting. Choosing a field
never inserts a record. After saving, generated-ID insertion can resolve
omitted companions with these checked constant defaults while preserving original
field presence for fresh proof and recovery. Supplied values take precedence;
a missing or unsupported default does not silently select another route. The
initial ID keeps Increment generation and its primary key; it has no explicit constant default. Other Number sizes, Decimal,
Replication ID/GUID and Date/Time Extended are not silently substituted.
Long Text, Lookup & Relationship, Rich Text, Attachment, Hyperlink, Calculated
Field and More Fields remain visible disabled entries with unavailable tooltips.

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
now connects the bounded initial schema to a real same-file save and retains
one pending record through the explicit continuation below. Automatic field
inference, general schema changes and complete Design view remain unfinished.

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

## Click to Add reference boundaries

Microsoft documents explicit Click to Add choices for
[Short Text](https://support.microsoft.com/en-us/access/create-or-delete-a-short-text-field),
[Number, Large Number and Currency](https://support.microsoft.com/en-us/access/create-or-delete-a-number-or-currency-field),
[Date & Time](https://support.microsoft.com/en-us/access/add-a-date-or-time-stamp-to-new-records)
and [Yes/No](https://support.microsoft.com/en-us/access/create-or-delete-a-yes-no-field-in-access-databases).
Choosing a declaration and then naming it is distinct from entering data and
letting Access infer a type. Neither workflow establishes all persisted field
properties merely from a type's storage representation.

Number has multiple Field Size choices. Access exposes a configurable
[default number field size](https://support.microsoft.com/en-us/access/customize-design-settings-for-objects-in-your-database),
so a generic Number choice alone does not establish one universal numeric subtype.
TableCraft labels its supported Number choice as Long Integer explicitly;
it does not import Access application options or offer the other subtypes.
The gallery's constant zero/False defaults are explicit declarations. Arbitrary
Default Value expressions, Required, validation, extra indexing and display
properties remain separate capabilities. Microsoft's
[default-value contract](https://support.microsoft.com/en-us/access/set-default-values-for-fields-or-controls)
distinguishes an absent default from an explicit value. The
[date-stamp workflow](https://support.microsoft.com/en-us/access/add-a-date-or-time-stamp-to-new-records)
sets `Now()` or `Date()` separately after adding a Date & Time field; TableCraft
does not invent either expression.

The retained Windows Access Datasheet and Design images were reopened for this
slice: 1920×1141 and 1920×1165 pixels, respectively, inside VirtualBox, with
unknown display scale. They show Click to Add and native field types but no
expanded field-type gallery. Gallery composition and subtype controls therefore
use the documented workflows rather than a claimed captured-menu match.
Long Text, Rich Text, Attachment, lookup and calculated-field workflows require
additional semantics; listing their documented names does not enable them.

## Schema naming and record saving are distinct

Microsoft documents renaming a field by editing its column heading, and naming a
new table through Save As on its first Save. Closing a changed initial `Table1`
offers save, discard or cancellation; closing it without ever saving can discard
the table together with entered data. These are initial-table lifecycle rules,
not a general promise that closing any persisted table reverses its records.
See [Create a new database](https://support.microsoft.com/en-us/access/create-a-new-database)
and the [keyboard table-creation workflow](https://support.microsoft.com/en-us/accessibility/access/use-a-screen-reader-to-create-tables-in-access-desktop-databases).

For persisted tables, Microsoft describes record saving when viewing another
record or closing the table, with Shift+Enter as an explicit record-save action.
TableCraft currently retains its explicit whole-row Review and Apply flow.
Header naming, schema Save and reviewed insertion must not be described as
Access record autosave. See
[Ways to add, edit, and delete records](https://support.microsoft.com/en-us/access/ways-to-add-edit-and-delete-records).

The retained Windows Datasheet image shows stored records, the new-record
placeholder and field headings. It does not show an active header-name editor,
first-record entry in an unsaved table, or the Save/Discard interactions. Those
interaction details use documented contracts and headless checks; exact native
interaction parity remains unverified.

## Saving the initial table

The shared Save prompt accepts an exact table name and submits
the current immutable draft snapshot. Its supported schema is the initial ID
AutoNumber primary key plus the explicit field declarations and defaults listed
above, within the sixteen-field ceiling. No records are authored by this operation.
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

Default-bearing definitions use closed `TCNT0002` intents. Existing
`TCNT0001` declarations retain their encoding and recovery route; they do not
acquire newly inferred defaults during replay. Fresh recovery reconstructs the
captured constant declarations before qualifying the original and replacement
images. Neither intent permits executing an arbitrary default expression.

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

## Continuing to the first record

Schema Save and row Save are separate operations. After the schema receives its
acknowledged completion and guarded reopen succeeds, an actually modified record
buffer is transferred to a fresh runtime-issued insertion draft for that real
table and document revision. The transfer retains the original draft identities
and checks the real field names, order and declared carriers. An untouched placeholder never inserts an empty
record. Original Omitted/NULL/supplied intent and hidden raw text are retained;
display formatting never becomes edit input.

The reopened Datasheet shows the retained record for the existing explicit
Review and Apply flow. Preparing this editor does not review or apply the row
automatically. Invalid values can therefore leave a successfully saved table
alongside an unsaved record; the UI retains the input for correction and keeps
the schema completion distinct from any insertion outcome.

If the fresh target or schema does not match, a pending-record dialog retains
the input and offers the admitted Retry or Discard action. Discard abandons only
the unsaved record. It never deletes the committed table or rewrites the schema
receipt as a rollback. Cancelling the table-name prompt keeps both the pending
name and record. Close/open transitions wait for this continuation and any
insertion worker/resource owner; a retained journal still requires its explicit
recovery or release path.

This is a TableCraft continuation through two reviewed operations, not Access
record autosave or an atomic schema-and-record transaction. Connected headless
desktop checks cover the real schema save/reopen followed by retained row input;
native Windows execution and Access interaction comparison remain unqualified.

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
