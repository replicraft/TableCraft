//! Nonallocating ownership checks keep private object-session storage local.
use super::{ObjectKind, ObjectTab, Objects};

impl Objects {
    /// Scratch SQL/QBE objects have no persisted definition, even after Run.
    pub(in crate::workspace) fn has_unsaved_queries(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.tab.target.kind == ObjectKind::Sql)
    }

    pub(in crate::workspace) fn is_active_tab(&self, tab: &ObjectTab) -> bool {
        self.active == Some(tab.id) && self.contains_tab(tab)
    }

    pub(in crate::workspace) fn contains_tab(&self, tab: &ObjectTab) -> bool {
        self.entries.iter().any(|entry| entry.tab == *tab)
    }
}
