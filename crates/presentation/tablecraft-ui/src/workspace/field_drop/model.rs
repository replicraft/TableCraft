use super::{FieldDropRequest, FieldDropSaveIdentity, FieldDropTarget};
use crate::workspace::schema_save::Status;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Confirming,
    PendingDeletion,
    Reviewing { request_id: u64 },
    Reviewed { request_id: u64, review_id: u64 },
    Owned { identity: FieldDropSaveIdentity },
}

pub(super) enum Observation {
    Pending { request_id: Option<u64> },
    Eligible,
    Unsupported,
    Failed { message: String },
    Interrupted,
}

pub(super) struct Draft {
    pub target: FieldDropTarget,
    pub revision: u64,
    pub phase: Phase,
    pub observation: Observation,
}

#[derive(Default)]
pub(in crate::workspace) struct Model {
    pub(super) draft: Option<Draft>,
    pub(super) status: Option<Status<FieldDropSaveIdentity>>,
    pub(super) warning: Option<super::warning::Warning>,
    pub(super) close: Option<super::close::Confirmation>,
}

impl Model {
    pub fn begin(&mut self, target: FieldDropTarget) -> bool {
        if self.draft.is_some()
            || self.status.is_some()
            || self.warning.is_some()
            || self.close.is_some()
            || !target.bounded()
        {
            return false;
        }
        self.draft = Some(Draft {
            target,
            revision: 0,
            phase: Phase::Confirming,
            observation: Observation::Pending { request_id: None },
        });
        true
    }

    pub(super) fn matches(&self, request: &FieldDropRequest, phase: Phase) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.target == request.target
                && draft.revision == request.draft_revision
                && draft.phase == phase
        })
    }

    pub fn inspection_matches(&self, target: &FieldDropTarget, request_id: u64) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.target == *target && draft.phase == Phase::Confirming
                && matches!(draft.observation, Observation::Pending { request_id: Some(id) } if id == request_id)
        })
    }

    pub(super) fn presented(&self) -> Option<&Draft> {
        self.draft.as_ref().or_else(|| {
            self.warning
                .as_ref()
                .filter(|warning| warning.open)
                .map(|warning| &warning.owner)
        })
    }

    pub fn dirty(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.phase != Phase::Confirming)
    }

    pub fn discard(&mut self, target: &FieldDropTarget, revision: u64) -> bool {
        if let Some(draft) = &self.draft {
            if draft.target != *target
                || draft.revision != revision
                || draft.phase != Phase::PendingDeletion
            {
                return false;
            }
            self.draft = None;
            return true;
        }
        let Some(warning) = &mut self.warning else {
            return false;
        };
        if !warning.open || warning.owner.target != *target || warning.owner.revision != revision {
            return false;
        }
        // Dismiss presentation only. Known disk facts and stale-source refusal
        // remain retained until an explicit fresh document revision replaces them.
        warning.open = false;
        true
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>();
        if let Some(draft) = &self.draft {
            bytes = bytes.checked_add(draft.heap_bytes()?)?;
        }
        if let Some(status) = &self.status {
            bytes = bytes.checked_add(status.heap_bytes()?)?;
        }
        if let Some(warning) = &self.warning {
            bytes = bytes
                .checked_add(warning.owner.heap_bytes()?)?
                .checked_add(warning.status.heap_bytes()?)?;
        }
        if let Some(close) = &self.close {
            bytes = bytes.checked_add(close.target.heap_bytes()?)?;
        }
        Some(bytes)
    }
}

impl Draft {
    pub fn heap_bytes(&self) -> Option<usize> {
        self.target
            .heap_bytes()?
            .checked_add(match &self.observation {
                Observation::Failed { message } => message.capacity(),
                _ => 0,
            })
    }
}
