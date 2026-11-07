//! Explicit first-table authoring uses a host-qualified, revision-bound fact.
use crate::workspace::State;

impl State {
    /// Publish qualification for the current opened document. This enables a
    /// local draft; native persistence still requires complete admission.
    pub fn set_first_table_creation_available(&mut self, revision: u64, available: bool) {
        if self
            .database
            .as_ref()
            .map(|database| database.document_revision)
            != Some(revision)
        {
            return;
        }
        self.first_table_creation = available.then_some(revision);
    }

    pub fn can_create_first_table(&self) -> bool {
        self.table_save_available
            && self.object_switch_ready()
            && !self.creation_is_pending()
            && !self.has_unsaved_table_drafts()
            && !self.insertion_dialog_open()
            && !self.recovery_prompt_open()
            && self.database.as_ref().is_some_and(|database| {
                self.first_table_creation == Some(database.document_revision)
                    && database.tables.iter().all(|table| table.system)
            })
    }
}

#[cfg(test)]
mod tests;
