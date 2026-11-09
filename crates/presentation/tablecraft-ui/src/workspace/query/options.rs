//! Host-supplied read-run preferences; the runtime validates their actual limits.
mod dialog;
#[cfg(test)]
mod tests;

use crate::workspace::{DocumentView, State};

/// Editable presentation values, not permission or a native execution budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryPoolLimits {
    pub bytes: u64,
    pub rows: u64,
    pub steps: u64,
}

/// A host maps these values through its checked read-only runtime constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryRunLimits {
    pub snapshot_rows: u32,
    pub batch_rows: u32,
    pub native: QueryPoolLimits,
    pub query: QueryPoolLimits,
    pub comparison: QueryPoolLimits,
    pub presentation: QueryPoolLimits,
}

#[derive(Default)]
pub(in crate::workspace) struct Model {
    current: Option<QueryRunLimits>,
    draft: Option<QueryRunLimits>,
    error: Option<String>,
    submitted: bool,
}

impl State {
    /// Preferences for a future run; hosts separately capture accepted requests.
    pub fn query_run_limits(&self) -> Option<QueryRunLimits> {
        self.query_limits.current
    }

    /// Initialize from actual runtime defaults or acknowledge a validated edit.
    pub fn set_query_run_limits(&mut self, limits: QueryRunLimits) {
        self.query_limits.current = Some(limits);
        self.query_limits.draft = None;
        self.query_limits.error = None;
        self.query_limits.submitted = false;
    }

    /// Keep rejected edits available for correction without replacing results.
    pub fn reject_query_run_limits(&mut self, error: String) {
        if self.query_limits.draft.is_some() {
            self.query_limits.error = Some(error);
            self.query_limits.submitted = false;
        }
    }

    pub fn query_run_limits_open(&self) -> bool {
        self.query_limits.draft.is_some()
    }

    /// Recheck other operation owners while allowing this preferences dialog.
    pub fn can_change_query_run_limits(&self) -> bool {
        !self.busy
            && !self.database_edit_dialog_open()
            && !self.parameter_prompt_open()
            && !self.query_action_dialog_open()
            && !self.options_dialog_open()
            && !self.zoom_dialog_open()
    }

    /// Open presentation only; no query is inspected, executed or retried.
    pub fn open_query_run_limits(&mut self) -> bool {
        if !self.can_change_query_run_limits()
            || self.query_run_limits_open()
            || self.view != DocumentView::Query
            || self.backstage_visible()
        {
            return false;
        }
        let Some(current) = self.query_limits.current else {
            return false;
        };
        self.query_limits.draft = Some(current);
        self.query_limits.error = None;
        self.query_limits.submitted = false;
        true
    }
}
