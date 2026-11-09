//! Shared native-mutation presentation guards, independent of read/navigation.
use super::State;

impl State {
    pub fn native_source_stale(&self) -> bool {
        self.field_addition_source_stale()
            || self.database_options_source_stale()
            || self.field_rename_source_stale()
            || self.field_drop_source_stale()
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
