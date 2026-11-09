# TableCraft UI: datasheet.

datasheet-column-count = Columns: { $count }
datasheet-previous-page = ‹ Previous page
datasheet-next-page = Next page ›
datasheet-row-range = Rows { $first }–{ $last }
datasheet-no-rows = No rows.
datasheet-field-name = Field name: { $name }
datasheet-select-record = Select record { $number }
datasheet-copy-record = Copy displayed record
datasheet-record-selected-help = Record { $number } is selected. Copy uses all displayed fields, including collection summaries; it does not copy child payloads.
datasheet-select-record-help = Select record { $number }. Copy uses all displayed fields, including collection summaries; it does not copy child payloads.
datasheet-view-contents = View contents
datasheet-edit-cell = Edit cell…
datasheet-cell-context = { $column }, row { $row }: { $text }
datasheet-cell-details = { $column }, row { $row }: { $text }; { $context }
datasheet-copy-cell-help = Copy copies the full cell value.
datasheet-copy-incomplete = The selected record does not contain every displayed field.
datasheet-copy-limit = The displayed record exceeds the 1 MiB clipboard limit.
datasheet-copy-allocation = The clipboard buffer could not be allocated.
datasheet-copy-encoding = The displayed record could not be encoded for the clipboard.
datasheet-record = Record:
datasheet-first = First
datasheet-previous = Previous
datasheet-next = Next
datasheet-go-record = Go to record
datasheet-record-input-help = Enter a whole record number from 1 to 4294967296.
datasheet-wrong-source = The returned page belongs to another source.
datasheet-wrong-offset = The returned page starts at another record position.
datasheet-missing-column = The requested column is absent from the returned page.
datasheet-missing-record = There is no record at that position in this snapshot.
datasheet-incomplete-page = The returned page does not contain the requested record.
datasheet-badge-stored = stored
datasheet-badge-null = NULL
datasheet-badge-list = list
datasheet-badge-encoded = encoded
datasheet-badge-history = history
datasheet-null-context = NULL value
datasheet-stored-context = Stored calculated result; no formula evaluated; freshness unknown
datasheet-stored-null-context = Stored NULL calculated result; no formula evaluated; freshness unknown
datasheet-control-uninspected = Field display settings are not inspected; showing the stored value
datasheet-control-absent = No display control is declared; showing the stored value
datasheet-control-unsupported = Field display settings are unsupported; showing the stored value
datasheet-control-textbox = This field uses a text box
datasheet-control-lookup = Lookup display is not supported yet; showing the stored value
datasheet-context-pair = { $base }; { $presentation }

datasheet-go = Go

datasheet-null-value = NULL
datasheet-close-table = Close table
datasheet-column-menu = Column menu { $column }

datasheet-last = Last

datasheet-value-preview-truncated = Preview shortened. Copy preserves the complete displayed value.

datasheet-row-identity-invalid = The row identities do not match this page.
datasheet-filter-page-unavailable = Only the loaded filtered records are available here.

datasheet-click-to-add = Click to Add

datasheet-add-field-unavailable = Adding fields is not available yet.

datasheet-record-position = Record number

datasheet-record-of = of { $count }

datasheet-count-unknown = ?

datasheet-search-records = Search records

datasheet-no-filter = No Filter

datasheet-filtered = Filtered
datasheet-page-tools = Page tools

# Saved-table Number field review and checked reopen.
datasheet-number-inspecting = Checking whether a Number field can be added…
datasheet-number-reviewing = Reviewing the new Number field…
datasheet-number-reopening = Reopening the saved table…
datasheet-number-retry-inspection = Retry field eligibility check

# Saved Number field close confirmation
datasheet-number-close-title = Save field changes?
datasheet-number-close-question = Save the new field “{ $column }” in table “{ $table }”?
datasheet-number-close-stale = The field name or table changed. Close this prompt and try again.
datasheet-number-close-unavailable = This field cannot be saved right now.
datasheet-number-close-save = Save
datasheet-number-close-discard = Don't Save
datasheet-number-close-cancel = Cancel

# Interrupted readonly field eligibility observation.
datasheet-number-inspection-interrupted = Field check interrupted. Retry to check this table again.

# Known saved field after failed checked reopening.
datasheet-number-saved-warning = The field was saved, but the database could not be verified. Reopen the database before another save.
datasheet-number-abandon-title = Close field draft?
datasheet-number-abandon-question = The field was saved. Closing this draft does not undo that save.
datasheet-number-abandon-discard = Close draft

# Leave failed checked reopening without undoing an acknowledged field save.
datasheet-number-return-draft = Return to draft
