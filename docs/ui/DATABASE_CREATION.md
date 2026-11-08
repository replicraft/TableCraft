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

| Gallery choice | Initial native declaration | Initial type default |
| --- | --- | --- |
| Short Text (255) | Short Text, 255 UTF-16 units | Absent |
| Number | Long Integer initially; selectable Field Size below | Typed numeric zero; Replication ID has no default |
| Large Number | Signed 64-bit integer | `0` |
| Currency | Fixed-point Currency | `0.0000` |
| Date & Time | Legacy Date/Time | Absent; no implicit current date/time |
| Yes/No | Boolean | `False` |

These are the initial Type default choices before any Design property edits.
The default selected for Save is persisted metadata retained in the reviewed
declaration; it is not a placeholder row value or display formatting. Choosing a
field never inserts a record. After saving, generated-ID insertion can resolve
omitted companions using checked stored defaults, or NULL for a genuinely absent
default in an eligible nullable companion. Original field presence remains exact
for fresh proof and recovery. Supplied values take precedence, including explicit
NULL; an unsupported or malformed stored default never falls back to NULL. All
existing source, carrier, Required and index checks still apply. The initial ID
keeps Increment generation and its primary key; it has no explicit constant
default. Number Field Size changes select an explicit carrier rather than
inferring one from input. Date/Time Extended remains unavailable.
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
Save capability and no conflicting draft, modal or operation.
Selecting the command changes only local draft state. It neither acquires a
writable target nor creates a journal. Review still qualifies the original source
and declarations; Save freshly rechecks the retained target before mutation.

