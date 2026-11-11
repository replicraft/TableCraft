//! Sealed SQL-origin publication facts; these grant no native write authority.
use super::{Origin, Phase, QueryActionEffect, QueryActionKind, QueryActionReview};
use crate::localization::tr;
use crate::workspace::{Action, EditSaveOutcome, State};
use egui::{Frame, Ui};
use tablecraft_builders::table::TypedFieldAdditionDeclaration;

/// Read publication state only; this supplies no native reopen authority.
pub(in crate::workspace) struct QuerySchemaRead {
    pub(in crate::workspace) revision: u64,
    running: bool,
}

/// Exact reviewed schema effect, separate from matched or changed record counts.
#[derive(Debug, PartialEq, Eq)]
pub enum QuerySchemaEffect {
    /// Append one unadorned native field without a gallery policy snapshot.
    AddColumn {
        column: String,
        declaration: TypedFieldAdditionDeclaration,
    },
    /// Create an ordinary nonunique index; source population is not a data delta.
    CreateIndex {
        index: String,
        column: String,
        native_number: usize,
        descending: bool,
        indexed_rows: usize,
    },
    /// Create the named table with its exact declared number of fields.
    CreateTable { column_count: usize },
    /// Create one table and its complete selected scalar rows atomically.
    MakeTable {
        column_count: usize,
        inserted_rows: usize,
    },
    /// Remove the named table; the count is a native table fact, not a row action.
    DropTable { declared_rows: u32 },
    /// Remove one real field without renumbering its surviving native neighbors.
    DropColumn {
        column: String,
        native_number: usize,
    },
}

impl QuerySchemaEffect {
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::AddColumn {
                column,
                declaration,
            } => !column.is_empty() && !declaration.unicode_compression(),
            Self::CreateIndex { index, column, .. } => !index.is_empty() && !column.is_empty(),
            Self::CreateTable { column_count } | Self::MakeTable { column_count, .. } => {
                *column_count > 0
            }
            Self::DropTable { .. } => true,
            Self::DropColumn { column, .. } => !column.is_empty(),
        }
    }

    pub(super) fn retained_bytes(&self) -> Option<usize> {
        match self {
            Self::CreateIndex { index, column, .. } => {
                index.capacity().checked_add(column.capacity())
            }
            Self::AddColumn { column, .. } | Self::DropColumn { column, .. } => {
                Some(column.capacity())
            }
            Self::CreateTable { .. } | Self::MakeTable { .. } | Self::DropTable { .. } => Some(0),
        }
    }
}

/// Issued only by consuming the actual SQL review session after a matching save.
/// A host must still verify its original runtime reopen witness before publishing.
pub struct QuerySchemaPublication {
    origin: Origin,
    review: QueryActionReview,
}

impl QuerySchemaPublication {
    pub(in crate::workspace) fn current(&self, state: &State) -> bool {
        self.origin.current(state)
            && state
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == self.review.document_revision)
    }

    pub(in crate::workspace) fn review(&self) -> &QueryActionReview {
        &self.review
    }
}

impl State {
    pub(in crate::workspace) fn query_schema_target_closed(
        &self,
        review: &QueryActionReview,
    ) -> bool {
        if !matches!(
            &review.effect,
            QueryActionEffect::Schema(
                QuerySchemaEffect::DropTable { .. } | QuerySchemaEffect::DropColumn { .. }
            )
        ) {
            return true;
        }
        !self.table_object_is_open(&review.table)
    }

    /// Source-neutral preflight before the host acknowledges its actual native actor.
    pub fn can_take_query_schema_publication(&self, save_id: u64) -> bool {
        self.can_acknowledge_query_action_save(save_id)
            && self.query_action.as_ref().is_some_and(|session| {
                session.kind == QueryActionKind::Schema
                    && matches!(
                        &session.phase,
                        Phase::Finished {
                            outcome: EditSaveOutcome::Saved(_),
                            ..
                        }
                    )
            })
    }

    /// Move the accepted SQL origin and native-target display facts after terminal
    /// acknowledgment. The host must separately prove known AFTER from the actual
    /// native save actor before requesting a guarded reopen. View changes cannot
    /// prevent release of that actor; they can still refuse later UI publication.
    /// No design request or replacement SQL is fabricated.
    pub fn take_query_schema_publication(
        &mut self,
        save_id: u64,
    ) -> Option<QuerySchemaPublication> {
        if !self.can_take_query_schema_publication(save_id) {
            return None;
        }
        let mut session = self.query_action.take()?;
        match session.phase {
            Phase::Finished { review, .. } => {
                // AFTER is known, but no freshly verified read session is published yet.
                self.schema_reopen = Some(QuerySchemaRead {
                    revision: review.document_revision,
                    running: false,
                });
                Some(QuerySchemaPublication {
                    origin: session.origin,
                    review,
                })
            }
            phase => {
                session.phase = phase;
                self.query_action = Some(session);
                None
            }
        }
    }
}

impl State {
    /// The current reader still describes the original image after a known save.
    /// Retained result viewing is safe; new queries require the guarded AFTER read.
    pub(in crate::workspace) fn schema_after_awaits_reopen(&self) -> bool {
        self.schema_reopen.as_ref().is_some_and(|read| {
            self.database
                .as_ref()
                .is_some_and(|database| database.document_revision == read.revision)
        })
    }

    /// Mark a guarded read running only for this known-AFTER source revision.
    pub fn begin_query_schema_reopen(&mut self, revision: u64) -> bool {
        if self.query_schema_reopen_retry_revision() != Some(revision) {
            return false;
        }
        if let Some(read) = &mut self.schema_reopen {
            read.running = true;
        }
        true
    }

    /// Retain a known-AFTER read failure without granting native write permission.
    pub fn fail_query_schema_reopen(&mut self, revision: u64) {
        if let Some(read) = &mut self.schema_reopen
            && read.revision == revision
        {
            read.running = false;
        }
    }

    /// Revision whose known saved result can be explicitly reopened, never rerun.
    pub fn query_schema_reopen_retry_revision(&self) -> Option<u64> {
        self.schema_reopen
            .as_ref()
            .filter(|read| {
                !read.running
                    && !self.busy
                    && self
                        .database
                        .as_ref()
                        .is_some_and(|database| database.document_revision == read.revision)
            })
            .map(|read| read.revision)
    }

    pub(in crate::workspace) fn query_schema_reopen_status(
        &self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        let Some(read) = &self.schema_reopen else {
            return;
        };
        if self
            .database
            .as_ref()
            .is_none_or(|database| database.document_revision != read.revision)
        {
            return;
        }
        Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.label(tr(
                    ui,
                    if read.running {
                        "objects-loading"
                    } else {
                        "query-action-schema-reopen-failed"
                    },
                ));
                if ui
                    .add_enabled(
                        self.query_schema_reopen_retry_revision().is_some(),
                        egui::Button::new(tr(ui, "query-action-schema-retry-reopen")),
                    )
                    .clicked()
                {
                    actions.push(Action::RetryQuerySchemaReopen {
                        document_revision: read.revision,
                    });
                }
            });
        ui.add_space(10.0);
    }
}
