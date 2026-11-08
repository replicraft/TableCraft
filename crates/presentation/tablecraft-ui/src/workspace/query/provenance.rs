//! Submitted text and result provenance are presentation-owned, never executable authority.
use crate::workspace::{Data, ObjectKind, ObjectTab, State};

#[derive(Default)]
pub(in crate::workspace) enum ResultOrigin {
    #[default]
    Unknown,
    Sql(String),
    Design(String),
}

impl ResultOrigin {
    pub(in crate::workspace) fn retained_bytes(&self) -> usize {
        match self {
            Self::Unknown => 0,
            Self::Sql(sql) | Self::Design(sql) => sql.capacity(),
        }
    }
}

pub(in crate::workspace) struct PendingSql {
    pub(in crate::workspace) sql: String,
    request_id: u64,
    object: ObjectTab,
}

impl State {
    fn source_bound_sql_object(&self) -> Option<ObjectTab> {
        self.active_object().filter(|object| {
            object.target.kind == ObjectKind::Sql
                && self.database.as_ref().is_some_and(|database| {
                    object.target.document_revision == database.document_revision
                })
        })
    }

    /// Checked edit witness for scratch inspection and parameter acquisition.
    /// Executed-result provenance retains its separate submitted-SQL contract.
    pub fn query_sql_revision(&self) -> Option<u64> {
        self.query_sql_revision
            .checked_add(1)
            .map(|_| self.query_sql_revision)
    }

    /// A text witness is current only for a source-bound scratch object.
    /// Callers additionally retain their exact tab/document/publication identity.
    pub fn query_sql_witness_current(&self, revision: u64, sql: &str) -> bool {
        self.query_sql_revision() == Some(revision)
            && self.source_bound_sql_object().is_some()
            && self.sql == sql
    }

    /// Replace the global scratch draft without changing prior result ownership.
    /// Identical text is a no-op; exhausted revisions refuse before mutation.
    pub fn replace_query_sql(&mut self, sql: String) -> bool {
        if self.sql == sql {
            return true;
        }
        let Some(revision) = self.query_sql_revision.checked_add(1) else {
            return false;
        };
        self.clear_query_parameter_acquisition();
        self.close_query_update();
        self.sql = sql;
        self.query_sql_revision = revision;
        true
    }

    pub(super) fn record_query_sql_edit(&mut self) -> bool {
        let Some(revision) = self.query_sql_revision.checked_add(1) else {
            return false;
        };
        self.clear_query_parameter_acquisition();
        self.close_query_update();
        self.query_sql_revision = revision;
        true
    }

    fn reserve_query_sql(&mut self, bytes: usize, copies: usize) -> Option<String> {
        if self.source_bound_sql_object().is_none() || self.query_sql_revision().is_none() {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return None;
        }
        if !self.admit_object_bytes(bytes.checked_mul(copies)) {
            return None;
        }
        let mut captured = String::new();
        if captured.try_reserve_exact(bytes).is_err() {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        }
        self.admit_object_bytes(captured.capacity().checked_mul(copies))
            .then_some(captured)
    }

    /// Capture caller-owned submitted SQL under retained UI limits.
    pub fn capture_query_sql(&mut self, sql: &str) -> Option<String> {
        let mut captured = self.reserve_query_sql(sql.len(), 1)?;
        captured.push_str(sql);
        Some(captured)
    }

    /// Preflight the GUI command copy and pending witness before cloning a draft.
    pub fn capture_query_draft(&mut self) -> Option<String> {
        let mut captured = self.reserve_query_sql(self.sql.len(), 2)?;
        captured.push_str(&self.sql);
        Some(captured)
    }

    /// Associate submitted text with this exact source-bound SQL object.
    pub fn begin_query_run(&mut self, request_id: u64, sql: String) -> bool {
        let Some(object) = self.source_bound_sql_object().filter(|_| request_id != 0) else {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return false;
        };
        if self.query_sql_revision().is_none() || !self.admit_object_bytes(Some(sql.capacity())) {
            return false;
        }
        self.query_pending = Some(PendingSql {
            sql,
            request_id,
            object,
        });
        true
    }

    /// Superseding/cancelling work releases only its unpublished SQL witness.
    pub fn clear_query_run(&mut self) {
        self.query_pending = None;
    }

    pub fn set_query_run_data(&mut self, request_id: u64, data: Data) -> bool {
        let current = self.query_pending.as_ref().is_some_and(|pending| {
            pending.request_id == request_id
                && self.active_object().as_ref() == Some(&pending.object)
                && self.database.as_ref().is_some_and(|database| {
                    pending.object.target.document_revision == database.document_revision
                })
        });
        if !current {
            return false;
        }
        let pending = self.query_pending.take().expect("matching scratch run");
        self.set_query_data(Some(pending.sql), data)
    }

    /// Unknown-origin results never assert equivalence to the editable draft.
    pub fn query_results_previous(&self) -> bool {
        self.query_data.is_some()
            && match &self.query_result {
                ResultOrigin::Unknown => true,
                ResultOrigin::Sql(sql) | ResultOrigin::Design(sql) => {
                    sql != &self.sql
                        || self.busy
                        || self.query_pending.is_some()
                        || self.query_parameters.is_some()
                        || self.error.is_some()
                        || (matches!(self.query_result, ResultOrigin::Design(_))
                            && self.query_design_results_previous())
                }
            }
    }
}
