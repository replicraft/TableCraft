//! Shared mechanical native actor ownership for real addition and rename consumers.
//! Request admission, operation scope and atomic publication stay with each feature.
use super::SaveOwner;
use crate::workspace::{Desktop, filtering::RestoreFilter};
use std::sync::Arc;
use tablecraft_runtime::{
    AddedColumnReopen, Command, FieldRenamedReopen, SaveOutcome, SaveReceipt, SaveReport,
};
use tablecraft_ui::SaveCleanupNotice;

/// One typed original request with retained native facts; no UI phase machine.
pub(in crate::workspace) struct FieldSaveHost<Request, Witness, Identity> {
    pub(super) request_id: u64,
    pub(super) request: Request,
    pub(super) identity: Option<Identity>,
    pub(super) witness: Option<Arc<Witness>>,
    pub(super) terminal: Option<SaveOutcome>,
    pub(super) cleanup: Option<SaveCleanupNotice>,
    pub(super) filters: Vec<RestoreFilter>,
    pub(super) reopening: bool,
    pub(super) cancelled: bool,
}

/// A real operation selects its own runtime receipt classifier.
#[derive(Clone, Copy)]
pub(super) enum FieldOperation {
    Addition,
    Rename,
}

pub(super) struct FieldReplacementFacts {
    pub(super) replacement: bool,
    pub(super) completed: Option<SaveReceipt>,
}

impl FieldOperation {
    fn owns(self, owner: SaveOwner) -> bool {
        matches!(
            (self, owner),
            (Self::Addition, SaveOwner::FieldAddition) | (Self::Rename, SaveOwner::FieldRename)
        )
    }

    pub(super) fn facts(self, outcome: SaveOutcome) -> Option<FieldReplacementFacts> {
        match self {
            Self::Addition => {
                outcome
                    .field_addition_reopen_facts()
                    .ok()
                    .map(|facts| FieldReplacementFacts {
                        replacement: facts.replacement,
                        completed: facts.completed,
                    })
            }
            Self::Rename => {
                outcome
                    .field_rename_reopen_facts()
                    .ok()
                    .map(|facts| FieldReplacementFacts {
                        replacement: facts.replacement,
                        completed: facts.completed,
                    })
            }
        }
    }
}

impl Desktop {
    /// Require the exact actual actor and report before any ACK composition.
    pub(super) fn field_actor_report(
        &self,
        operation: FieldOperation,
        save_id: u64,
    ) -> Option<SaveReport> {
        self.editing
            .save
            .as_ref()
            .filter(|save| save.id == save_id && operation.owns(save.owner))
            .and_then(|save| save.report)
            .filter(|report| report.id == save_id)
    }

    /// Operation-specific guards and facts must have passed first. The actual
    /// SaveWorker ACK ends retained native ownership; a receipt alone never does.
    pub(super) fn acknowledge_field_actor(
        &mut self,
        operation: FieldOperation,
        save_id: u64,
    ) -> Option<SaveReport> {
        let report = self.field_actor_report(operation, save_id)?;
        if !self.editing.worker.acknowledge(save_id) {
            return None;
        }
        self.editing.save = None;
        self.state.busy = false;
        Some(report)
    }

    /// Issue only a real typed read witness. The operation has already admitted
    /// its exact native/no-op boundary; generic submit remains protected.
    pub(super) fn issue_field_witness_read(&mut self, witness: FieldReadWitness) -> u64 {
        let command = match witness {
            FieldReadWitness::Addition(witness) => Command::OpenColumnAdded { witness },
            FieldReadWitness::Rename(witness) => Command::OpenFieldRenamed { witness },
        };
        self.worker.request(command)
    }
}

pub(super) enum FieldReadWitness {
    Addition(Arc<AddedColumnReopen>),
    Rename(Arc<FieldRenamedReopen>),
}
