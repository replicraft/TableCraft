//! Explicit table authoring uses a host-qualified, revision-bound fact.
use crate::workspace::State;

impl State {
    /// Publish qualification for the current opened document. This enables a
    /// local draft; native persistence still requires complete admission.
    pub fn set_table_creation_available(&mut self, revision: u64, available: bool) {
        if self
            .database
            .as_ref()
            .map(|database| database.document_revision)
            != Some(revision)
        {
            return;
        }
        self.table_creation = available.then_some(revision);
        if !available {
            self.table_design = None;
        }
    }

    /// Manual Design currently requires the canonical-blank source profile.
    /// This presentation fact never substitutes for native review admission.
    pub fn set_table_design_available(&mut self, revision: u64, available: bool) {
        if self
            .database
            .as_ref()
            .map(|database| database.document_revision)
            != Some(revision)
        {
            return;
        }
        self.table_design =
            (available && self.table_creation == Some(revision)).then_some(revision);
    }

    pub fn can_create_table_design(&self) -> bool {
        self.can_create_table()
            && self
                .database
                .as_ref()
                .is_some_and(|database| self.table_design == Some(database.document_revision))
    }

    pub fn can_create_table(&self) -> bool {
        self.table_save_available
            && self.object_switch_ready()
            && !self.creation_is_pending()
            && !self.has_unsaved_table_drafts()
            && !self.insertion_dialog_open()
            && !self.recovery_prompt_open()
            && self
                .database
                .as_ref()
                .is_some_and(|database| self.table_creation == Some(database.document_revision))
    }
}

#[cfg(test)]
mod tests;
