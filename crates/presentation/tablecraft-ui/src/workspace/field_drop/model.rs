use super::{
    FieldDropConfirmationDecision, FieldDropConfirmationResult, FieldDropRequest,
    FieldDropSaveIdentity, FieldDropTarget,
};
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
    pub focus_no: bool,
}

#[derive(Clone, Copy)]
pub(super) struct RequestReservation {
    pub request_id: u64,
    pub bytes: usize,
}

#[derive(Default)]
pub(in crate::workspace) struct Model {
    pub(super) draft: Option<Draft>,
    pub(super) status: Option<Status<FieldDropSaveIdentity>>,
    pub(super) warning: Option<super::warning::Warning>,
    pub(super) close: Option<super::close::Confirmation>,
    pub(super) selection: Option<FieldDropTarget>,
    pub(super) request_memory: Option<RequestReservation>,
}

impl Model {
    pub fn begin(&mut self, target: FieldDropTarget) -> bool {
        if self.draft.is_some()
            || self.status.is_some()
            || self.warning.is_some()
            || self.close.is_some()
            || self.request_memory.is_some()
            || !target.bounded()
        {
            return false;
        }
        self.draft = Some(Draft {
            target,
            revision: 0,
            phase: Phase::Confirming,
            focus_no: true,
            observation: Observation::Pending { request_id: None },
        });
        true
    }

    pub(super) fn choose_confirmation(
        &mut self,
        decision: FieldDropConfirmationDecision,
    ) -> Option<FieldDropConfirmationResult> {
        let draft = self.draft.as_ref()?;
        if draft.phase != Phase::Confirming {
            return None;
        }
        let inspection_request_id = match draft.observation {
            Observation::Pending { request_id } => request_id,
            _ => None,
        };
        match decision {
            FieldDropConfirmationDecision::Yes => {
                if !matches!(draft.observation, Observation::Eligible) {
                    return None;
                }
                let revision = draft.revision.checked_add(1)?;
                let draft = self.draft.as_mut()?;
                draft.revision = revision;
                draft.phase = Phase::PendingDeletion;
            }
            FieldDropConfirmationDecision::No => self.draft = None,
        }
        Some(FieldDropConfirmationResult {
            decision,
            inspection_request_id,
        })
    }

    pub(super) fn matches(&self, request: &FieldDropRequest, phase: Phase) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.target == request.target
                && draft.revision == request.draft_revision
                && draft.phase == phase
                && match phase {
                    Phase::Reviewing { request_id } | Phase::Reviewed { request_id, .. } => {
                        self.reserves_request(request, request_id)
                    }
                    Phase::Owned { identity } => {
                        self.reserves_request(request, identity.request_id)
                    }
                    Phase::Confirming | Phase::PendingDeletion => true,
                }
        })
    }

    pub(super) fn reserves_request(&self, request: &FieldDropRequest, request_id: u64) -> bool {
        self.request_memory.is_some_and(|reserved| {
            reserved.request_id == request_id && request.retained_bytes() == Some(reserved.bytes)
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
        if self.request_memory.is_some() {
            return false;
        }
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
        let mut bytes = size_of::<Self>()
            .checked_add(self.request_memory.map_or(0, |reserved| reserved.bytes))?;
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
        if let Some(selection) = &self.selection {
            bytes = bytes.checked_add(selection.heap_bytes()?)?;
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
