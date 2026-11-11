//! Protocol requests compose the same typed commands as the desktop host.
mod table;

use super::{Session, arguments as arg};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tablecraft_runtime::{
    Command, ContentsTarget, DeleteTarget, EditTarget, InsertInput, InsertTarget,
};

impl Session {
    pub(super) fn command(&mut self, operation: &str, args: &Value) -> Result<Command, String> {
        if operation == "open_database" {
            let path = PathBuf::from(arg::text(args, "path")?);
            if !path.is_absolute() {
                return Err("path must be absolute".into());
            }
            self.reviewed = None;
            self.query = None;
            self.drop_source = None;
            return Ok(Command::Open {
                path,
                password: arg::optional(args, "password")
                    .map(|_| arg::owned(args, "password"))
                    .transpose()?
                    .unwrap_or_default(),
            });
        }
        let revision = self.current_revision(args)?;
        let command = match operation {
            "read_rows" => Command::Page {
                table: arg::owned(args, "table")?,
                offset: arg::u32_value(args, "offset")?,
            },
            "describe_table" => Command::Describe {
                table: arg::owned(args, "table")?,
            },
            "inspect_collection" => Command::InspectCollection {
                target: ContentsTarget {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                    column: arg::owned(args, "column")?,
                    parent_ordinal: arg::u32_value(args, "parent_ordinal")?,
                },
            },
            "inspect_query" => Command::InspectQuery {
                document_revision: revision,
                sql: arg::owned(args, "sql")?,
            },
            "run_query" => Command::RunQuery {
                target: self.query_handle(args)?,
                parameters: arg::parameters(args)?,
                options: crate::requests::read_query::options(args)?,
            },
            "review_query_insert" => Command::ReviewQueryInsert {
                target: self.query_handle(args)?,
                parameters: arg::parameters(args)?,
            },
            "review_query_delete" => Command::ReviewQueryDelete {
                target: self.query_handle(args)?,
                parameters: arg::parameters(args)?,
            },
            "review_query_update" => Command::ReviewQueryUpdate {
                target: self.query_handle(args)?,
                parameters: arg::parameters(args)?,
            },
            "inspect_saved_query" => Command::InspectSavedQuery {
                target: arg::saved(args, revision)?,
            },
            "run_saved_query" => Command::RunSavedQuery {
                target: arg::saved(args, revision)?,
                parameters: arg::parameters(args)?,
                options: crate::requests::read_query::options(args)?,
            },
            "begin_edit" => Command::BeginEdit {
                target: EditTarget {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                    column: arg::owned(args, "column")?,
                    parent_ordinal: arg::u32_value(args, "parent_ordinal")?,
                },
            },
            "review_edit" => Command::ReviewEdit {
                draft_id: arg::u64_value(args, "draft_id")?,
                input: arg::input(args)?,
            },
            "begin_insert" => Command::BeginInsert {
                target: InsertTarget {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                },
            },
            "review_insert" => {
                let inputs = args
                    .get("inputs")
                    .and_then(Value::as_array)
                    .ok_or("inputs must be an array in native draft-field order")?
                    .iter()
                    .map(|input| {
                        if arg::boolean(input, "omitted", false)? {
                            Ok(InsertInput::Omitted)
                        } else {
                            Ok(InsertInput::Value(arg::input(input)?))
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Command::ReviewInsert {
                    draft_id: arg::u64_value(args, "draft_id")?,
                    inputs,
                }
            }
            "review_delete" => Command::ReviewDelete {
                target: DeleteTarget {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                    parent_ordinal: arg::u32_value(args, "parent_ordinal")?,
                },
            },
            "review_table_rename" => Command::ReviewTableRename {
                document_revision: revision,
                table: arg::owned(args, "table")?,
                new_name: arg::owned(args, "new_name")?,
            },
            "inspect_table_drop" => Command::InspectTableDropSource {
                document_revision: revision,
                table: arg::owned(args, "table")?,
            },
            "review_table_drop" => {
                let table = arg::text(args, "table")?;
                if !self.drop_source.as_ref().is_some_and(|source| {
                    source.document_revision == revision
                        && source.table == table
                        && source.source_eligible
                }) {
                    return Err(
                        "Inspect and admit this exact table before destructive review".into(),
                    );
                }
                Command::ReviewTableDrop {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                }
            }
            "inspect_field_addition" => Command::InspectFieldAdditionAvailability {
                document_revision: revision,
                table: arg::owned(args, "table")?,
            },
            "review_field_addition" => {
                let declaration = crate::requests::field_addition::declaration(args)?;
                Command::ReviewAddField {
                    document_revision: revision,
                    table: arg::owned(args, "table")?,
                    column: arg::owned(args, "column")?,
                    declaration,
                }
            }
            "inspect_field_rename" => Command::InspectFieldRenameSource {
                document_revision: revision,
                table: arg::owned(args, "table")?,
                column: arg::owned(args, "column")?,
                native_number: arg::usize_value(args, "native_number")?,
            },
            "review_field_rename" => Command::ReviewFieldRename {
                document_revision: revision,
                table: arg::owned(args, "table")?,
                column: arg::owned(args, "column")?,
                native_number: arg::usize_value(args, "native_number")?,
                new_name: arg::owned(args, "new_name")?,
            },
            "inspect_field_drop" => Command::InspectFieldDropSource {
                document_revision: revision,
                table: arg::owned(args, "table")?,
                column: arg::owned(args, "column")?,
                native_number: arg::usize_value(args, "native_number")?,
            },
            "review_field_drop" => Command::ReviewFieldDrop {
                document_revision: revision,
                table: arg::owned(args, "table")?,
                column: arg::owned(args, "column")?,
                native_number: arg::usize_value(args, "native_number")?,
            },
            "inspect_database_options" => Command::InspectDatabaseOptions {
                document_revision: revision,
            },
            "review_disable_name_auto_correct" => Command::ReviewDisableNameAutoCorrect {
                document_revision: revision,
            },
            "inspect_query_sql" => Command::InspectQuerySqlDraft {
                target: arg::saved(args, revision)?,
            },
            "review_query_sql" => Command::ReviewQuerySql {
                target: arg::saved(args, revision)?,
                original_sql: arg::owned(args, "original_sql")?,
                proposed_sql: arg::owned(args, "proposed_sql")?,
            },
            "review_query_expression" => Command::ReviewQueryLiteralExpression {
                target: arg::saved(args, revision)?,
                expected: arg::owned(args, "expected")?,
                replacement: arg::owned(args, "replacement")?,
            },
            "review_table_creation" => Command::ReviewTableCreation {
                document_revision: revision,
                snapshot: table::snapshot(args)?,
            },
            _ => {
                return Err(format!(
                    "Unknown or unavailable session operation: {operation}"
                ));
            }
        };
        Ok(command)
    }

    fn query_handle(
        &self,
        args: &Value,
    ) -> Result<Arc<tablecraft_runtime::InspectedQuery>, String> {
        let requested = arg::u64_value(args, "query_id")?;
        let Some((id, query)) = self.query.as_ref().filter(|(id, query)| {
            *id == requested && Some(query.document_revision()) == self.revision
        }) else {
            return Err("Unknown, superseded or stale query handle".into());
        };
        let _ = id;
        // NO_CLONE: The retained immutable inspected query remains available to later parameter
        // runs while this runtime command independently owns a shared thread-safe reference.
        Ok(Arc::clone(query))
    }
}
