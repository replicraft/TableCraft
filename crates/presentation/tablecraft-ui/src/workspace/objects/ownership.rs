//! Nonallocating ownership checks keep private object-session storage local.
use super::{ObjectTab, Objects};

impl Objects {
    pub(in crate::workspace) fn is_active_tab(&self, tab: &ObjectTab) -> bool {
        self.active == Some(tab.id) && self.contains_tab(tab)
    }

    pub(in crate::workspace) fn contains_tab(&self, tab: &ObjectTab) -> bool {
        self.entries.iter().any(|entry| entry.tab == *tab)
    }
}
