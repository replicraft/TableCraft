//! Host adaptation of explicit UI limits to checked shared-runtime read options.
use super::Desktop;
use tablecraft_runtime::{QueryPoolLimits, QueryRunOptions, QueryRunOptionsError};
use tablecraft_ui::{QueryPoolLimits as UiPoolLimits, QueryRunLimits};

impl Desktop {
    pub(super) fn read_query_options(&self) -> QueryRunOptions {
        self.query_run_options
    }

    pub(super) fn configure_query_run_limits(&mut self, limits: QueryRunLimits) {
        if self.state.busy
            || self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || self.state.parameter_prompt_open()
            || self.state.edit_dialog_open()
            || self.state.insertion_dialog_open()
            || self.state.deletion_dialog_open()
            || self.state.zoom_dialog_open()
        {
            self.state.reject_query_run_limits(
                "Finish the active operation or dialog before changing query limits".into(),
            );
            return;
        }
        match runtime_options(limits) {
            Ok(options) => {
                self.query_run_options = options;
                self.state.set_query_run_limits(ui_limits(options));
            }
            Err(error) => self.state.reject_query_run_limits(error.to_string()),
        }
    }
}

fn runtime_options(limits: QueryRunLimits) -> Result<QueryRunOptions, QueryRunOptionsError> {
    QueryRunOptions::with_limits(
        limits.snapshot_rows,
        limits.batch_rows,
        runtime_pool(limits.native)?,
        runtime_pool(limits.query)?,
        runtime_pool(limits.comparison)?,
        runtime_pool(limits.presentation)?,
    )
}

fn runtime_pool(limits: UiPoolLimits) -> Result<QueryPoolLimits, QueryRunOptionsError> {
    QueryPoolLimits::new(limits.bytes, limits.rows, limits.steps)
}

pub(super) fn ui_limits(options: QueryRunOptions) -> QueryRunLimits {
    QueryRunLimits {
        snapshot_rows: options.snapshot().maximum_rows(),
        batch_rows: options.snapshot().batch_rows(),
        native: ui_pool(options.native()),
        query: ui_pool(options.query()),
        comparison: ui_pool(options.comparison()),
        presentation: ui_pool(options.presentation()),
    }
}

fn ui_pool(limits: QueryPoolLimits) -> UiPoolLimits {
    UiPoolLimits {
        bytes: limits.bytes(),
        rows: limits.rows(),
        steps: limits.steps(),
    }
}

#[cfg(test)]
mod tests;
