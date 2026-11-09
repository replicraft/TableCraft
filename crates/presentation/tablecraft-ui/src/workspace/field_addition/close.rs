//! Exact saved-field close confirmation; native Access appearance is unqualified.
//! Hosts own deferred tab/file/window intent and retain it through save failures.
mod modal;
#[cfg(test)]
mod tests;

use super::{FieldAdditionRequest, Phase, copy_tab, table_name};
use crate::workspace::{ObjectTab, State};
use tablecraft_builders::table::FieldAdditionDeclaration;

/// User decision only, without a native capability or deferred host action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionCloseDecision {
    Save,
    Discard,
    Cancel,
}

/// Exact naming owner/revision approved by this choice. Only Save carries the
/// immutable naming request; it does not start a runtime review or save worker.
pub struct FieldAdditionCloseResult {
    pub declaration: FieldAdditionDeclaration,
    pub tab: ObjectTab,
    pub draft_revision: u64,
    pub decision: FieldAdditionCloseDecision,
    pub request: Option<FieldAdditionRequest>,
}

pub(super) struct Confirmation {
    declaration: FieldAdditionDeclaration,
    tab: ObjectTab,
    draft_revision: u64,
}

impl Confirmation {
    /// Inline storage belongs to Model; only the owned table spelling is heap.
    pub(super) fn heap_bytes(&self) -> Option<usize> {
        Some(table_name(&self.tab)?.capacity())
    }
}

impl State {
    pub fn field_addition_close_open(&self) -> bool {
        self.field_addition.close.is_some()
    }

    /// Can prompt without activating a parked owner. Only an exact unstarted
    /// naming draft can be discarded; a running review/save must settle first.
    pub(in crate::workspace) fn can_request_field_addition_close(&self, tab: &ObjectTab) -> bool {
        self.object_switch_ready_without_field_addition()
            && self.field_addition_status.is_none()
            && self.field_addition_close_owner_bound(tab)
            && self
                .field_addition
                .draft()
                .is_some_and(|(owner, _, _, phase)| owner == tab && phase == Phase::Naming)
    }

    pub fn request_field_addition_close(&mut self, tab: &ObjectTab) -> bool {
        let previous = self.backstage;
        self.dismiss_backstage();
        if self.request_field_addition_close_in_document(tab) {
            true
        } else {
            self.backstage = previous;
            false
        }
    }

    fn request_field_addition_close_in_document(&mut self, tab: &ObjectTab) -> bool {
        if let Some(existing) = &self.field_addition.close {
            return existing.tab == *tab && self.field_addition_close_current();
        }
        if !self.can_request_field_addition_close(tab) {
            return false;
        }
        let Some((_, _, draft_revision, _)) = self.field_addition.draft() else {
            return false;
        };
        let Some(tab) = copy_tab(tab) else {
            return false;
        };
        let Some(declaration) = self.field_addition_declaration() else {
            return false;
        };
        let confirmation = Confirmation {
            declaration,
            tab,
            draft_revision,
        };
        if !self.admit_object_bytes(confirmation.heap_bytes()) {
            return false;
        }
        self.field_addition.close = Some(confirmation);
        true
    }

    pub(in crate::workspace) fn field_addition_unstarted_matches(
        &self,
        tab: &ObjectTab,
        revision: u64,
    ) -> bool {
        self.field_addition
            .draft()
            .is_some_and(|(owner, _, current, phase)| {
                owner == tab && current == revision && phase == Phase::Naming
            })
    }

    fn field_addition_close_current(&self) -> bool {
        self.field_addition
            .close
            .as_ref()
            .is_some_and(|confirmation| {
                self.field_addition_declaration() == Some(confirmation.declaration)
                    && self.field_addition_unstarted_matches(
                        &confirmation.tab,
                        confirmation.draft_revision,
                    )
                    && self.field_addition_close_owner_bound(&confirmation.tab)
            })
    }

    pub fn can_save_field_addition_close(&self) -> bool {
        self.field_addition_close_current()
            && !self.native_mutation_blocked()
            && self.field_addition_available
            && self.object_switch_ready_without_field_addition()
            && self.field_addition_status.is_none()
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self
                .field_addition
                .close
                .as_ref()
                .is_some_and(|confirmation| {
                    self.field_addition_close_owner_can_review(&confirmation.tab)
                        && self
                            .field_addition
                            .draft()
                            .is_some_and(|(_, name, _, _)| !name.is_empty())
                        && self.field_addition.inspections.iter().any(|inspection| {
                            inspection.tab == confirmation.tab
                                && inspection.available.is_some_and(|choices| {
                                    choices.supports(confirmation.declaration.kind())
                                })
                        })
                })
    }

    /// A stale Save/Discard does nothing. Cancel dismisses only this prompt.
    /// Save keeps the exact name and returns a snapshot; the host may resume its
    /// deferred close only after known AFTER ACK and full-witness publication.
    /// A failed/cancelled native operation must keep that host intent and name.
    pub fn choose_field_addition_close(
        &mut self,
        decision: FieldAdditionCloseDecision,
    ) -> Option<FieldAdditionCloseResult> {
        let confirmation = self.field_addition.close.as_ref()?;
        if decision != FieldAdditionCloseDecision::Cancel
            && (!self.field_addition_close_current()
                || (decision == FieldAdditionCloseDecision::Save
                    && !self.can_save_field_addition_close()))
        {
            return None;
        }
        let request = if decision == FieldAdditionCloseDecision::Save {
            let request = self.field_addition.request(&confirmation.tab)?;
            if !self.admit_object_bytes(request.retained_bytes()) {
                return None;
            }
            Some(request)
        } else {
            None
        };
        let confirmation = self.field_addition.close.take()?;
        if decision == FieldAdditionCloseDecision::Save
            && !self
                .activate_field_addition_close_owner(&confirmation.tab, confirmation.draft_revision)
        {
            self.field_addition.close = Some(confirmation);
            return None;
        }
        if decision == FieldAdditionCloseDecision::Discard
            && !self
                .field_addition
                .discard(&confirmation.tab, confirmation.draft_revision)
        {
            self.field_addition.close = Some(confirmation);
            return None;
        }
        Some(FieldAdditionCloseResult {
            declaration: confirmation.declaration,
            tab: confirmation.tab,
            draft_revision: confirmation.draft_revision,
            decision,
            request,
        })
    }
}
