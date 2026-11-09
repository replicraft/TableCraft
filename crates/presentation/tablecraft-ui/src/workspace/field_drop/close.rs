use super::{FieldDropRequest, FieldDropTarget, model::Phase};
use crate::workspace::{State, objects::MAX_RETAINED_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldDropCloseDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldDropCloseResult {
    pub target: FieldDropTarget,
    pub draft_revision: u64,
    pub decision: FieldDropCloseDecision,
    pub request: Option<FieldDropRequest>,
}

pub(super) struct Confirmation {
    pub target: FieldDropTarget,
    pub revision: u64,
}

impl State {
    /// Host keeps the exact deferred tab/file/window action; UI grants no close.
    pub fn can_request_field_drop_transition(&self, tab: &crate::ObjectTab) -> bool {
        let Some(draft) = self.field_drop.presented() else {
            return false;
        };
        draft.phase == Phase::PendingDeletion
            && self.field_drop.request_memory.is_none()
            && self.field_drop.close.is_none()
            && self.field_drop.status.is_none()
            && self.field_drop_owner_bound(&draft.target)
            && self.object_binding(&tab.target)
            && self.objects.contains_tab(tab)
            && self.object_switch_ready_without_saved_field_name()
    }

    pub fn request_current_field_drop_close(&mut self) -> bool {
        let previous = self.backstage;
        self.dismiss_backstage();
        if self.request_current_field_drop_close_in_document() {
            true
        } else {
            self.backstage = previous;
            false
        }
    }

    fn request_current_field_drop_close_in_document(&mut self) -> bool {
        if self.field_drop.close.is_some()
            || self.field_drop.status.is_some()
            || self.field_drop.request_memory.is_some()
        {
            return false;
        }
        let Some(draft) = self.field_drop.presented() else {
            return false;
        };
        if draft.phase != Phase::PendingDeletion || !self.field_drop_owner_bound(&draft.target) {
            return false;
        }
        let Some(target) = draft.target.copy() else {
            return false;
        };
        let revision = draft.revision;
        if !self.admit_object_bytes(target.heap_bytes()) {
            return false;
        }
        self.field_drop.close = Some(Confirmation { target, revision });
        true
    }

    pub fn field_drop_close_open(&self) -> bool {
        self.field_drop.close.is_some()
    }

    pub fn field_drop_close_current(&self) -> bool {
        let Some(close) = &self.field_drop.close else {
            return false;
        };
        self.field_drop.presented().is_some_and(|draft| {
            draft.target == close.target
                && draft.revision == close.revision
                && draft.phase == Phase::PendingDeletion
                && self.field_drop_owner_bound(&draft.target)
        })
    }

    pub fn can_save_field_drop_close(&self) -> bool {
        self.field_drop_close_current()
            && self.field_drop.draft.as_ref().is_some_and(|draft| {
                self.field_drop_context_without_close(&draft.target)
                    && matches!(draft.observation, super::model::Observation::Eligible)
            })
    }

    pub fn choose_field_drop_close(
        &mut self,
        decision: FieldDropCloseDecision,
    ) -> Option<FieldDropCloseResult> {
        if decision != FieldDropCloseDecision::Cancel && !self.field_drop_close_current() {
            return None;
        }
        if decision == FieldDropCloseDecision::Save && !self.can_save_field_drop_close() {
            return None;
        }
        if decision == FieldDropCloseDecision::Cancel {
            let close = self.field_drop.close.take()?;
            return Some(FieldDropCloseResult {
                target: close.target,
                draft_revision: close.revision,
                decision,
                request: None,
            });
        }
        let close = self.field_drop.close.as_ref()?;
        let target = close.target.copy()?;
        let revision = close.revision;
        let request = if decision == FieldDropCloseDecision::Save {
            Some(FieldDropRequest {
                target: target.copy()?,
                draft_revision: revision,
            })
        } else {
            None
        };
        let additional = target
            .heap_bytes()?
            .checked_add(size_of::<FieldDropCloseResult>())?
            .checked_add(
                request
                    .as_ref()
                    .map_or(Some(0), |request| request.target.heap_bytes())?,
            )?;
        if self.object_bytes()?.checked_add(additional)? > MAX_RETAINED_BYTES {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        }
        if decision == FieldDropCloseDecision::Discard
            && !self.field_drop.discard(&target, revision)
        {
            return None;
        }
        self.field_drop.close = None;
        Some(FieldDropCloseResult {
            target,
            draft_revision: revision,
            decision,
            request,
        })
    }
}
