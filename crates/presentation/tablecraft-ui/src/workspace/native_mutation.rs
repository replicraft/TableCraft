//! Shared native-mutation presentation guards, independent of read/navigation.
use super::State;

impl State {
    pub fn native_source_stale(&self) -> bool {
        self.field_addition_source_stale() || self.database_options_source_stale()
    }

    pub fn native_mutation_blocked(&self) -> bool {
        self.native_source_stale() || self.options_dialog_open() || self.database_options_pending()
    }
}
