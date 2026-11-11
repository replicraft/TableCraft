//! Original schema capabilities retain their distinct native reopen witnesses.
use super::{Desktop, PendingOperation, ReviewedAction};
use crate::workspace::{
    editing::{SaveOwner, cleanup_notice},
    filtering::RestoreFilter,
    request_read,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tablecraft_runtime::{
    Command, CreatedTableReopen, FieldDropReview, FieldDroppedReopen, ReviewedFieldDrop,
    ReviewedTableCreation, ReviewedTableDrop, SaveOutcome, SaveWorker, TableDropReview,
    TableDroppedReopen,
};
use tablecraft_ui::{
    Database, QueryActionEffect, QueryActionReview, QuerySchemaEffect, QuerySchemaPublication,
    SaveCleanupNotice,
};

pub(super) enum ReviewedSchema {
    Create {
        capability: ReviewedTableCreation,
        witness: Arc<CreatedTableReopen>,
    },
    DropField {
        capability: ReviewedFieldDrop,
        witness: Arc<FieldDroppedReopen>,
    },
    DropTable {
        capability: ReviewedTableDrop,
        witness: Arc<TableDroppedReopen>,
    },
}

pub(super) enum Reopen {
    Created(Arc<CreatedTableReopen>),
    FieldDropped(Arc<FieldDroppedReopen>),
    TableDropped(Arc<TableDroppedReopen>),
}

impl ReviewedSchema {
    pub(super) fn create(capability: ReviewedTableCreation) -> Option<Self> {
        let witness = capability.reopen_witness();
        // A design-draft capability cannot be relabeled as a SQL creation.
        if witness.draft_revision().is_some() {
            return None;
        }
        Some(Self::Create {
            capability,
            witness,
        })
    }

    pub(super) fn drop_field(capability: ReviewedFieldDrop) -> Self {
        let witness = capability.reopen_witness();
        Self::DropField {
            capability,
            witness,
        }
    }

    pub(super) fn drop_table(capability: ReviewedTableDrop) -> Self {
        let witness = capability.reopen_witness();
        Self::DropTable {
            capability,
            witness,
        }
    }

    pub(super) fn document_revision(&self) -> u64 {
        match self {
            Self::Create { witness, .. } => witness.document_revision(),
            Self::DropField { witness, .. } => witness.document_revision(),
            Self::DropTable { witness, .. } => witness.document_revision(),
        }
    }

    pub(super) fn source_path(&self) -> &Path {
        match self {
            Self::Create { witness, .. } => witness.source_path(),
            Self::DropField { witness, .. } => witness.source_path(),
            Self::DropTable { witness, .. } => witness.source_path(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Create { witness, .. } => witness.table_name(),
            Self::DropField { witness, .. } => witness.table_name(),
            Self::DropTable { witness, .. } => witness.table_name(),
        }
    }

    pub(super) fn start(
        self,
        worker: &SaveWorker,
        bundle: PathBuf,
    ) -> Result<(u64, Reopen), (String, Option<Self>)> {
        match self {
            Self::Create {
                capability,
                witness,
            } => match worker.start_table_creation(capability, bundle) {
                Ok(id) => Ok((id, Reopen::Created(witness))),
                Err(error) => {
                    let message = error.to_string();
                    let original = error.into_reviewed().map(|capability| Self::Create {
                        capability,
                        witness,
                    });
                    Err((message, original))
                }
            },
            Self::DropField {
                capability,
                witness,
            } => match worker.start_field_drop(capability, bundle) {
                Ok(id) => Ok((id, Reopen::FieldDropped(witness))),
                Err(error) => {
                    let message = error.to_string();
                    let original = error.into_reviewed().map(|capability| Self::DropField {
                        capability,
                        witness,
                    });
                    Err((message, original))
                }
            },
            Self::DropTable {
                capability,
                witness,
            } => match worker.start_table_drop(capability, bundle) {
                Ok(id) => Ok((id, Reopen::TableDropped(witness))),
                Err(error) => {
                    let message = error.to_string();
                    let original = error.into_reviewed().map(|capability| Self::DropTable {
                        capability,
                        witness,
                    });
                    Err((message, original))
                }
            },
        }
    }
}

impl Reopen {
    pub(super) fn replacement_known(&self, outcome: SaveOutcome) -> bool {
        match self {
            Self::Created(witness) => outcome
                .table_creation_reopen_facts(witness.as_ref())
                .is_ok_and(|facts| facts.replacement),
            Self::FieldDropped(_) => outcome
                .field_drop_reopen_facts()
                .is_ok_and(|facts| facts.replacement),
            Self::TableDropped(_) => outcome
                .table_drop_reopen_facts()
                .is_ok_and(|facts| facts.replacement),
        }
    }

    pub(super) fn command(&self) -> Command {
        // NO_CLONE: The retained read-only witness survives a failed guarded open;
        // the independent worker command shares its exact immutable native proof.
        match self {
            Self::Created(witness) => Command::OpenTableCreated {
                witness: Arc::clone(witness),
            },
            Self::FieldDropped(witness) => Command::OpenFieldDropped {
                witness: Arc::clone(witness),
            },
            Self::TableDropped(witness) => Command::OpenTableDropped {
                witness: Arc::clone(witness),
            },
        }
    }
}

pub(super) enum Publication {
    Rows,
    Schema(Reopen),
}

pub(super) struct Host {
    reopen: Reopen,
    // This native owner has no fresh-read publication context until its actor is
    // acknowledged. The context then survives guarded-open failure and retry.
    read: Option<AfterRead>,
}

struct AfterRead {
    publication: QuerySchemaPublication,
    request: ReadRequest,
    cleanup: Option<SaveCleanupNotice>,
    filters: Vec<RestoreFilter>,
}

impl Host {
    pub(super) fn saving(reopen: Reopen) -> Self {
        Self { reopen, read: None }
    }

    pub(super) fn replacement_known(&self, outcome: SaveOutcome) -> bool {
        self.reopen.replacement_known(outcome)
    }
}

pub(super) enum ReadRequest {
    Running(u64),
    Failed,
}

impl Desktop {
    /// Return true only when this acknowledgment belongs to a real schema save.
    pub(in crate::workspace) fn acknowledge_query_schema(&mut self, id: u64) -> bool {
        let Some(host) = &self.editing.query_schema else {
            return false;
        };
        if host.read.is_some() {
            return false;
        }
        let Some(save) = self
            .editing
            .save
            .as_ref()
            .filter(|save| save.id == id && matches!(save.owner, SaveOwner::QueryAction))
        else {
            return true;
        };
        if !self.state.can_acknowledge_query_action_save(id) {
            return true;
        }
        let Some(report) = save.report else {
            return true;
        };
        if !host.reopen.replacement_known(report.outcome) {
            // Existing failed/original acknowledgment retains its normal conservative
            // source handling. No schema AFTER witness has been qualified here.
            self.editing.query_schema = None;
            return false;
        }
        let cleanup = match report.outcome {
            SaveOutcome::Saved { receipt } => cleanup_notice(receipt),
            _ => None,
        };
        let Some(filters) = self.capture_filter_restore_definitions() else {
            return true;
        };
        if !self.state.can_take_query_schema_publication(id) {
            return true;
        }
        if !self.editing.worker.acknowledge(id) {
            return true;
        }
        let Some(publication) = self.state.take_query_schema_publication(id) else {
            self.state.error = Some(self.state.localizer.text("objects-restore-refused"));
            return true;
        };
        let Some(mut host) = self.editing.query_schema.take() else {
            return true;
        };
        self.editing.save = None;
        host.read = Some(AfterRead {
            publication,
            request: ReadRequest::Failed,
            cleanup,
            filters,
        });
        self.editing.query_schema = Some(host);
        self.retry_query_schema_reopen();
        true
    }

    pub(in crate::workspace) fn retry_query_schema_reopen(&mut self) {
        let Some(host) = &self.editing.query_schema else {
            return;
        };
        if !matches!(
            host.read.as_ref().map(|read| &read.request),
            Some(ReadRequest::Failed)
        ) {
            return;
        }
        let command = host.reopen.command();
        let Some(id) = request_read(command, &self.worker, &mut self.state) else {
            self.state.error = Some(
                self.state
                    .localizer
                    .text("objects-read-identities-exhausted"),
            );
            return;
        };
        if let Some(read) = self
            .editing
            .query_schema
            .as_mut()
            .and_then(|host| host.read.as_mut())
        {
            read.request = ReadRequest::Running(id);
        }
        self.generation = id;
        self.pending = PendingOperation::QuerySchemaReopen { request_id: id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn fail_query_schema_reopen(&mut self, id: u64, message: String) {
        let Some(read) = self
            .editing
            .query_schema
            .as_mut()
            .and_then(|host| host.read.as_mut())
        else {
            return;
        };
        if !matches!(read.request, ReadRequest::Running(expected) if expected == id) {
            return;
        }
        read.request = ReadRequest::Failed;
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        self.state.error = Some(message);
    }

    pub(in crate::workspace) fn finish_query_schema_reopen(
        &mut self,
        id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(read) = self
            .editing
            .query_schema
            .as_ref()
            .and_then(|host| host.read.as_ref())
        else {
            return;
        };
        if !matches!(read.request, ReadRequest::Running(expected) if expected == id) {
            return;
        }
        let revision = database.document_revision;
        if !self
            .state
            .resolve_query_schema_after_reopen(&read.publication, database)
        {
            self.fail_query_schema_reopen(id, self.state.localizer.text("objects-restore-refused"));
            return;
        }
        let Some(AfterRead {
            cleanup, filters, ..
        }) = self.editing.query_schema.take().and_then(|host| host.read)
        else {
            return;
        };
        self.retain_filter_restore_definitions(filters);
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, cleanup);
        self.clear_query_parameters();
        self.editing.clear_review();
        self.state.busy = false;
        self.state.error = None;
        self.pending = PendingOperation::Read;
        // SQL remains the active source-neutral tab; never page a removed table
        // or silently rerun the statement after successful guarded publication.
    }
}

impl ReviewedSchema {
    pub(super) fn matches_effect(&self, effect: &QueryActionEffect) -> bool {
        match (self, effect) {
            (
                Self::Create { witness, .. },
                QueryActionEffect::Schema(QuerySchemaEffect::CreateTable { column_count }),
            ) => witness.column_count() == *column_count,
            (
                Self::DropField { witness, .. },
                QueryActionEffect::Schema(QuerySchemaEffect::DropColumn {
                    column,
                    native_number,
                }),
            ) => {
                witness.original_column_name() == column
                    && witness.native_number() == *native_number
            }
            (
                Self::DropTable { .. },
                QueryActionEffect::Schema(QuerySchemaEffect::DropTable { .. }),
            ) => true,
            _ => false,
        }
    }
}

impl Desktop {
    pub(in crate::workspace) fn publish_query_schema_creation(
        &mut self,
        request_id: u64,
        capability: ReviewedTableCreation,
    ) {
        let id = capability.id();
        let Some(capability) = ReviewedSchema::create(capability) else {
            return;
        };
        let mut table = String::new();
        if table.try_reserve_exact(capability.table().len()).is_err() {
            self.fail_query_action_review(
                request_id,
                self.state.localizer.text("objects-memory-limit"),
            );
            return;
        }
        // NO_CLONE: UI review text has an independent lifetime from the native
        // capability's immutable reopen witness; it cannot borrow that owner.
        table.push_str(capability.table());
        let ReviewedSchema::Create { witness, .. } = &capability else {
            return;
        };
        let review = QueryActionReview {
            id,
            document_revision: witness.document_revision(),
            table,
            effect: QueryActionEffect::Schema(QuerySchemaEffect::CreateTable {
                column_count: witness.column_count(),
            }),
        };
        self.publish_query_action_review(request_id, review, ReviewedAction::Schema(capability));
    }

    pub(in crate::workspace) fn publish_query_schema_field_drop(
        &mut self,
        request_id: u64,
        review: FieldDropReview,
        capability: ReviewedFieldDrop,
    ) {
        if review.table != capability.captured_table()
            || review.column != capability.captured_column()
            || review.native_number != capability.captured_native_number()
        {
            return;
        }
        self.publish_query_action_review(
            request_id,
            QueryActionReview {
                id: review.id,
                document_revision: review.document_revision,
                table: review.table,
                effect: QueryActionEffect::Schema(QuerySchemaEffect::DropColumn {
                    column: review.column,
                    native_number: review.native_number,
                }),
            },
            ReviewedAction::Schema(ReviewedSchema::drop_field(capability)),
        );
    }

    pub(in crate::workspace) fn publish_query_schema_table_drop(
        &mut self,
        request_id: u64,
        review: TableDropReview,
        capability: ReviewedTableDrop,
    ) {
        if review.table != capability.captured_table()
            || review.native_id != capability.captured_native_id()
        {
            return;
        }
        self.publish_query_action_review(
            request_id,
            QueryActionReview {
                id: review.id,
                document_revision: review.document_revision,
                table: review.table,
                effect: QueryActionEffect::Schema(QuerySchemaEffect::DropTable {
                    declared_rows: review.declared_rows,
                }),
            },
            ReviewedAction::Schema(ReviewedSchema::drop_table(capability)),
        );
    }
}
