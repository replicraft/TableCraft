//! Exact saved-field close confirmation; native Access appearance is unqualified.
//! Hosts own deferred tab/file/window intent and retain it through save failures.
mod modal;
#[cfg(test)]
mod tests;

use super::{NumberColumnRequest, Phase, copy_tab, table_name};
use crate::workspace::{ObjectTab, State};

/// User decision only, without a native capability or deferred host action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberColumnCloseDecision {
    Save,
    Discard,
    Cancel,
}

/// Exact naming owner/revision approved by this choice. Only Save carries the
/// immutable naming request; it does not start a runtime review or save worker.
pub struct NumberColumnCloseResult {
    pub tab: ObjectTab,
    pub draft_revision: u64,
    pub decision: NumberColumnCloseDecision,
    pub request: Option<NumberColumnRequest>,
}

pub(super) struct Confirmation {
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
    pub fn number_column_close_open(&self) -> bool {
        self.number_column.close.is_some()
    }

    /// Can prompt without activating a parked owner. Only an exact unstarted
    /// naming draft can be discarded; a running review/save must settle first.
    pub(in crate::workspace) fn can_request_number_column_close(&self, tab: &ObjectTab) -> bool {
        self.object_switch_ready_without_number_column()
            && self.number_column_status.is_none()
            && self.number_column_close_owner_bound(tab)
            && self
                .number_column
                .draft()
                .is_some_and(|(owner, _, _, phase)| owner == tab && phase == Phase::Naming)
    }

    pub fn request_number_column_close(&mut self, tab: &ObjectTab) -> bool {
        let previous = self.backstage;
        self.dismiss_backstage();
        if self.request_number_column_close_in_document(tab) {
            true
        } else {
            self.backstage = previous;
            false
        }
    }

    fn request_number_column_close_in_document(&mut self, tab: &ObjectTab) -> bool {
        if let Some(existing) = &self.number_column.close {
            return existing.tab == *tab && self.number_column_close_current();
        }
        if !self.can_request_number_column_close(tab) {
            return false;
        }
        let Some((_, _, draft_revision, _)) = self.number_column.draft() else {
            return false;
        };
        let Some(tab) = copy_tab(tab) else {
            return false;
        };
        let confirmation = Confirmation {
            tab,
            draft_revision,
        };
        if !self.admit_object_bytes(confirmation.heap_bytes()) {
            return false;
        }
        self.number_column.close = Some(confirmation);
        true
    }

    pub(in crate::workspace) fn number_column_unstarted_matches(
        &self,
        tab: &ObjectTab,
        revision: u64,
    ) -> bool {
        self.number_column
            .draft()
            .is_some_and(|(owner, _, current, phase)| {
                owner == tab && current == revision && phase == Phase::Naming
            })
    }

    fn number_column_close_current(&self) -> bool {
        self.number_column
            .close
            .as_ref()
            .is_some_and(|confirmation| {
                self.number_column_unstarted_matches(&confirmation.tab, confirmation.draft_revision)
                    && self.number_column_close_owner_bound(&confirmation.tab)
            })
    }

    pub fn can_save_number_column_close(&self) -> bool {
        self.number_column_close_current()
            && !self.number_column_source_stale()
            && self.number_column_available
            && self.object_switch_ready_without_number_column()
            && self.number_column_status.is_none()
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self
                .number_column
                .close
                .as_ref()
                .is_some_and(|confirmation| {
                    self.number_column_close_owner_can_review(&confirmation.tab)
                        && self
                            .number_column
                            .draft()
                            .is_some_and(|(_, name, _, _)| !name.is_empty())
                        && self.number_column.inspections.iter().any(|inspection| {
                            inspection.tab == confirmation.tab && inspection.available == Some(true)
                        })
                })
    }

    /// A stale Save/Discard does nothing. Cancel dismisses only this prompt.
    /// Save keeps the exact name and returns a snapshot; the host may resume its
    /// deferred close only after known AFTER ACK and full-witness publication.
    /// A failed/cancelled native operation must keep that host intent and name.
    pub fn choose_number_column_close(
        &mut self,
        decision: NumberColumnCloseDecision,
    ) -> Option<NumberColumnCloseResult> {
        let confirmation = self.number_column.close.as_ref()?;
        if decision != NumberColumnCloseDecision::Cancel
            && (!self.number_column_close_current()
                || (decision == NumberColumnCloseDecision::Save
                    && !self.can_save_number_column_close()))
        {
            return None;
        }
        let request = if decision == NumberColumnCloseDecision::Save {
            let request = self.number_column.request(&confirmation.tab)?;
            if !self.admit_object_bytes(request.retained_bytes()) {
                return None;
            }
            Some(request)
        } else {
            None
        };
        let confirmation = self.number_column.close.take()?;
        if decision == NumberColumnCloseDecision::Save
            && !self
                .activate_number_column_close_owner(&confirmation.tab, confirmation.draft_revision)
        {
            self.number_column.close = Some(confirmation);
            return None;
        }
        if decision == NumberColumnCloseDecision::Discard
            && !self
                .number_column
                .discard(&confirmation.tab, confirmation.draft_revision)
        {
            self.number_column.close = Some(confirmation);
            return None;
        }
        Some(NumberColumnCloseResult {
            tab: confirmation.tab,
            draft_revision: confirmation.draft_revision,
            decision,
            request,
        })
    }
}
