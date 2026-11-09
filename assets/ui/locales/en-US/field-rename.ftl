# Saved-field names: source checks, reviewed changes and verified reopening.

field-rename-title = Rename Field
field-rename-name = Field name
field-rename-design-help = Use { field-rename-title } to propose a field name. Other field properties remain read-only.
field-rename-original = Original field: { $column }
field-rename-proposed = Proposed name: { $column }
field-rename-inspecting = Checking whether this field can be renamed…
field-rename-inspection-interrupted = Field check was interrupted. Retry to check this field again.
field-rename-retry-inspection = Retry field check
field-rename-source-unavailable = Renaming this field is not available for this database.
field-rename-profile =
    { $units ->
        [one] Current support requires an ASCII field name of exactly { $units } character.
       *[other] Current support requires an ASCII field name of exactly { $units } characters.
    }
field-rename-reviewing = Reviewing the field name…
field-rename-reopening = Reopening and verifying the renamed field…
field-rename-unchanged-reopening = Checking the unchanged database…
field-rename-unchanged = No field name change was written.
field-rename-cancel = Cancel rename
field-rename-return = Return to name editor
field-rename-close-title = Field name changes
field-rename-close-question = Save changes to the name of { $column } in { $table } before closing?
field-rename-close-save = Save
field-rename-close-discard = Discard
field-rename-close-cancel = Cancel
field-rename-close-unavailable = Saving is not available for this proposal. Discard it or return to the name editor.
field-rename-close-stale = This proposal changed. Cancel and try closing again.
field-rename-close-confirm = Close the name editor? This does not undo any change already written to the file.
field-rename-warning-saved = The field name was written, but the database could not be verified. Reopen the database before making further changes.
field-rename-warning-unchanged = No field name change was written, but the database could not be verified. Reopen the database before making further changes.
field-rename-close-editor = Close name editor