A readable noncanonical blank remains unavailable for creation. This does not
restrict opening or reading it. The same command also supports the explicitly
qualified populated source below. Table Design remains disabled. Microsoft's
[Create a table and add fields](https://support.microsoft.com/en-us/access/create-a-table-and-add-fields)
documents Create > Table in an existing database, opening a new Datasheet. That
workflow differs from Create > Table Design, which opens a fresh Design view.
The retained Datasheet and Design captures guide those separate surfaces; they
do not capture activation of the Create command.

### Creating a second table

Create > Table is enabled for a qualified one-table ACE 16 source with the
supported catalog, allocation, row and primary-index layout. Qualification uses
actual immutable captured bytes, not a filename, header version, remembered
creator flag or the mere presence of one user table. See the
[append-table source profile](../access/writes/ADD_TABLE_PERSISTENCE.md) for the
native limits. Unqualified imported layouts, additional object/payload roles and
a third table remain unavailable; ordinary supported reads are unaffected.

The runtime publishes one table-creation availability fact for the opened source
revision. Shared UI accepts only a matching fact and retains the same Save,
busy, modal, draft and operation guards. Selecting Create changes presentation
state only: no native plan, target lease, journal or file write occurs. Review
selects the captured first-table or added-table profile internally. A caller does
not choose that profile, and a failed action is never retried through another
operation or decoder.

The new draft suggests the first unused `TableN` name from the already loaded
table and query names. For example, an existing `Table1` suggests `Table2`, while
an unrelated `Items` table can leave `Table1` available. This bounded ASCII name
scan is a presentation suggestion; it does not establish native collation
semantics, reserve the name or replace authoritative native admission. The tab
and Save prompt use the same proposed name, retained through locale/view changes
and parking. Entering an existing name can therefore refuse at review; the prompt
and exact raw field/record input remain available for correction, with source
bytes and journal namespace unchanged.

Successful schema Save preserves the old table and restores existing table tabs
and unsaved SQL text. Restored objects use the new document revision and fresh
reads; old row handles and cached native capabilities are not reused. A pending
record for the new table still follows the separate Review/Apply continuation
below. After the second schema is committed, the source fact disables another
Create > Table because the current append profile does not admit a third table.

A connected headless desktop scenario creates `Table1` with a record, opens an
unsaved SQL query, creates `Table2`, refuses a duplicate `Table1` name without a
write, corrects it and separately saves the retained first record. Fresh reads
retain the original row and new row; old source/row bindings refuse reuse and
both table tabs and SQL text survive. The affected local batch passes all 427
shared UI, 111 desktop and 149 runtime tests. Six-target cross-checks compile these
affected packages; native Windows/Linux execution and Access comparison remain
unqualified. These current observations are separate from the historical counts
elsewhere in this document.

The focused desktop follow-up also runs actual SQL queries after dropping the
original host and reopening the file. Its SQL editor reads the old value `17`
from `Table1` and the exact inserted text `  Ada  ` from `Table2`, independently
of the previous host's display caches.

Fresh Table Design has its own key-selection workflow. Microsoft's
[Table Designer procedure](https://support.microsoft.com/en-us/access/training/video-build-tables-with-the-table-designer)
starts with a field the user defines, then an explicit Primary Key choice. Saving
without a key asks whether to create one. The working Datasheet-to-Design switch
retains the ID already created by Datasheet; it does not implement that fresh
Design workflow. Enabling Create > Table Design requires the actual editable
key/no-key behavior rather than opening another protected-ID draft.

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
TableCraft initially selects Long Integer for its Number declaration. The
selected Field Size can be changed in Design view; this explicit profile does
not import Access application options.
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

## Initial-field header commands

Microsoft documents two rename entry points in Datasheet view: editing the
heading directly or choosing Rename Field from its shortcut menu. Deletion uses
Delete Field from that menu and an explicit confirmation; it also discards the
column's data. A primary-key field cannot be deleted through Datasheet view.
See [Add or delete a column in a datasheet](https://support.microsoft.com/en-us/access/add-or-delete-a-column-in-a-datasheet).
These are shortcut-menu commands, distinct from clicking a heading to select a
column or using its arrow to filter values. Microsoft documents those separate
surfaces in [Working with datasheets](https://support.microsoft.com/en-us/access/working-with-datasheets).
The [Access keyboard contract](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
provides Shift+F10 or the Windows Menu key for shortcut menus. The draft's field
commands use the header context menu rather than repurposing a sort/filter
chevron as a schema-command dropdown.
The current shared UI supports secondary click, focused Shift+F10 and the
accessibility context-menu action.

TableCraft applies these commands only to its initial, unsaved table draft.
Rename Field opens the existing header-name editor; validation, cancellation and
stable field identity keep the pending record input attached to the same field.
Delete Field asks for confirmation using the full current field name and warns
that the unsaved declaration and its pending record input will be discarded.
Cancellation keeps both. Confirmation removes that field from the draft rather
than issuing native DDL or changing a persisted table. The generated ID primary
key remains protected. Click to Add is a declaration placeholder, not a field
to rename or delete.

The deletion confirmation is a bounded initial-draft adaptation, with explicit
Delete field and Cancel actions. It is not a claim to reproduce Access's exact
confirmation wording or persisted schema-deletion behavior. An existing
TableCraft save, review or unresolved outcome must finish before these draft
commands become available; retained record input cannot bypass that boundary.

The retained 1920×1141 Windows/VirtualBox Datasheet image was reopened for this
slice; its display scale is unknown. It shows field headings and Click to Add,
but no open header shortcut menu or deletion confirmation. Those surfaces follow
the documented interaction and TableCraft's explicit draft boundary rather than
a captured-menu pixel match. Native Windows/Access interaction remains unverified.

## Initial-table Design view

The retained Design capture, `Bildschirmfoto_2026-10-28_12-07-50.png`, was
reopened for this slice: 1920×1165 pixels inside Windows/VirtualBox, with unknown
display scale. Its upper field grid has a narrow selector/key gutter and Field
Name, Data Type and Description (Optional) columns. A separate lower Field
Properties pane contains General and Lookup tabs, property/value rows and a help
area. The contextual Table Design Ribbon remains separate from the grid.
These captured surfaces guide the draft editor's layout; they do not establish
native editing or exact interaction parity.

Microsoft's [new-table Design workflow](https://support.microsoft.com/en-us/access/create-a-new-database)
enters names and types in the upper grid and uses the lower pane for the selected
field's properties. Its [type-change contract](https://support.microsoft.com/en-us/access/modify-or-change-the-data-type-setting-for-a-field)
selects a new Data Type and then saves the table. Persisted-field conversion can
truncate or remove data or be refused. TableCraft does not implement those
persisted conversion semantics through this draft surface.

The editable Design view belongs to the same initial, unsaved table as Datasheet
view. Ordinary field names and the six supported declaration types can be
changed there; the initial AutoNumber primary key remains protected. The lower
General pane edits Field Size, Required, Allow Zero Length and Default Value,
plus Precision and Scale for a Number field whose size is Decimal. Description, Caption, validation
rules/messages and Lookup authoring remain unavailable. The pane describes the
current draft declaration, not an observation of already stored metadata.
Existing persisted-table Design view remains read only.

Changing a draft field's type keeps its field identity and all pending record
input exactly as entered, including Omitted/NULL modes and text hidden by those
modes. The Type default mode follows the newly selected type; other explicit
default choices keep their original raw input. Short Text capacity and Allow
Zero Length, Number size and Decimal constraints retain their hidden choices
while another type applies, becoming effective again when their type is restored. None of these edits converts, clears or invents a record
value. Returning to Datasheet view allows correction. Invalid or noncanonical Yes/No text remains editable as text
instead of silently appearing as False; an incompatible explicit NULL is retained
rather than changed to a value. The eventual row review still requires valid
input for the saved declaration. Schema saving and record review remain separate,
so a failed record preparation does not recreate the schema or discard its input.

Switching views does not write the database. Save uses the same immutable
initial-schema request and reviewed runtime save route as Datasheet view. It does
not introduce native ALTER TABLE, persisted schema conversion, additional field
types or an alternative record-save path. Pending schema saves and unresolved
outcomes keep their existing editing guards in both views.

### Number Field Size and Decimal declarations

Number is one authoring type with seven explicit Field Size choices, following
Microsoft's [Set the field size](https://support.microsoft.com/en-us/access/set-the-field-size)
and [field-property reference](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties).
Byte uses an unsigned 8-bit carrier, Integer signed 16-bit, Long Integer signed
32-bit, Single binary32, Double binary64, Decimal its declared exact precision
and scale, and Replication ID a stored GUID. Large Number remains a separate
signed 64-bit declaration. Choosing Replication ID does not enable replication,
GUID generation or a GUID AutoNumber; ACCDB replication remains unsupported.
The retained Design image shows Long Integer for the selected AutoNumber ID,
not an expanded Number selector or Decimal properties. Those controls follow
the documented workflow rather than a captured-menu match.

The Decimal profile begins at precision 18 and scale 0, following Microsoft's
[Access SQL data types](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types).
The [Number/Currency field article](https://support.microsoft.com/en-us/access/create-or-delete-a-number-or-currency-field)
currently reverses those defaults to precision 0 and scale 18. TableCraft uses
the SQL reference's consistent 1–28 precision and 0–precision scale contract;
the conflicting article is not an oracle for its initialization. Native Access
comparison of this authoring workflow remains pending.

A Decimal input is parsed exactly and rescaled only without changing its numeric
value. For a scale-2 field, `1.2` becomes `1.20`, and `1.2300` becomes `1.23`;
`1.234` is refused rather than rounded. The coefficient must also fit the declared
precision. This applies to typed defaults and the declared input path for record
insertion/editing. Raw input stays available for correction. The independent
Decimal Places display property does not set stored scale or round an edit.
See Microsoft's [number-format contract](https://support.microsoft.com/en-us/access/format-a-number-or-currency-field).

Numeric Type default selects exact zero in the chosen carrier, including Decimal
zero at its declared scale; Replication ID selects no default. Finite Single and
Double defaults use bounded scientific spellings with exact bit reparse, retaining
signed zero. NaN and infinities are refused. Microsoft's
[expression syntax](https://support.microsoft.com/en-us/access/guide-to-expression-syntax)
documents signed decimal and E/e scientific values; those grammar rules do not
establish native float-bit or signed-zero parity. GUID fields accept supplied
values, NULL and qualified omission, but non-NULL GUID constant defaults remain
outside the native default-authoring profile. No random or replication provider
is activated by selecting a Number size.

The [archived Microsoft KB 170117](https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/170/117.HTM)
documents Access SQL GUID constants using `{guid {xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}}`.
That is a historical Microsoft-authored SQL reference, not an ACE 16 stored
DefaultValue comparison. It does not qualify GUID generation or expand the current
non-NULL GUID default boundary.

### Supported field properties

| Property | Current initial-table behavior |
| --- | --- |
| Field Size | Short Text accepts 1–255 UTF-16 code units, initially 255. Number selects Byte, Integer, Long Integer, Single, Double, Decimal or Replication ID, initially Long Integer. |
| Precision / Scale | Decimal only: precision 1–28 and scale 0–precision, initially 18 / 0. These are stored constraints, separate from display Decimal Places. |
| Required | No or Yes. New ordinary fields start with explicit No. |
| Allow Zero Length | Short Text only: Yes or No. New Short Text starts with explicit Yes. |
| Default Value | Type default, No default, NULL or a typed literal constant. The generated ID has none of these editable properties. |

Microsoft documents [Required's default No and AutoNumber exclusion](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/required-property-access).
Its [AllowZeroLength contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/allowzerolength-property)
distinguishes the user-interface default Yes from the programmatic default No.
TableCraft's authoring defaults follow those documented UI choices. Its lower-level
native API still preserves omitted properties separately from explicit Boolean
declarations; the UI offers Access's Yes/No choices. These declarations do not
establish native Access save/reopen
or interaction parity.

Required and Allow Zero Length remain separate: an empty text value is not NULL.
They do not change the pending record's Omitted/NULL/Value mode or generate input
while the schema is being edited. Invalid Field Size text stays editable and
blocks Save until corrected. Native review still checks complete declarations;
choosing a property value does not bypass type, capacity or insertion constraints.

Type default uses the selected type and Number size's defaults described above.
No default removes that declaration; NULL requests a typed NULL default rather than the
text `NULL`. Literal input goes through the runtime's exact scalar-input parser
for the selected field type. Enter text without wrapping SQL quotes, integers
or Currency with invariant numeric syntax, lowercase `true`/`false`, or a legacy
Date/Time such as `2026-11-07T12:30:00`. Text `Now()` is stored as text; a Date/Time
literal `Now()` is refused. No SQL, VBA, formula, clock or name lookup is executed.
NULL remains subject to the native carrier rules; Yes/No does not admit it.

For a qualified ordinary nullable companion of the generated ID, leaving record
input Omitted with No default now resolves to NULL during native review. Required
Yes rejects that resolved NULL; Allow Zero Length Yes does not replace it with an
empty string. Supplied empty text and explicit NULL remain distinct inputs, and
neither activates a stored default. A present but empty, malformed or unsupported
DefaultValue expression is not proof that the declaration is absent. An omitted
Yes/No field with no default remains refused in this bounded route; no False
value is invented. The untouched `(New)` placeholder still does not request a row.

Microsoft's [INSERT INTO contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql)
describes omitted-column default/NULL behavior. Its
[Required property contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/required-property-access)
keeps NULL restrictions separate from empty strings, and the
[Access glossary](https://support.microsoft.com/en-us/access/access-glossary)
explicitly excludes NULL from native Yes/No values. This implementation retains
the existing [normal Increment source profile](../access/writes/NORMAL_INCREMENTING_AUTONUMBER.md),
including its source, index, validation/data-macro and resource gates. It does not
qualify arbitrary imported fields or Boolean omission without a default.

The raw literal editor retains up to 4,096 UTF-8 bytes. This is an input-retention
limit, separate from the native constant-expression ceiling of 255 UTF-16 code
units. Stored text expressions include their added surrounding quotes and doubled
embedded quotes, so even a value that fits the field's data capacity may exceed
the default-expression limit. Review refuses such declarations before a journal
or write; it does not truncate them. Literal/type mismatches are also reported at
review, with the draft retained for correction. The default buffer stays associated
with its field when switching modes or types; another field cannot inherit it.

## Saving the initial table

The shared Save prompt accepts an exact table name and submits
the current immutable draft snapshot. Its supported schema is the initial ID
AutoNumber primary key plus the explicit field declarations and defaults listed
above, within the sixteen-field ceiling. No records are authored by this operation.
The UI keeps the draft distinct from the persisted database while review and
saving run; neither accepting the name nor closing the prompt proves a save.

Runtime qualifies the immutable source and declarations using the captured
first-table or added-table profile, then issues a consuming review. Its existing
nonreplaceable SaveWorker acquires the target, compares all original bytes and
rederives the native operation under that same retained descriptor. First-table saves use a distinct TCNT schema
intent. Added-table saves use the separate TCAT action with an original full-source
binding and a closed declaration template. Each uses its checked V2 journal to
resize the same original file; neither replaces the path with a freshly created
image. Unix finalization acknowledges retirement/removal barriers.
Windows acknowledges its write-through retirement rename and retains the intact
retired artifact; the completion report distinguishes those namespace facts from
native commitment and does not claim durable Windows deletion. See
[first-table persistence](../access/writes/FIRST_TABLE_PERSISTENCE.md) and
[second-table persistence](../access/writes/ADD_TABLE_PERSISTENCE.md).

Definitions with a default on Byte, Integer, Single, Double, Decimal or GUID
use closed `TCNT0004` intents, including an explicit NULL default on those target
families. Otherwise explicit Required or Allow Zero Length policies select
`TCNT0003`, retaining presence independently from true/false. Policy-free
legacy-family default-bearing definitions retain `TCNT0002`; policy-free
definitions without defaults retain `TCNT0001`. These existing encodings and
recovery routes remain distinct, without newly inferred defaults, properties or
wider default carriers during replay. Fresh recovery reconstructs the captured declarations
before qualifying the original and replacement images. No intent permits executing
an arbitrary default expression.

Only an acknowledged committed/verified-replacement receipt with the matching
schema action and column count starts guarded reopening. Reopening compares the complete captured
file against the expected content retained during review before native reading.
The host matches the reviewed expected schema action as well as the original
document, draft revision and table identity.
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
and checks the real field names, order and declared carriers. An untouched
placeholder never inserts an empty record. Original Omitted/NULL/supplied intent and hidden raw text are retained;
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

### Retaining input after a row-save failure

Acknowledging a failed or cancelled insertion retains the original input and its
Omitted/NULL/value choices in a copyable view. Retaining that input does not
repeat a write or assert that the original file was restored. The schema's known
completion remains separate; discarding retained row input never deletes the
table. A terminal or unknown row outcome uses a neutral saved-schema notice;
it does not claim that the row is unsaved when commitment is uncertain.

The retained view renders only the first 512 Unicode scalar values per field,
with an explicit shortened-preview notice. Copy complete input copies that
field's entire original raw text, including text hidden under Omitted or NULL.
The bounded preview does not truncate the retained backing input or shape up to
eight MiB into the read-only editor.

Reopen original and edit is enabled only by typed runtime evidence that the
operation stopped before target writes, or that the original image was rolled
back or verified. An observed committed/replacement outcome blocks this retry;
unknown outcomes default to blocked. Releasing resources alone does not grant
retry permission. The host must complete a guarded read against the captured
original-content witness, then obtain and validate a fresh insertion draft
before restoring the input. A failed or cancelled reopen retains the copyable
input. Successful reopening still requires a new explicit Review and Apply;
it never replays the previous operation automatically.

This is input retention with a narrowly admitted return to editing, not general
native Undo, reversal of a committed insertion or a promise of restored physical
file identity.

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
