# TableCraft UI: editing.

editing-title = Offline cell edit
editing-exclusive-help = Exclusive offline editing. Close this file in Access and other applications before applying.
editing-checking = Checking this cell and reading its original value…
editing-original = Original stored value
editing-replacement = Replacement
editing-use-null = Use NULL (subject to validation)
editing-validating = Validating the complete edit…
editing-reviewed = Reviewed change
editing-before = Before
editing-after = After
editing-changed-rows = Changed rows: { $count }
editing-apply = Apply to original file
editing-review = Review change
editing-resolving = Resolving retained save resources. Keep this window open until its outcome is known.
editing-applying = Applying the reviewed edit. Keep this window open until its outcome is known.
editing-cancellation-requested = Cancellation requested; waiting for the actual save outcome.
editing-request-cancellation = Request cancellation
editing-recovery-owned = The save session still owns recovery resources. Resolve it before continuing.
editing-recover = Recover save
editing-release = Release save resources
editing-diagnosis-required = Diagnosis is required; automatic recovery and release are unavailable.
editing-target = { $table } · { $column } · row { $row }
editing-stored-value = Stored value

editing-mock-rejected = Mock validation rejected this sample value. No database was opened or changed.

editing-mock-failed = Mock save failed. No database was opened or changed.

editing-mock-diagnosis = Mock outcome requires diagnosis. No database file or journal exists in this preview.

editing-outcome-saved = Save completed

editing-outcome-cancelled = Save cancelled

editing-outcome-failed = Save needs attention

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = committed
editing-save-known-rolled-back = rolled back
editing-save-known-original = original image verified
editing-save-known-replacement = replacement image verified
editing-save-known-unknown = not established
editing-save-completion-applied = applied
editing-save-completion-recovered = recovered
editing-save-completion-confirmed = confirmed
editing-save-completed = Save completed and lock release was acknowledged.
editing-save-released = Resources released. Recovery files may remain; resolve them before using this file. { $details }
editing-save-current = Current operation outcome: { $outcome }.
editing-save-no-receipt = No completed-operation receipt is available.
editing-save-prior = Last completed operation: { $completion }; outcome: { $outcome }.
editing-save-matched = Matched rows: { $count }.
editing-save-changed = Changed rows: { $count }.
editing-save-failure = Save did not finish normally ({ $reason }). { $details } Do not retry automatically.
editing-save-reason-stale = the viewed document was replaced
editing-save-reason-open = the file could not be opened for exclusive editing
editing-save-reason-source = the file changed after it was viewed
editing-save-reason-cancelled = cancellation was requested
editing-save-reason-resource = an operation limit was reached
editing-save-reason-planning = the edit could not be validated
editing-save-reason-intent = the change record could not be prepared
editing-save-reason-setup = the recovery files could not be set up
editing-save-reason-prepare = the save could not be prepared
editing-save-reason-commit = the commit did not finish normally
editing-save-reason-recovery = recovery did not finish normally
editing-save-reason-finalization = save finalization did not finish normally
editing-save-reason-release = lock release was not acknowledged
editing-save-reason-state = the save session requires diagnosis

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = Boolean: true or false
editing-input-byte = Byte: 0 to 255
editing-input-int16 = Signed 16-bit integer
editing-input-int32 = Signed 32-bit integer
editing-input-int64 = Signed 64-bit integer
editing-input-single = Single: decimal number Use "." as the decimal separator.
editing-input-double = Double: decimal number Use "." as the decimal separator.
editing-input-currency = Currency: decimal amount (four decimal places) Use "." as the decimal separator.
editing-input-decimal = Decimal number Use "." as the decimal separator.
editing-input-text = Literal text
editing-input-binary = Binary: hexadecimal byte pairs
editing-input-guid = GUID: canonical identifier
editing-input-date = Legacy DATE: ISO civil date-time (no time zone)
editing-input-date-extended = Extended date-time: ISO civil date-time (100 ns precision)

editing-paste-text = Paste text
editing-paste-help = Paste literal text into one selected native cell, then review it. Use the field’s invariant input format; formatted values and typed Access clipboard formats are not interpreted. Multiline text stays in one cell.
editing-paste-too-large = Clipboard text exceeds the 1 MiB limit. Nothing was pasted.
editing-paste-stale = The clipboard request no longer matches the selected cell. The text was discarded.
editing-paste-multiple = The clipboard returned multiple text payloads. Nothing was pasted.
