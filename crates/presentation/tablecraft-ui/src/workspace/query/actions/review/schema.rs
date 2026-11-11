//! Sealed SQL-origin publication facts; these grant no native write authority.
use super::{Origin, Phase, QueryActionEffect, QueryActionKind, QueryActionReview};
use crate::workspace::{EditSaveOutcome, ObjectKind, State};

/// Exact reviewed schema effect, separate from matched or changed record counts.
#[derive(Debug, PartialEq, Eq)]
pub enum QuerySchemaEffect {
    /// Create the named table with its exact declared number of fields.
    CreateTable { column_count: usize },
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
            Self::CreateTable { column_count } => *column_count > 0,
            Self::DropTable { .. } => true,
            Self::DropColumn { column, .. } => !column.is_empty(),
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        match self {
            Self::DropColumn { column, .. } => column.capacity(),
            Self::CreateTable { .. } | Self::DropTable { .. } => 0,
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
        !self.objects.entries.iter().any(|entry| {
            matches!(&entry.tab.target.kind, ObjectKind::Table { name } if name == &review.table)
        })
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
                self.schema_reopen_revision = Some(review.document_revision);
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
