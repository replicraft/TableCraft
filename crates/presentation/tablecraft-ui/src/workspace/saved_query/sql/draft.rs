use super::super::SavedQueryTarget;
use crate::workspace::{objects::memory::Counter, query::parameters::copy_text};

/// Host-reported eligibility observation, without an editing capability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SavedQuerySqlEligibility {
    QualifiedExpressionSelect,
    #[default]
    Unavailable,
}

/// Exact submitted presentation snapshot. A host must acquire native authority.
#[derive(Debug, PartialEq, Eq)]
pub struct SavedQuerySqlRequest {
    pub target: SavedQueryTarget,
    pub draft_revision: u64,
    pub original_sql: Option<String>,
    pub proposed_sql: String,
}

/// Failures never replace the draft's original or proposed SQL.
#[derive(Debug, PartialEq, Eq)]
pub enum SavedQuerySqlError {
    SourceChanged,
    MemoryLimit,
    Host(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Inspection,
    Review,
    Reviewed(u64),
}

pub(super) struct Pending {
    pub(super) request_id: u64,
    pub(super) snapshot: SavedQuerySqlRequest,
    pub(super) phase: Phase,
}

pub(in crate::workspace) struct Draft {
    pub(super) target: SavedQueryTarget,
    pub(super) original: Option<String>,
    pub(super) proposed: String,
    pub(super) revision: u64,
    pub(super) eligibility: SavedQuerySqlEligibility,
    pub(super) pending: Option<Pending>,
    pub(super) error: Option<SavedQuerySqlError>,
}

impl SavedQuerySqlRequest {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.string(&self.target.name);
        count.optional(&self.original_sql);
        count.string(&self.proposed_sql);
        count.finish()
    }
}

impl Draft {
    pub(super) fn new(target: SavedQueryTarget) -> Self {
        Self {
            target,
            original: None,
            proposed: String::new(),
            revision: 0,
            eligibility: SavedQuerySqlEligibility::Unavailable,
            pending: None,
            error: None,
        }
    }

    pub(in crate::workspace) fn target(&self) -> &SavedQueryTarget {
        &self.target
    }

    pub(in crate::workspace) fn dirty(&self) -> bool {
        self.original
            .as_ref()
            .is_some_and(|original| *original != self.proposed)
    }

    pub(in crate::workspace) fn revision(&self) -> u64 {
        self.revision
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.string(&self.target.name);
        count.optional(&self.original);
        count.string(&self.proposed);
        if let Some(pending) = &self.pending {
            count.add(pending.snapshot.retained_bytes()?);
        }
        if let Some(SavedQuerySqlError::Host(error)) = &self.error {
            count.string(error);
        }
        count.finish()
    }

    pub(super) fn snapshot(&self) -> Option<SavedQuerySqlRequest> {
        Some(SavedQuerySqlRequest {
            target: SavedQueryTarget {
                document_revision: self.target.document_revision,
                object_id: self.target.object_id,
                name: copy_text(&self.target.name)?,
            },
            draft_revision: self.revision,
            original_sql: match &self.original {
                Some(sql) => Some(copy_text(sql)?),
                None => None,
            },
            proposed_sql: copy_text(&self.proposed)?,
        })
    }

    pub(super) fn matches(&self, snapshot: &SavedQuerySqlRequest) -> bool {
        self.target == snapshot.target
            && self.revision == snapshot.draft_revision
            && self.original == snapshot.original_sql
            && self.proposed == snapshot.proposed_sql
    }

    pub(super) fn accepts(
        &self,
        request_id: u64,
        snapshot: &SavedQuerySqlRequest,
        phase: Phase,
    ) -> bool {
        self.matches(snapshot)
            && self.pending.as_ref().is_some_and(|pending| {
                pending.request_id == request_id
                    && pending.phase == phase
                    && pending.snapshot == *snapshot
            })
    }

    pub(super) fn replace_text(&mut self, text: String) -> bool {
        if self.proposed == text {
            return true;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        self.proposed = text;
        self.revision = revision;
        self.pending = None;
        self.error = None;
        true
    }

    pub(in crate::workspace) fn interrupt(&mut self) {
        self.pending = None;
    }
}
