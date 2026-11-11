//! Terminal readonly observations resolve their recorded owner across view changes.
use crate::workspace::{State, objects::MAX_RETAINED_BYTES};
use std::sync::Arc;
use tablecraft_builders::table::TypedFieldAdditionAvailability;

impl State {
    /// Native Worker IDs are unique within this host. Only issued, still-pending
    /// availability requests enter the superseded-read completion path.
    pub fn has_pending_field_addition_inspection(&self, request_id: u64) -> bool {
        self.field_addition
            .inspections
            .iter()
            .any(|item| item.request_id == Some(request_id) && item.pending())
    }

    /// Capture only the exact recorded source fact. This does not select a tab,
    /// change the current document or grant a native editing capability.
    pub fn complete_field_addition_inspection(
        &mut self,
        request_id: u64,
        document_revision: u64,
        table: &str,
        number: bool,
        short_text: bool,
        typed_declarations: Arc<[TypedFieldAdditionAvailability]>,
    ) -> bool {
        let additional = std::mem::size_of_val(typed_declarations.as_ref());
        if !self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(additional))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
        {
            return false;
        }
        let Some(item) = self.field_addition.inspections.iter_mut().find(|item| {
            item.request_id == Some(request_id)
                && item.pending()
                && item.tab.target.document_revision == document_revision
                && super::table_name(&item.tab).is_some_and(|name| name == table)
        }) else {
            return false;
        };
        item.available = Some(super::FieldAdditionChoices {
            number,
            short_text,
            typed_declarations,
        });
        true
    }

    /// Resolve the exact terminal error without replacing another operation's
    /// busy/pending/error presentation. An empty retained report is the existing
    /// localized memory-limit sentinel; terminal retry controls remain usable.
    pub fn fail_recorded_field_addition_inspection(
        &mut self,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.has_pending_field_addition_inspection(request_id) {
            return false;
        }
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(message.capacity()))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        let Some(item) = self
            .field_addition
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
    /// Issuance failed before an identity existed. Retire only this exact unbound
    /// reservation; no native source observation or availability is invented.
    pub fn interrupt_prepared_field_addition_inspection(&mut self, tab: &crate::ObjectTab) -> bool {
        let Some(item) = self
            .field_addition
            .inspections
            .iter_mut()
            .find(|item| item.tab == *tab && item.request_id.is_none() && item.pending())
        else {
            return false;
        };
        item.interrupted = true;
        true
    }

    /// Publication was revoked by the readonly worker's generation owner. This
    /// does not assert that its native computation has already stopped.
    pub fn interrupt_field_addition_inspection(&mut self, request_id: u64) -> bool {
        let Some(item) = self
            .field_addition
            .inspections
            .iter_mut()
            .find(|item| item.request_id == Some(request_id) && item.pending())
        else {
            return false;
        };
        item.interrupted = true;
        true
    }

    pub fn field_addition_inspection_interrupted(&self, tab: &crate::ObjectTab) -> bool {
        self.field_addition
            .inspections
            .iter()
            .any(|item| item.tab == *tab && item.interrupted)
    }

    /// Host supplies its runtime-owned publication-current fact; no runtime or
    /// backend type enters shared presentation state.
    pub fn retire_field_addition_inspection_publications(
        &mut self,
        mut current: impl FnMut(u64) -> bool,
    ) {
        for item in &mut self.field_addition.inspections {
            if item.pending() && item.request_id.is_some_and(|id| !current(id)) {
                item.interrupted = true;
            }
        }
    }
}
