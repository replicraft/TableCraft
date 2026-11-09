//! Terminal readonly observations resolve their recorded owner across view changes.
use crate::workspace::{State, objects::MAX_RETAINED_BYTES};

impl State {
    /// Native Worker IDs are unique within this host. Only issued, still-pending
    /// availability requests enter the superseded-read completion path.
    pub fn has_pending_number_column_inspection(&self, request_id: u64) -> bool {
        self.number_column
            .inspections
            .iter()
            .any(|item| item.request_id == Some(request_id) && item.pending())
    }

    /// Capture only the exact recorded source fact. This does not select a tab,
    /// change the current document or grant a native editing capability.
    pub fn complete_number_column_inspection(
        &mut self,
        request_id: u64,
        document_revision: u64,
        table: &str,
        available: bool,
    ) -> bool {
        let Some(item) = self.number_column.inspections.iter_mut().find(|item| {
            item.request_id == Some(request_id)
                && item.pending()
                && item.tab.target.document_revision == document_revision
                && super::table_name(&item.tab).is_some_and(|name| name == table)
        }) else {
            return false;
        };
        item.available = Some(available);
        true
    }

    /// Resolve the exact terminal error without replacing another operation's
    /// busy/pending/error presentation. An empty retained report is the existing
    /// localized memory-limit sentinel; terminal retry controls remain usable.
    pub fn fail_recorded_number_column_inspection(
        &mut self,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.has_pending_number_column_inspection(request_id) {
            return false;
        }
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(message.capacity()))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        let Some(item) = self
            .number_column
            .inspections
            .iter_mut()
            .find(|item| item.request_id == Some(request_id) && item.pending())
        else {
            return false;
        };
        item.error = Some(if admitted { message } else { String::new() });
        true
    }
}

#[cfg(test)]
mod tests;

impl State {
    /// Publication was revoked by the readonly worker's generation owner. This
    /// does not assert that its native computation has already stopped.
    pub fn interrupt_number_column_inspection(&mut self, request_id: u64) -> bool {
        let Some(item) = self
            .number_column
            .inspections
            .iter_mut()
            .find(|item| item.request_id == Some(request_id) && item.pending())
        else {
            return false;
        };
        item.interrupted = true;
        true
    }

    pub fn number_column_inspection_interrupted(&self, tab: &crate::ObjectTab) -> bool {
        self.number_column
            .inspections
            .iter()
            .any(|item| item.tab == *tab && item.interrupted)
    }

    /// Host supplies its runtime-owned publication-current fact; no runtime or
    /// backend type enters shared presentation state.
    pub fn retire_number_column_inspection_publications(
        &mut self,
        mut current: impl FnMut(u64) -> bool,
    ) {
        for item in &mut self.number_column.inspections {
            if item.pending() && item.request_id.is_some_and(|id| !current(id)) {
                item.interrupted = true;
            }
        }
    }
}
