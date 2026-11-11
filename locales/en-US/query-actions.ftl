# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = Update query
query-action-update-reviewing = Reviewing the update query…
query-action-table = Table: { $table }
query-action-matched = Records matched: { $count }
query-action-update-changed = Records with effective changes: { $count }
query-action-update-confirmation = Apply these reviewed changes to the original database?
query-action-no-undo = Undo is not available for this operation.
query-action-yes = Yes
query-action-no = No
query-action-invalid-review = The action review no longer matches this SQL draft.
query-action-delete-title = Delete query
query-action-delete-reviewing = Reviewing the delete query…
query-action-delete-deleted = Records to delete: { $count }
query-action-delete-confirmation = Permanently delete these reviewed records from the original database?
query-action-insert-title = Append query
query-action-insert-reviewing = Reviewing the append query…
query-action-insert-inserted = Records to append: { $count }
query-action-insert-confirmation = Append the reviewed records to the original database?

## Reviewed SQL schema changes
query-action-schema-title = Data-definition query
query-action-schema-reviewing = Reviewing the schema query…
query-action-schema-confirmation = Apply this reviewed database structure change?
query-action-schema-create-fields = Fields to create: { $count }
query-action-schema-drop-records = Records in the table to delete: { $count }
query-action-schema-drop-field = Field to delete: { $column }
query-action-schema-create-confirmation = Create this reviewed table in the original database?
query-action-schema-drop-table-confirmation = Permanently delete this reviewed table and its records from the original database?
query-action-schema-drop-column-confirmation = Permanently delete this reviewed field and its values from the original database?
query-action-schema-close-target = Close the target table before running this schema query.
query-action-schema-reopen-failed = The schema change was saved, but the database could not be reopened. Do not repeat the query.
query-action-schema-retry-reopen = Retry reopening
query-action-schema-retained-results = Previous results are retained. Reopen the saved database to refresh its structure; do not repeat the query.

query-action-schema-make-table-confirmation = Create this table and insert all selected records as one operation?
query-action-schema-make-table-records = Selected records to insert: { $count }
