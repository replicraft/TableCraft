//! Resource-free schema-save presentation shared by distinct operation owners.
//! The generic identity keeps request/witness scopes distinct; facts grant no authority.
use super::EditSaveOutcome;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::workspace) enum StatusPhase {
    Saving { cancelled: bool, resolving: bool },
    Finished,
    Reopening { cancelled: bool },
    ReopenFailed,
}

pub(in crate::workspace) struct Status<I> {
    pub(in crate::workspace) identity: I,
    pub(in crate::workspace) phase: StatusPhase,
    pub(in crate::workspace) outcome: Option<EditSaveOutcome>,
    pub(in crate::workspace) error: Option<String>,
    pub(in crate::workspace) omitted: bool,
}

impl<I> Status<I> {
    pub(in crate::workspace) fn heap_bytes(&self) -> Option<usize> {
        self.outcome
            .as_ref()
            .map_or(0, outcome_bytes)
            .checked_add(self.error.as_ref().map_or(0, String::capacity))
    }
}

pub(in crate::workspace) fn outcome_bytes(outcome: &EditSaveOutcome) -> usize {
    match outcome {
        EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Failed(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Retained { message, .. } => message.capacity(),
    }
}

impl<I: Copy + PartialEq> Status<I> {
    pub(in crate::workspace) fn accepts_outcome(&self, identity: I) -> bool {
        self.identity == identity && matches!(self.phase, StatusPhase::Saving { .. })
    }

    pub(in crate::workspace) fn record_outcome(
        &mut self,
        mut outcome: EditSaveOutcome,
        admitted: bool,
    ) {
        if !admitted {
            match &mut outcome {
                EditSaveOutcome::Saved(message)
                | EditSaveOutcome::Failed(message)
                | EditSaveOutcome::Cancelled(message)
                | EditSaveOutcome::Retained { message, .. } => *message = String::new(),
            }
        }
        self.omitted = !admitted;
        self.outcome = Some(outcome);
        self.phase = StatusPhase::Finished;
    }

    pub(in crate::workspace) fn can_resolve(&self, identity: I, recover: bool) -> bool {
        if self.identity != identity || self.phase != StatusPhase::Finished {
            return false;
        }
        let Some(EditSaveOutcome::Retained {
            can_recover,
            can_release,
            ..
        }) = self.outcome.as_ref()
        else {
            return false;
        };

        if recover {
            return *can_recover;
        }
        *can_release
    }

    pub(in crate::workspace) fn can_acknowledge(&self, identity: I) -> bool {
        self.identity == identity
            && self.phase == StatusPhase::Finished
            && self
                .outcome
                .as_ref()
                .is_some_and(|outcome| !matches!(outcome, EditSaveOutcome::Retained { .. }))
    }

    pub(in crate::workspace) fn begin_reopen(&mut self, identity: I) -> bool {
        if self.identity != identity
            || (self.phase != StatusPhase::Finished && self.phase != StatusPhase::ReopenFailed)
        {
            return false;
        }
        self.phase = StatusPhase::Reopening { cancelled: false };
        self.error = None;
        true
    }

    pub(in crate::workspace) fn reopening(&self, identity: I) -> bool {
        self.identity == identity && matches!(self.phase, StatusPhase::Reopening { .. })
    }

    pub(in crate::workspace) fn record_reopen_failure(&mut self, message: String, admitted: bool) {
        self.phase = StatusPhase::ReopenFailed;
        self.error = Some(if admitted { message } else { String::new() });
        self.omitted |= !admitted;
    }

    pub(in crate::workspace) fn can_retry_reopen(&self, identity: I) -> bool {
        self.identity == identity && self.phase == StatusPhase::ReopenFailed
    }

    pub(in crate::workspace) fn cancel(&mut self, identity: I) -> bool {
        if self.identity != identity {
            return false;
        }
        match &mut self.phase {
            StatusPhase::Saving { cancelled, .. } | StatusPhase::Reopening { cancelled }
                if !*cancelled =>
            {
                *cancelled = true;
                true
            }
            _ => false,
        }
    }
}

/// The same allocation-free phase/status install and abandoned-start rollback.
/// Operation-specific admission and runtime capabilities remain with each owner.
pub(in crate::workspace) struct Preparation<'a, I, P: Copy> {
    phase: &'a mut P,
    status: &'a mut Option<Status<I>>,
    fallback: P,
    bound: bool,
}

impl<'a, I, P: Copy> Preparation<'a, I, P> {
    pub(in crate::workspace) fn new(
        phase: &'a mut P,
        status: &'a mut Option<Status<I>>,
        fallback: P,
    ) -> Self {
        Self {
            phase,
            status,
            fallback,
            bound: false,
        }
    }

    pub(in crate::workspace) fn install(
        &mut self,
        identity: I,
        owned_phase: P,
        status_phase: StatusPhase,
    ) {
        *self.phase = owned_phase;
        *self.status = Some(Status {
            identity,
            phase: status_phase,
            outcome: None,
            error: None,
            omitted: false,
        });
        self.bound = true;
    }
}

impl<I, P: Copy> Drop for Preparation<'_, I, P> {
    fn drop(&mut self) {
        if !self.bound {
            *self.phase = self.fallback;
        }
    }
}
