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
request cannot replace it. Cancellation keeps the draft. Save is visibly
unavailable: no confirmation claims to persist a table that was not saved.

Microsoft's [Create a new database](https://support.microsoft.com/en-us/access/create-a-new-database)
describes the intended complete continuation: Create opens an initial `Table1`
in Datasheet view, with focus in Click to Add; entering data defines fields,
and closing a changed table offers save, discard or cancel. Genuine same-file
schema persistence, the table-name prompt, record entry and complete Design view
remain unfinished. Fresh-file constructors for ID-only and ordinary-companion
AutoNumber tables do not supply mutation of an existing blank database.

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
Native Access comparison remains pending.

The New form is a headless presentation adaptation of the retained Backstage
layout and documented Microsoft workflow. No native Access New-form capture or
cross-platform visual/interaction parity is claimed. See
[visual references](VISUAL_REFERENCES.md).
