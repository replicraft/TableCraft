//! Only actual runtime-issued consuming capabilities enter the session registry.
use super::{
    Session, arguments as arg,
    save::{ActiveSave, Reopen},
};
use serde_json::{Value, json};
use std::path::Path;
use tablecraft_runtime::{
    ReviewedDatabaseOptions, ReviewedDelete, ReviewedEdit, ReviewedFieldAddition,
    ReviewedFieldDrop, ReviewedFieldRename, ReviewedInsert, ReviewedQueryDelete,
    ReviewedQueryExpression, ReviewedQueryInsert, ReviewedQuerySql, ReviewedQueryUpdate,
    ReviewedTableCreation, ReviewedTableDrop, ReviewedTableRename, journal_path,
};

pub(super) enum Reviewed {
    Edit(ReviewedEdit),
    Insert(ReviewedInsert),
    Delete(ReviewedDelete),
    TableRename(ReviewedTableRename),
    TableDrop(ReviewedTableDrop),
    TableCreation(ReviewedTableCreation),
    FieldAddition(ReviewedFieldAddition),
    FieldRename(ReviewedFieldRename),
    FieldDrop(ReviewedFieldDrop),
    QuerySql(ReviewedQuerySql),
    QueryExpression(ReviewedQueryExpression),
    QueryInsert(ReviewedQueryInsert),
    QueryDelete(ReviewedQueryDelete),
    QueryUpdate(ReviewedQueryUpdate),
    DatabaseOptions(ReviewedDatabaseOptions),
}

impl Reviewed {
    fn operation(&self) -> &'static str {
        match self {
            Self::Edit(_) => "apply_edit",
            Self::Insert(_) => "apply_insert",
            Self::Delete(_) => "apply_delete",
            Self::TableRename(_) => "apply_table_rename",
            Self::TableDrop(_) => "apply_table_drop",
            Self::TableCreation(_) => "apply_table_creation",
            Self::FieldAddition(_) => "apply_field_addition",
            Self::FieldRename(_) => "apply_field_rename",
            Self::FieldDrop(_) => "apply_field_drop",
            Self::QuerySql(_) => "apply_query_sql",
            Self::QueryExpression(_) => "apply_query_expression",
            Self::QueryInsert(_) => "apply_query_insert",
            Self::QueryDelete(_) => "apply_query_delete",
            Self::QueryUpdate(_) => "apply_query_update",
            Self::DatabaseOptions(_) => "apply_database_options",
        }
    }

    fn source_path(&self) -> &Path {
        match self {
            Self::Edit(capability) => capability.source_path(),
            Self::Insert(capability) => capability.source_path(),
            Self::Delete(capability) => capability.source_path(),
            Self::TableRename(capability) => capability.source_path(),
            Self::TableDrop(capability) => capability.source_path(),
            Self::TableCreation(capability) => capability.source_path(),
            Self::FieldAddition(capability) => capability.source_path(),
            Self::FieldRename(capability) => capability.source_path(),
            Self::FieldDrop(capability) => capability.source_path(),
            Self::QuerySql(capability) => capability.source_path(),
            Self::QueryExpression(capability) => capability.source_path(),
            Self::QueryInsert(capability) => capability.source_path(),
            Self::QueryDelete(capability) => capability.source_path(),
            Self::QueryUpdate(capability) => capability.source_path(),
            Self::DatabaseOptions(capability) => capability.source_path(),
        }
    }

    fn witness(&self) -> Option<Reopen> {
        match self {
            Self::TableRename(capability) => Some(Reopen::TableRename(capability.reopen_witness())),
            Self::TableDrop(capability) => Some(Reopen::TableDrop(capability.reopen_witness())),
            Self::TableCreation(capability) => {
                Some(Reopen::TableCreation(capability.reopen_witness()))
            }
            Self::FieldAddition(capability) => {
                Some(Reopen::FieldAddition(capability.reopen_witness()))
            }
            Self::FieldRename(capability) => Some(Reopen::FieldRename(capability.reopen_witness())),
            Self::FieldDrop(capability) => Some(Reopen::FieldDrop(capability.reopen_witness())),
            Self::QuerySql(capability) => {
                Some(Reopen::QueryExpression(capability.reopen_witness()))
            }
            Self::QueryExpression(capability) => {
                Some(Reopen::QueryExpression(capability.reopen_witness()))
            }
            Self::DatabaseOptions(capability) => {
                Some(Reopen::DatabaseOptions(capability.reopen_witness()))
            }
            _ => None,
        }
    }
}

