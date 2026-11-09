//! Provisional close workflow; no retained Access SQL-close capture exists.
mod modal;

use super::{Draft, SavedQuerySqlEligibility, SavedQuerySqlRequest};
use crate::workspace::{ObjectTab, State, objects::memory::Counter, query::parameters::copy_text};

/// User decision only; native capabilities and host transitions stay outside UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedQuerySqlCloseDecision {
    Save,
    Discard,
    Cancel,
}

/// Exact object/text bound to the choice. Only Save carries the complete SQL.
/// A host retains any deferred path/window transition independently.
pub struct SavedQuerySqlCloseResult {
    pub decision: SavedQuerySqlCloseDecision,
    pub tab: ObjectTab,
    pub draft_revision: u64,
    pub request: Option<SavedQuerySqlRequest>,
}

pub(in crate::workspace) struct Confirmation {
    tab: ObjectTab,
    draft_revision: u64,
}

impl Confirmation {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let crate::workspace::ObjectKind::SavedQuery { name, .. } = &self.tab.target.kind {
            count.string(name);
        }
        count.finish()
    }
}

impl Draft {
    pub(in crate::workspace) fn close_save_eligible(&self) -> bool {
        self.dirty()
            && self.eligibility == SavedQuerySqlEligibility::QualifiedExpressionSelect
            && self.original.is_some()
            && self.pending.is_none()
    }
}

impl State {
    pub fn saved_query_sql_close_open(&self) -> bool {
        self.saved_query_sql_close.is_some()
    }

    /// Open confirmation without activating or discarding a parked query.
    pub fn request_saved_query_sql_close(&mut self, tab: &ObjectTab) -> bool {
        if let Some(existing) = &self.saved_query_sql_close {
            return existing.tab == *tab
                && self.saved_query_sql_revision_for(tab) == Some(existing.draft_revision);
        }
        if !self.can_close_object(tab) {
            return false;
        }
        let Some(draft_revision) = self.saved_query_sql_revision_for(tab) else {
            return false;
        };
        let crate::workspace::ObjectKind::SavedQuery { object_id, name } = &tab.target.kind else {
            return false;
        };
        let Some(name) = copy_text(name) else {
            return false;
        };
        let copied = ObjectTab {
            id: tab.id,
            target: crate::workspace::ObjectTarget {
                document_revision: tab.target.document_revision,
                kind: crate::workspace::ObjectKind::SavedQuery {
                    object_id: *object_id,
                    name,
                },
            },
        };
        let confirmation = Confirmation {
            tab: copied,
            draft_revision,
        };
        if !self.admit_object_bytes(confirmation.retained_bytes()) {
            return false;
        }
        self.interrupt_saved_sql_for(tab);
        self.saved_query_sql_close = Some(confirmation);
        true
    }

    /// A host may call this repeatedly while retaining its own deferred action.
    pub fn request_saved_query_sql_close_all(&mut self) -> bool {
        let Some(tab) = self
            .object_tabs()
            .into_iter()
            .find(|tab| self.saved_query_sql_revision_for(tab).is_some())
        else {
            return false;
        };
        self.request_saved_query_sql_close(&tab)
    }

    fn saved_query_sql_close_current(&self) -> bool {
        self.saved_query_sql_close
            .as_ref()
            .is_some_and(|confirmation| {
                let crate::workspace::ObjectKind::SavedQuery { object_id, name } =
                    &confirmation.tab.target.kind
                else {
                    return false;
                };
                self.database.as_ref().is_some_and(|database| {
                    database.document_revision == confirmation.tab.target.document_revision
                        && database
                            .queries
                            .iter()
                            .any(|query| query.object_id == *object_id && query.name == *name)
                }) && self.saved_query_sql_revision_for(&confirmation.tab)
                    == Some(confirmation.draft_revision)
            })
    }

    pub fn can_save_saved_query_sql_close(&self) -> bool {
        self.saved_query_sql_close
            .as_ref()
            .is_some_and(|confirmation| {
                !self.busy
                    && self.saved_query_sql_edit_available
                    && self.saved_query_sql_close_current()
                    && !self.has_unsaved_table_drafts()
                    && self.saved_query_sql_revision_for(&confirmation.tab)
                        == Some(confirmation.draft_revision)
                    && self.saved_sql_close_eligible(&confirmation.tab)
            })
    }

    /// Stale Save/Discard choices leave every original/proposed string intact.
    /// Cancel always dismisses this presentation confirmation without mutation.
    pub fn choose_saved_query_sql_close(
        &mut self,
        decision: SavedQuerySqlCloseDecision,
    ) -> Option<SavedQuerySqlCloseResult> {
        self.saved_query_sql_close.as_ref()?;
        if decision != SavedQuerySqlCloseDecision::Cancel
            && (!self.saved_query_sql_close_current()
                || (decision == SavedQuerySqlCloseDecision::Save
                    && !self.can_save_saved_query_sql_close()))
        {
            return None;
        }
        let confirmation = self.saved_query_sql_close.take()?;
        let request = if decision == SavedQuerySqlCloseDecision::Save {
            if !self.activate_object(&confirmation.tab) {
                self.saved_query_sql_close = Some(confirmation);
                return None;
            }
            let request = self.saved_query_sql_review_request();
            if request.is_none() {
                self.saved_query_sql_close = Some(confirmation);
                return None;
            }
            request
        } else {
            if decision == SavedQuerySqlCloseDecision::Discard
                && !self.discard_saved_query_sql(&confirmation.tab, confirmation.draft_revision)
            {
                self.saved_query_sql_close = Some(confirmation);
                return None;
            }
            None
        };
        Some(SavedQuerySqlCloseResult {
            decision,
            tab: confirmation.tab,
            draft_revision: confirmation.draft_revision,
            request,
        })
    }
}

#[cfg(test)]
mod tests;
