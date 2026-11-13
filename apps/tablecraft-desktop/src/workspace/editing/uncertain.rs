//! Actor loss is neither original/no-write nor a checked replacement.
use super::{Desktop, SaveOutcome, SaveOwner};
use tablecraft_ui::InsertRetryPermission;

impl Desktop {
    /// Handle uncertainty before any operation-specific original/AFTER branch.
    /// A true result means the request was handled, even if ACK was refused.
    pub(super) fn acknowledge_uncertain_save(&mut self, id: u64) -> bool {
        let Some(save) = self.editing.save.as_ref().filter(|save| {
            save.id == id
                && save
                    .report
                    .is_some_and(|report| matches!(report.outcome, SaveOutcome::Uncertain { .. }))
        }) else {
            return false;
        };
        let owner = save.owner;
        let acknowledged = match owner {
            SaveOwner::FieldRename => self.acknowledge_uncertain_field_rename(id),
            SaveOwner::TableDrop => self.acknowledge_uncertain_table_drop(id),
            SaveOwner::FieldDrop => self.acknowledge_uncertain_field_drop(id),
            SaveOwner::FieldAddition => self.acknowledge_uncertain_field_addition(id),
            SaveOwner::QueryDefinition => self.acknowledge_uncertain_query_definition(id),
            SaveOwner::QueryCreation => self.acknowledge_uncertain_query_creation(id),
            SaveOwner::DatabaseOptions => self.acknowledge_uncertain_database_options(id),
            SaveOwner::TableCreation => self.acknowledge_uncertain_table_creation(id),
            SaveOwner::TableRename => {
                if !self.state.can_acknowledge_table_rename_save(id)
                    || !self.editing.worker.acknowledge(id)
                {
                    return true;
                }
                self.state.acknowledge_uncertain_table_rename_save(id);
                self.table_rename = None;
                true
            }
            SaveOwner::Insert => {
                if !self.state.can_acknowledge_insert_save(id)
                    || !self.editing.worker.acknowledge(id)
                {
                    return true;
                }
                self.state
                    .acknowledge_insert_save(id, InsertRetryPermission::Blocked);
                true
            }
            SaveOwner::Cell | SaveOwner::Delete | SaveOwner::QueryAction => {
                if matches!(owner, SaveOwner::QueryAction)
                    && !self.state.can_acknowledge_query_action_save(id)
                {
                    return true;
                }
                if !self.editing.worker.acknowledge(id) {
                    return true;
                }
                match owner {
                    SaveOwner::Cell => {
                        self.state.close_edit();
                    }
                    SaveOwner::Delete => {
                        self.state.close_delete();
                    }
                    SaveOwner::QueryAction => {
                        self.state.acknowledge_query_action_save(id);
                    }
                    _ => unreachable!("matched ordinary native save owner"),
                }
                true
            }
        };
        if acknowledged {
            self.editing.save = None;
            self.editing.insert_retry = None;
            self.editing.clear_review();
            self.state.mark_native_save_uncertain();
            self.state.busy = false;
            self.cancel_table_draft_action();
            self.state.error = Some(self.state.localizer.text("editing-save-uncertain"));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::editing::save_outcome;
    use tablecraft_runtime::{SaveKnownOutcome, SaveOperation, SaveReport};
    use tablecraft_ui::{EditSaveOutcome, Localizer};

    #[test]
    fn uncertain_native_result_never_becomes_success_original_or_retained_controls() {
        let localizer = Localizer::default();
        let outcome = SaveOutcome::Uncertain {
            last_known_outcome: Some(SaveKnownOutcome::OriginalVerified),
            last_completed: None,
        };
        let report = SaveReport {
            id: 8,
            operation: SaveOperation::Recover,
            outcome,
        };
        assert!(!report.stopped_before_target_writes());
        assert!(outcome.field_rename_reopen_facts().is_err());
        assert!(outcome.field_addition_reopen_facts().is_err());
        assert!(outcome.database_options_reopen_facts().is_err());
        assert!(outcome.query_expression_reopen_facts().is_err());
        let EditSaveOutcome::Failed(message) = save_outcome(&localizer, outcome) else {
            panic!("uncertain native actor must have no success/recovery presentation");
        };
        assert_eq!(message, localizer.text("editing-save-uncertain"));
        assert!(message.contains("Restart TableCraft"));
    }
}
