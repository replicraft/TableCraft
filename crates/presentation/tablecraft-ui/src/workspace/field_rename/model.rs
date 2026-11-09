//! Exact local proposal ownership. Native facts and reviews remain host-issued.
use super::{FieldRenameRequest, FieldRenameSaveIdentity, FieldRenameTarget};
use crate::workspace::field_name::bounded_name;
use crate::workspace::schema_save::Status;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Naming,
    Reviewing { request_id: u64 },
    Reviewed { request_id: u64, review_id: u64 },
    Owned { identity: FieldRenameSaveIdentity },
}

pub(super) enum Observation {
    Pending { request_id: Option<u64> },
    Eligible { units: usize },
    Unsupported,
    Failed { message: String },
    Interrupted,
}

pub(super) struct Draft {
    pub target: FieldRenameTarget,
    pub proposed: String,
    pub revision: u64,
    pub phase: Phase,
    pub observation: Observation,
    pub focus: bool,
}

#[derive(Default)]
pub(in crate::workspace) struct Model {
    pub(super) draft: Option<Draft>,
    pub(super) status: Option<Status<FieldRenameSaveIdentity>>,
    pub(super) warning: Option<super::warning::Warning>,
    pub(super) close: Option<super::close::Confirmation>,
}

impl Model {
    /// All three owned strings must be admitted by State before installation.
    pub(super) fn begin(&mut self, target: FieldRenameTarget, proposed: String) -> bool {
        if self.draft.is_some()
            || self.status.is_some()
            || self.warning.is_some()
            || self.close.is_some()
            || !target.bounded()
            || proposed != target.column
            || !bounded_name(&proposed)
        {
            return false;
        }

        self.draft = Some(Draft {
            target,
            proposed,
            revision: 0,
            phase: Phase::Naming,
            observation: Observation::Pending { request_id: None },
            focus: true,
        });

        true
    }

    #[cfg(test)]
    pub(super) fn bind_inspection(&mut self, target: &FieldRenameTarget, request_id: u64) -> bool {
        let Some(draft) = &mut self.draft else {
            return false;
        };
        if draft.target != *target
            || !matches!(draft.observation, Observation::Pending { request_id: None })
        {
            return false;
        }

        draft.observation = Observation::Pending {
            request_id: Some(request_id),
        };

        true
    }

