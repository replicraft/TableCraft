use crate::workspace::State;

impl State {
    /// Whether presentation-owned drafts and operations allow a deliberate restart.
    ///
    /// Native resource owners remain a separate host gate. This observation
    /// neither discards user input nor cancels workers or closes documents.
    pub fn can_restart_for_update(&self) -> bool {
        let native_or_edit_pending = self.busy
            || self.native_mutation_blocked()
            || self.edit.is_some()
            || self.insertion.is_some()
            || self.insert_continuation.is_some()
            || self.initial_record_notice.is_some()
            || self.deletion.is_some()
            || self.recovery.is_some()
            || self.creation_is_pending()
            || self.active_save.is_pending();
        if native_or_edit_pending {
            return false;
        }

        let unsaved_draft = self.has_unsaved_table_drafts()
            || self.table_draft.is_some()
            || self.has_modified_saved_query_sql()
            || !self.sql.is_empty()
            || self.objects.has_unsaved_queries()
            || self.field_addition_open()
            || self.field_addition_status.is_some()
            || self.creation_form_open();
        if unsaved_draft {
            return false;
        }

        let interaction_pending = self.paste_request.is_some()
            || self.ribbon_paste.is_some()
            || self.contents.is_some()
            || self.query_pending.is_some()
            || self.query_parameters.is_some()
            || self.query_action.is_some()
            || self.saved_query_sql_pending()
            || self.saved_query_sql_close.is_some()
            || self.saved_query_sql_save.is_some()
            || self.table_rename.is_some()
            || self.table_draft_save.is_some()
            || self.table_draft_close.is_some()
            || self.table_draft_field_delete.is_some()
            || self.schema_activation_pending
            || self.query_design_request_id().is_some()
            || self.query_design_dialog_open()
            || self.query_run_limits_open()
            || self.parameter_prompt_open()
            || self.find.is_some()
            || self.zoom_dialog_open()
            || self.table_filter_pending()
            || self.totals_request_id().is_some();
        !interaction_pending
    }
}
