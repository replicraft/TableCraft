//! Nonallocating ownership checks keep private object-session storage local.
use super::{ObjectTab, Objects};

impl Objects {
    pub(in crate::workspace) fn contains_tab(&self, tab: &ObjectTab) -> bool {
        self.entries.iter().any(|entry| entry.tab == *tab)
    }
}
