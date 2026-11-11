//! Resolve the recorded read owner on actual execution or issuance failure.
use super::{Desktop, PendingOperation};
use tablecraft_runtime::Error;

impl Desktop {
    pub(super) fn fail_read(&mut self, generation: u64, error: &Error) {
        match &self.pending {
            PendingOperation::QueryContents { target, request_id } => {
                self.state
                    .fail_query_contents(*request_id, target, error.to_string());
            }

            PendingOperation::DatabaseOptionsInspection {
                revision,
                request_id,
            } => {
                let (revision, id) = (*revision, *request_id);
                self.fail_database_options_inspection(revision, id, error.to_string());
            }
            PendingOperation::DatabaseOptionsReview { request_id } => {
                let id = *request_id;
                self.database_options_review_failed(id, error.to_string());
            }
            PendingOperation::DatabaseOptionsReopen { request_id } => {
                let id = *request_id;
                self.database_options_reopen_failed(id, error.to_string());
            }
            PendingOperation::TableDropInspection { .. } => {
                self.table_drop_inspection_failed(generation, error.to_string());
            }
            PendingOperation::TableDropReview { request_id } => {
                let id = *request_id;
                self.table_drop_review_failed(id, error.to_string());
            }
            PendingOperation::TableDropReopen { request_id } => {
                let id = *request_id;
                self.table_drop_reopen_failed(id, error.to_string());
            }
            PendingOperation::FieldDropInspection { .. } => {
                self.field_drop_inspection_failed(generation, error.to_string());
            }
            PendingOperation::FieldDropReview { request_id } => {
                let id = *request_id;
                self.field_drop_review_failed(id, error.to_string());
            }
            PendingOperation::FieldDropReopen { request_id } => {
                let id = *request_id;
                self.field_drop_reopen_failed(id, error.to_string());
            }
            PendingOperation::FieldRenameInspection { .. } => {
                self.field_rename_inspection_failed(generation, error.to_string());
            }
            PendingOperation::FieldRenameReview { request_id } => {
                let id = *request_id;
                self.field_rename_review_failed(id, error.to_string());
            }
            PendingOperation::FieldRenameReopen { request_id } => {
                let id = *request_id;
                self.field_rename_reopen_failed(id, error.to_string());
            }
            PendingOperation::FieldAdditionAvailability { .. } => {
                self.field_addition_availability_failed(generation, error.to_string())
            }
            PendingOperation::FieldAdditionReview { request_id } => {
                let id = *request_id;
                self.field_addition_review_failed(id, error.to_string());
            }
            PendingOperation::FieldAdditionReopen { request_id } => {
                let id = *request_id;
                self.field_addition_reopen_failed(id, error.to_string());
            }
            PendingOperation::TableRenameReview { request_id } => {
                self.table_rename_review_failed(*request_id, error.to_string());
            }
            PendingOperation::RenamedTableReopen { .. } => {
                self.fail_table_rename_reopen(error);
            }

            PendingOperation::QuerySchemaReopen { request_id } => {
                self.fail_query_schema_reopen(*request_id, error.to_string());
            }
            PendingOperation::QueryActionReview { request_id } => {
                let request_id = *request_id;
                self.fail_query_action_review(request_id, error.to_string());
            }
            PendingOperation::QueryDiscovery { inspection_id, .. } => {
                let id = *inspection_id;
                self.query_parameters = None;
                if self.state.fail_query_parameter_inspection(id) {
                    self.state.error = Some(error.to_string());
                }
            }
            PendingOperation::QueryInspection { request_id, .. } => {
                let request_id = *request_id;
                self.query_parameters = None;
                if self.state.fail_query_parameter_inspection(request_id) {
                    self.state.error = Some(error.to_string());
                }
            }
            PendingOperation::QueryDefinitionReview { request_id } => {
                let request_id = *request_id;
                self.query_definition_review_failed(request_id, error.to_string());
            }
            PendingOperation::QueryDefinitionReopen { .. } => {
                self.fail_query_definition_reopen(error);
            }
            PendingOperation::TableCreationReview { request_id } => {
                let request_id = *request_id;
                self.table_creation_review_failed(request_id, error.to_string());
            }
            PendingOperation::CreatedTableReopen { request_id } => {
                let request_id = *request_id;
                self.table_creation_reopen_failed(request_id, error.to_string());
            }
            PendingOperation::Query { .. } => {
                self.state.clear_query_run();
                self.state.error = Some(error.to_string());
            }
            PendingOperation::ReplaceFind { request_id } => {
                self.state.fail_replace_find(*request_id, error.to_string());
            }
            PendingOperation::QueryDesignDescribe { request_id, .. }
            | PendingOperation::QueryDesignBuild { request_id, .. }
            | PendingOperation::QueryDesignImport { request_id, .. }
            | PendingOperation::QueryDesignRun { request_id } => {
                self.state.fail_query_design(*request_id, error.to_string());
            }
            PendingOperation::TotalsDescribe { request_id, .. }
            | PendingOperation::TotalsCompute { request_id, .. } => {
                self.state.fail_totals(*request_id, error.to_string());
            }
            PendingOperation::FieldFilterDescribe { request_id, .. }
            | PendingOperation::FieldFilterApply { request_id } => {
                self.state.fail_field_filter(*request_id, error.to_string());
            }
            PendingOperation::FilterSelection { request_id }
            | PendingOperation::FilterPage { request_id }
            | PendingOperation::FilterRestore { request_id }
            | PendingOperation::SortView { request_id }
            | PendingOperation::ClearFilter { request_id } => {
                self.state.fail_filter(*request_id, error.to_string());
            }
            PendingOperation::Find { request_id, .. }
            | PendingOperation::FindPage { request_id, .. } => {
                self.state.fail_find(*request_id, error.to_string());
            }
            PendingOperation::SavedSqlInspection {
                request_id,
                request,
            } => {
                self.state
                    .fail_saved_query_sql(*request_id, request, error.to_string());
            }
            PendingOperation::SavedInspection { target, request_id }
            | PendingOperation::SavedRun { target, request_id } => {
                self.state
                    .fail_saved_query(*request_id, target, error.to_string());
            }
            PendingOperation::Record { request_id, .. } => {
                self.state
                    .fail_record_navigation(*request_id, error.to_string());
            }
            PendingOperation::Edit { request_id } => {
                self.state.fail_edit(*request_id, error.to_string());
            }
            PendingOperation::Delete { request_id } => {
                self.state.fail_delete(*request_id, error.to_string());
            }
            PendingOperation::Insert { request_id } => {
                self.state.fail_insert(*request_id, error.to_string());
            }
            PendingOperation::InsertRetry { request_id, .. } => {
                self.state.fail_insert_retry(*request_id, error.to_string());
            }
            PendingOperation::Contents { target, request_id } => {
                self.state
                    .fail_contents(*request_id, target, error.to_string());
            }
            _ => self.state.error = Some(error.to_string()),
        }
    }
}
