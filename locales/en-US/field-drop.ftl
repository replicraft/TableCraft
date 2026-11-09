# Saved-field deletion. These prompts are TableCraft adaptations; no retained Access dialog capture exists.

## Confirmation

field-drop-title = Delete Field
field-drop-confirm = Do you want to delete the field “{ $column }”?
field-drop-data-loss = The field and its data will be permanently removed when you save the table.
field-drop-yes = Yes
field-drop-no = No

## Source inspection

field-drop-inspecting = Checking whether this field can be deleted…
field-drop-source-unavailable = This field cannot be deleted with the current editing profile.
field-drop-inspection-interrupted = Field inspection was interrupted.
field-drop-retry-inspection = Check again

## Pending deletion

field-drop-pending = Pending deletion: { $column }
field-drop-pending-help = Save the table to delete this field. The current view changes only after the saved table has been verified.
field-drop-design-help = Select a field and choose Delete Rows to mark it for deletion; save the table to apply.

## Save and reopen

field-drop-reviewing = Checking the requested field deletion…
field-drop-reopening = Verifying the saved table before reopening it…
field-drop-return = Return to table
field-drop-warning-saved = The field deletion was saved, but the table could not be reopened. Reopen the database before editing.

## Closing and retained status

field-drop-close-title = Save Table
field-drop-close-question = Save the pending deletion of “{ $column }” in table “{ $table }”?
field-drop-close-save = Save
field-drop-close-discard = Discard
field-drop-close-cancel = Cancel
field-drop-close-stale = This confirmation no longer matches the current table.
field-drop-close-unavailable = The deletion cannot be saved in the current state.
field-drop-close-warning = Dismiss this deletion status? The saved deletion and any cleanup notice remain recorded.
field-drop-close-editor = Dismiss deletion status
