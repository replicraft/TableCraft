//! Exact local close choices. The host alone retains deferred tab/file/window intent.
#[cfg(test)]
mod tests;

use super::{
    FieldRenameRequest, FieldRenameTarget, copy_text,
    model::{Model, Observation, Phase},
};
use crate::workspace::{State, objects::MAX_RETAINED_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRenameCloseDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldRenameCloseResult {
    pub target: FieldRenameTarget,
    pub draft_revision: u64,
    pub decision: FieldRenameCloseDecision,
    pub request: Option<FieldRenameRequest>,
    pub inspection_request_id: Option<u64>,
}

pub(super) struct Confirmation {
    pub target: FieldRenameTarget,
    pub revision: u64,
}

/// Reserve an exactly unchanged, unstarted editor before retiring its readonly
/// inspection. No native actor, proposal mutation or allocation is admitted here.
pub struct PreparedCleanFieldRenameClose<'a> {
    model: &'a mut Model,
    inspection_request_id: Option<u64>,
    warning: bool,
}

impl PreparedCleanFieldRenameClose<'_> {
    pub fn inspection_request_id(&self) -> Option<u64> {
        self.inspection_request_id
    }

    /// Host prerequisite: revoke only this token's exact readonly publication,
    /// if it still owns that worker. Native save actors never enter this token.
    pub fn complete(self) {
        if self.warning {
            self.model.warning.as_mut().unwrap().open = false;
        } else {
            self.model.draft = None;
        }
    }
}

impl State {
    pub fn prepare_clean_field_rename_close(
        &mut self,
    ) -> Option<PreparedCleanFieldRenameClose<'_>> {
        if self.field_rename.close.is_some() || self.field_rename.status.is_some() {
            return None;
        }
        let (inspection_request_id, warning) = if let Some(draft) = &self.field_rename.draft {
            if draft.phase != Phase::Naming || draft.proposed != draft.target.column {
                return None;
            }
            let request_id = match draft.observation {
                Observation::Pending { request_id } => request_id,
                _ => None,
            };
            (request_id, false)
        } else {
            let warning = self.field_rename.warning.as_ref()?;
            if !warning.open
                || warning.owner.proposed != warning.owner.target.column
                || !matches!(
                    warning.status.identity,
                    super::FieldRenameSaveIdentity::UnchangedReview { .. }
                )
            {
                return None;
            }
            (None, true)
        };

        Some(PreparedCleanFieldRenameClose {
            model: &mut self.field_rename,
            inspection_request_id,
            warning,
        })
    }

    /// Admit a deferred transition only. Actual tab mutation remains guarded
    /// until the host resolves this exact local naming owner.
    pub fn can_request_field_rename_transition(&self, tab: &crate::ObjectTab) -> bool {
        let Some(draft) = self.field_rename.presented() else {
            return false;
        };
        draft.phase == Phase::Naming
            && self.field_rename.close.is_none()
            && self.field_rename.status.is_none()
            && self.field_rename_owner_bound(&draft.target)
            && self.object_binding(&tab.target)
            && self.objects.contains_tab(tab)
            && self.object_switch_ready_without_saved_field_name()
    }

    pub fn request_current_field_rename_close(&mut self) -> bool {
        let previous = self.backstage;
        self.dismiss_backstage();
        if self.request_current_field_rename_close_in_document() {
            true
        } else {
            self.backstage = previous;
            false
        }
    }

    fn request_current_field_rename_close_in_document(&mut self) -> bool {
        if self.field_rename.close.is_some() || self.field_rename.status.is_some() {
            return false;
        }
        let Some(draft) = self.field_rename.presented() else {
            return false;
        };
        if draft.phase != Phase::Naming {
            return false;
        }
        let Some(target) = draft.target.copy() else {
            return false;
        };
        let revision = draft.revision;
        if !self.field_rename_owner_bound(&target) {
            return false;
        }
        if !self.admit_object_bytes(target.heap_bytes()) {
            return false;
        }

        self.field_rename.close = Some(Confirmation { target, revision });
        true
    }

    pub fn field_rename_close_open(&self) -> bool {
        self.field_rename.close.is_some()
    }

    pub fn field_rename_close_current(&self) -> bool {
        let Some(close) = &self.field_rename.close else {
            return false;
        };
        self.field_rename.presented().is_some_and(|draft| {
            draft.target == close.target
                && draft.revision == close.revision
                && draft.phase == Phase::Naming
                && self.field_rename_owner_bound(&draft.target)
        })
    }

    pub fn can_save_field_rename_close(&self) -> bool {
        let Some(close) = &self.field_rename.close else {
            return false;
        };
        self.field_rename_close_current()
            && self.field_rename.draft.is_some()
            && self.field_rename_context_without_close(&close.target)
            && self.field_rename.profile_fits()
    }

    pub fn choose_field_rename_close(
        &mut self,
        decision: FieldRenameCloseDecision,
    ) -> Option<FieldRenameCloseResult> {
        if decision != FieldRenameCloseDecision::Cancel && !self.field_rename_close_current() {
            return None;
        }
        if decision == FieldRenameCloseDecision::Save && !self.can_save_field_rename_close() {
            return None;
        }
        let close = self.field_rename.close.as_ref()?;
        let target = close.target.copy()?;
        let revision = close.revision;
        let inspection_request_id = self.pending_field_rename_inspection_id();
        let request = if decision == FieldRenameCloseDecision::Save {
            let draft = self.field_rename.draft.as_ref()?;
            Some(FieldRenameRequest {
                target: draft.target.copy()?,
                draft_revision: draft.revision,
                new_name: copy_text(&draft.proposed)?,
            })
        } else {
            None
        };
        let additional = target
            .heap_bytes()?
            .checked_add(request.as_ref().map_or(Some(0), |request| {
                request
                    .retained_bytes()?
                    .checked_sub(size_of::<FieldRenameRequest>())
            })?)?
            .checked_add(size_of::<FieldRenameCloseResult>())?;
        if self.object_bytes()?.checked_add(additional)? > MAX_RETAINED_BYTES {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        }

        if decision == FieldRenameCloseDecision::Discard
            && !self.field_rename.discard(&target, revision)
        {
            return None;
        }
        self.field_rename.close = None;
        Some(FieldRenameCloseResult {
            target,
            draft_revision: revision,
            decision,
            request,
            inspection_request_id,
        })
    }
}
