//! Shared native-mutation presentation guards, independent of read/navigation.
use super::State;

impl State {
    pub fn native_source_stale(&self) -> bool {
        self.uncertain_native_revision.is_some_and(|revision| {
            self.database
                .as_ref()
                .is_some_and(|database| database.document_revision == revision)
        }) || self.field_addition_source_stale()
            || self.database_options_source_stale()
            || self.field_rename_source_stale()
            || self.field_drop_source_stale()
    }

    /// The host observed actual save actor teardown without a native terminal
    /// result. This retires no draft and certifies neither original nor AFTER.
    /// The marker survives dismissal and clears only by a different source revision.
    pub fn mark_native_save_uncertain(&mut self) {
        self.uncertain_native_revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
    }

    pub fn native_mutation_blocked(&self) -> bool {
        self.native_mutation_blocked_without_field_rename() || self.field_rename_open()
    }

    pub(in crate::workspace) fn native_mutation_blocked_without_field_rename(&self) -> bool {
        self.native_mutation_blocked_without_saved_field() || self.field_drop_open()
    }

    pub(in crate::workspace) fn native_mutation_blocked_without_field_drop(&self) -> bool {
        self.native_mutation_blocked_without_saved_field() || self.field_rename_open()
    }

    fn native_mutation_blocked_without_saved_field(&self) -> bool {
        self.native_source_stale() || self.options_dialog_open() || self.database_options_pending()
    }
}
