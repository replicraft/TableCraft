//! Shared mechanical native actor ownership for real field-schema consumers.
//! Request admission, operation scope and atomic publication stay with each feature.
use super::SaveOwner;
use crate::workspace::request_read;
use crate::workspace::{Desktop, filtering::RestoreFilter};
use std::sync::Arc;
use tablecraft_runtime::{
    AddedColumnReopen, Command, FieldDroppedReopen, FieldRenamedReopen, SaveOutcome, SaveReceipt,
    SaveReport,
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
    Drop,
}

pub(super) struct FieldReplacementFacts {
    pub(super) replacement: bool,
    pub(super) completed: Option<SaveReceipt>,
}

impl FieldOperation {
    fn owns(self, owner: SaveOwner) -> bool {
        matches!(
            (self, owner),
            (Self::Addition, SaveOwner::FieldAddition)
                | (Self::Rename, SaveOwner::FieldRename)
                | (Self::Drop, SaveOwner::FieldDrop)
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
            Self::Drop => {
                outcome
                    .field_drop_reopen_facts()
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

    /// Each caller has already published its distinct full-image checked graph.
    /// Preserve user filter definitions; fresh-schema revalidation owns failures.
    pub(super) fn finish_field_graph_reopen(
        &mut self,
        operation: FieldOperation,
        revision: u64,
        table_creation_available: bool,
        table_design_available: bool,
        cleanup: Option<SaveCleanupNotice>,
        filters: Vec<RestoreFilter>,
    ) {
        self.retain_filter_restore_definitions(filters);
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, cleanup);
        self.state.busy = false;
        self.pending = crate::workspace::PendingOperation::Read;
        match operation {
            FieldOperation::Addition => self.editing.field_addition = None,
            FieldOperation::Rename => self.editing.field_rename = None,
            FieldOperation::Drop => self.editing.field_drop = None,
        }
        self.resume_after_field_schema = true;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    /// Issue only a real typed read witness. The operation has already admitted
    /// its exact native/no-op boundary; generic submit remains protected.
    pub(super) fn issue_field_witness_read(&mut self, witness: FieldReadWitness) -> Option<u64> {
        let command = match witness {
            FieldReadWitness::Addition(witness) => Command::OpenColumnAdded { witness },
            FieldReadWitness::Rename(witness) => Command::OpenFieldRenamed { witness },
            FieldReadWitness::Drop(witness) => Command::OpenFieldDropped { witness },
        };
        request_read(command, &self.worker, &mut self.state)
    }
}

pub(super) enum FieldReadWitness {
    Addition(Arc<AddedColumnReopen>),
    Rename(Arc<FieldRenamedReopen>),
    Drop(Arc<FieldDroppedReopen>),
}