impl Session {
    pub(super) fn retain_review(
        &mut self,
        capability: Reviewed,
        mut facts: Value,
    ) -> Result<Value, String> {
        let id = self.next_handle()?;
        facts["review_id"] = json!(id);
        facts["apply_operation"] = json!(capability.operation());
        self.reviewed = Some((id, capability));
        Ok(facts)
    }

    pub(super) fn apply_review(&mut self, operation: &str, args: &Value) -> Result<Value, String> {
        if self.active_save.is_some()
            || self.saves.is_busy()
            || self.creation.is_busy()
            || self.uncertain
        {
            return Err("A save, creation or uncertain native outcome blocks a new write".into());
        }
        let requested = arg::u64_value(args, "review_id")?;
        if !self
            .reviewed
            .as_ref()
            .is_some_and(|(id, capability)| *id == requested && capability.operation() == operation)
        {
            return Err("Unknown, consumed, superseded or wrong-operation review handle".into());
        }
        let Some((id, capability)) = self.reviewed.take() else {
            return Err("The reviewed capability is no longer owned".into());
        };
        let path = capability.source_path().to_path_buf();
        let witness = capability.witness();
        let insert_retry = match &capability {
            Reviewed::Insert(capability) => Some(capability.retry_witness()),
            _ => None,
        };
        let bundle = journal_path(&path);
        macro_rules! start {
            ($method:ident, $capability:expr, $variant:ident) => {
                match self.saves.$method($capability, bundle) {
                    Ok(save_id) => save_id,
                    Err(error) => {
                        let message = error.to_string();
                        self.reviewed = error
                            .into_reviewed()
                            .map(|capability| (id, Reviewed::$variant(capability)));
                        return Err(message);
                    }
                }
            };
        }
        let save_id = match capability {
            Reviewed::Edit(capability) => start!(start, capability, Edit),
            Reviewed::Insert(capability) => start!(start_insert, capability, Insert),
            Reviewed::Delete(capability) => start!(start_delete, capability, Delete),
            Reviewed::TableRename(capability) => {
                start!(start_table_rename, capability, TableRename)
            }
            Reviewed::TableDrop(capability) => start!(start_table_drop, capability, TableDrop),
            Reviewed::TableCreation(capability) => {
                start!(start_table_creation, capability, TableCreation)
            }
            Reviewed::FieldAddition(capability) => {
                start!(start_field_addition, capability, FieldAddition)
            }
            Reviewed::FieldRename(capability) => {
                start!(start_field_rename, capability, FieldRename)
            }
            Reviewed::FieldDrop(capability) => start!(start_field_drop, capability, FieldDrop),
            Reviewed::QuerySql(capability) => start!(start_query_sql, capability, QuerySql),
            Reviewed::QueryExpression(capability) => {
                start!(start_query_expression, capability, QueryExpression)
            }
            Reviewed::QueryInsert(capability) => {
                start!(start_query_insert, capability, QueryInsert)
            }
            Reviewed::QueryDelete(capability) => {
                start!(start_query_delete, capability, QueryDelete)
            }
            Reviewed::QueryUpdate(capability) => {
                start!(start_query_update, capability, QueryUpdate)
            }
            Reviewed::DatabaseOptions(capability) => {
                start!(start_disable_name_auto_correct, capability, DatabaseOptions)
            }
        };
        self.active_save = Some(ActiveSave {
            id: save_id,
            path,
            witness,
            acknowledged: false,
            report: None,
            restart: false,
        });
        self.insert_retry = insert_retry.map(|witness| super::retry::InsertRetry {
            save_id,
            witness,
            evidence: tablecraft_runtime::InsertRetryEvidence::default(),
        });
        self.query = None;
        self.find_state = super::auxiliary::FindState::default();
        self.view_state.retire_native();
        self.design_state = super::query_design::DesignState::default();
        Ok(json!({"save_id": save_id, "status": "pending"}))
    }
}
