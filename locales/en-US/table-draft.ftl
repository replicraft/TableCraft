# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = The unsaved table is no longer available.
table-draft-new-record = (New)
table-draft-short-text = Short Text (255)
table-draft-field-name = Field name
table-draft-invalid-name = Choose a unique valid field name.
table-draft-empty-note = Save this table to continue.
table-draft-save = Save
table-draft-save-unavailable = Saving this table is currently unavailable. Keep the draft open or discard it.
table-draft-close-title = Unsaved table
table-draft-close-question = Save changes to the unsaved table?
table-draft-discard = Discard
table-draft-cancel = Cancel

table-draft-save-as = Save As
table-draft-table-name = Table name
table-draft-invalid-table-name = Choose a valid table name (maximum 64 characters).
table-draft-ok = OK
table-draft-save-cancelled = The save was cancelled. Your table draft is unchanged.
table-draft-save-failed = The table could not be saved.
table-draft-save-retained = The save outcome needs recovery or verification. Keep this window open.
table-draft-save-working = Saving table…
table-draft-save-cancelling = Cancelling the save…
table-draft-save-stale = The save no longer belongs to this table draft.
table-draft-save-blocked = Saving remains blocked after an unresolved save. Reopen the database to inspect it, or discard this draft.
table-draft-save-reopen-failed = The table was saved, but reopening could not be completed. Reopen the database before saving this draft again.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = Large Number
table-draft-currency = Currency
table-draft-date-time = Date & Time
table-draft-yes-no = Yes/No
table-draft-long-text = Long Text
table-draft-lookup-relationship = Lookup & Relationship
table-draft-rich-text = Rich Text
table-draft-attachment = Attachment
table-draft-hyperlink = Hyperlink
table-draft-calculated-field = Calculated Field
table-draft-more-fields = More Fields
table-draft-record-field = New record { $field }
table-draft-record-schema-saved = The table was saved; this record has not been saved.

table-record-preparing = The table was saved. Preparing your new record…
table-record-retry = Retry new record
table-record-discard = Discard new record
table-record-invalid = The table was saved, but the new record could not be prepared. Your input is retained.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = Field options for { $field }
table-draft-rename-field = Rename Field
table-draft-delete-field = Delete Field
table-draft-delete-field-title = Delete unsaved field
table-draft-delete-field-question = Delete the unsaved field “{ $field }”?
table-draft-delete-field-warning = This removes the field and its pending record input from this unsaved table. The database has not been changed.
table-draft-delete-field-confirm = Delete field

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = Edit names, types and supported field properties in this unsaved table. Other properties are not available yet.
table-draft-design-name = Field name for { $field }
table-draft-design-type = Data type for { $field }
table-draft-design-input-help = Pending record input stays as entered when the type changes. It must be valid for the selected type before Review and Apply.
table-draft-design-protected = The initial AutoNumber primary key cannot be changed.
table-draft-design-add-field = Add field

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = Not set
table-draft-property-no = No
table-draft-property-yes = Yes
table-draft-property-type-default = Type default
table-draft-property-absent = No default
table-draft-property-null = NULL
table-draft-property-literal = Literal
table-draft-property-literal-input = Literal default
table-draft-property-invalid-size = Enter a whole number from 1 to 255.
table-draft-property-input-limit = Property input exceeds its limit. The previous text is retained.
table-draft-property-invalid = Correct invalid field-property input before saving.
table-draft-property-help = Constants only: enter text without SQL quotes, numbers with a decimal point, true/false, or an ISO date and time. No formulas are evaluated. Input allows 4096 UTF-8 bytes; a saved default must fit 255 UTF-16 units, including added and escaped quotes.

table-draft-number = Number
table-draft-number-byte = Byte
table-draft-number-integer = Integer
table-draft-number-single = Single
table-draft-number-double = Double
table-draft-number-decimal = Decimal
table-draft-number-replication-id = Replication ID
table-draft-number-precision = Precision
table-draft-number-scale = Scale
table-draft-number-invalid-decimal = Precision must be 1–28 and scale must be 0–precision. Correct both values before saving.
table-draft-number-guid-default = Literal defaults for Replication ID are not available yet. Choose another default mode; the entered text is retained.

table-draft-design-description = Description for { $field }
table-draft-description-invalid = The field description is too long.

table-draft-no-key-title = No primary key
table-draft-no-key-question = No primary key is defined. Create an AutoNumber primary key?
table-draft-no-key-failed = The primary key could not be added. Keep the current design or free a field slot.

table-draft-design-new-name = New field name
table-draft-design-new-type = New field data type

table-draft-field-limit = Access tables support up to 255 fields, including the primary key.
