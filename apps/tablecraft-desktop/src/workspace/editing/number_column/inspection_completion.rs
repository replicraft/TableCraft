//! Only recorded readonly source-inspection results bypass presentation generation.
use crate::workspace::Desktop;
use tablecraft_runtime::{Error, Response};

impl Desktop {
    pub(in crate::workspace) fn retire_number_inspection_publications(&mut self) {
        let worker = &self.worker;
        self.state
            .retire_number_column_inspection_publications(|id| worker.publication_is_current(id));
        self.state.prune_number_column_inspections();
    }

    pub(in crate::workspace) fn finish_superseded_number_inspection(
        &mut self,
        generation: u64,
        result: Result<Response, Error>,
    ) {
        if !self.state.has_pending_number_column_inspection(generation) {
            return;
        }
        if !self.worker.publication_is_current(generation) {
            self.state.interrupt_number_column_inspection(generation);
            self.state.prune_number_column_inspections();
            return;
        }
        match result {
            Ok(Response::NumberColumnAvailability { availability }) => {
                if !self.state.complete_number_column_inspection(
                    generation,
                    availability.document_revision,
                    &availability.table,
                    availability.available,
                ) {
                    self.state.fail_recorded_number_column_inspection(
                        generation,
                        self.state.localizer.text("objects-open-refused"),
                    );
                }
            }
            Err(error) => {
                self.state
                    .fail_recorded_number_column_inspection(generation, error.to_string());
            }
            Ok(_) => {
                // A terminal response of another shape is an explicit failure;
                // never reinterpret it as eligibility or leave this slot pending.
                self.state.fail_recorded_number_column_inspection(
                    generation,
                    self.state.localizer.text("objects-open-refused"),
                );
            }
        }
        self.state.prune_number_column_inspections();
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