    pub(super) fn inspection_matches(&self, target: &FieldRenameTarget, request_id: u64) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.target == *target
                && matches!(draft.observation, Observation::Pending { request_id: Some(id) } if id == request_id)
        })
    }

    pub(super) fn observed(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
        source_eligible: bool,
        units: Option<usize>,
    ) -> bool {
        self.inspection_matches(target, request_id)
            && self.observed_by_id(request_id, source_eligible, units)
    }

    pub(super) fn observed_by_id(
        &mut self,
        request_id: u64,
        source_eligible: bool,
        units: Option<usize>,
    ) -> bool {
        let Some(draft) = self.pending_mut(request_id) else {
            return false;
        };
        if source_eligible != units.is_some()
            || units.is_some_and(|units| {
                !draft.target.column.is_ascii() || units != draft.target.column.len() || units == 0
            })
        {
            return false;
        }

        draft.observation = match units {
            Some(units) => Observation::Eligible { units },
            None => Observation::Unsupported,
        };

        true
    }

    pub(super) fn pending_mut(&mut self, request_id: u64) -> Option<&mut Draft> {
        self.draft.as_mut().filter(|draft| {
            matches!(draft.observation, Observation::Pending { request_id: Some(id) } if id == request_id)
        })
    }

    /// Call only after the error String's actual capacity is admitted by State.
    pub(super) fn inspection_failed(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.inspection_matches(target, request_id) {
            return false;
        }

        self.draft.as_mut().unwrap().observation = Observation::Failed { message };

        true
    }

    pub(super) fn inspection_interrupted(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
    ) -> bool {
        if !self.inspection_matches(target, request_id) {
            return false;
        }

        self.draft.as_mut().unwrap().observation = Observation::Interrupted;

        true
    }

    pub(super) fn retry_inspection(&mut self, target: &FieldRenameTarget) -> bool {
        let Some(draft) = &mut self.draft else {
            return false;
        };
        if draft.target != *target
            || draft.phase != Phase::Naming
            || matches!(draft.observation, Observation::Pending { .. })
        {
            return false;
        }

        draft.observation = Observation::Pending { request_id: None };

        true
    }

    pub(super) fn replace(&mut self, target: &FieldRenameTarget, proposed: String) -> bool {
        let Some(draft) = &mut self.draft else {
            return false;
        };
        if draft.target != *target || draft.phase != Phase::Naming || !bounded_name(&proposed) {
            return false;
        }
        if draft.proposed != proposed {
            let Some(revision) = draft.revision.checked_add(1) else {
                return false;
            };
            draft.proposed = proposed;
            draft.revision = revision;
        }

        true
    }

    pub(super) fn profile_fits(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            matches!(draft.observation, Observation::Eligible { units }
                if draft.proposed.is_ascii() && draft.proposed.len() == units)
        })
    }

    pub(super) fn dirty(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.proposed != draft.target.column)
    }

    pub(super) fn matches(&self, request: &FieldRenameRequest, phase: Phase) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.target == request.target
                && draft.revision == request.draft_revision
                && draft.proposed == request.new_name
                && draft.phase == phase
        })
    }

    pub(super) fn begin_review(&mut self, request: &FieldRenameRequest, request_id: u64) -> bool {
        if !self.profile_fits() || !self.matches(request, Phase::Naming) {
            return false;
        }

        self.draft.as_mut().unwrap().phase = Phase::Reviewing { request_id };

        true
    }

    pub(super) fn reviewed(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.matches(request, Phase::Reviewing { request_id }) {
            return false;
        }

        self.draft.as_mut().unwrap().phase = Phase::Reviewed {
            request_id,
            review_id,
        };

        true
    }

    pub(super) fn release_review(&mut self, request: &FieldRenameRequest, request_id: u64) -> bool {
        if !self.matches(request, Phase::Reviewing { request_id }) {
            return false;
        }

        self.draft.as_mut().unwrap().phase = Phase::Naming;

        true
    }

    pub(super) fn release_accepted(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.matches(
            request,
            Phase::Reviewed {
                request_id,
                review_id,
            },
        ) {
            return false;
        }

        self.draft.as_mut().unwrap().phase = Phase::Naming;

        true
    }

    pub(super) fn discard(&mut self, target: &FieldRenameTarget, revision: u64) -> bool {
        if self.draft.is_none() {
            return self
                .warning
                .as_mut()
                .is_some_and(|warning| warning.discard(target, revision));
        }

        if !self.draft.as_ref().is_some_and(|draft| {
            draft.target == *target && draft.revision == revision && draft.phase == Phase::Naming
        }) {
            return false;
        }

        self.draft = None;

        true
    }

    pub(super) fn presented(&self) -> Option<&Draft> {
        self.draft.as_ref().or_else(|| {
            self.warning
                .as_ref()
                .and_then(|warning| warning.presented())
        })
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
            bytes = bytes.checked_add(warning.heap_bytes()?)?;
        }

        if let Some(close) = &self.close {
            bytes = bytes.checked_add(close.target.heap_bytes()?)?;
        }

        Some(bytes)
    }

    #[cfg(test)]
    pub(super) fn heap_bytes(&self) -> Option<usize> {
        self.retained_bytes()?.checked_sub(size_of::<Self>())
    }
}

impl Draft {
    pub(super) fn heap_bytes(&self) -> Option<usize> {
        self.target
            .heap_bytes()?
            .checked_add(self.proposed.capacity())?
            .checked_add(match &self.observation {
                Observation::Failed { message } => message.capacity(),
                _ => 0,
            })
    }
}
