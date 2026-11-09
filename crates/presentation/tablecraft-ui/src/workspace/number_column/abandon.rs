//! Release failed-reopen presentation without erasing historical saved facts.
use super::{
    NumberColumnRequest, NumberColumnSaveIdentity, Phase,
    status::{Status, StatusPhase},
};
use crate::localization::tr;
use crate::workspace::{EditSaveOutcome, SaveCleanupNotice, State, diagnostics, save_display};

/// Historical shared presentation facts. They grant no native retry or rollback.
pub struct NumberColumnSavedWarning<'a> {
    pub identity: NumberColumnSaveIdentity,
    pub outcome: Option<&'a EditSaveOutcome>,
    pub reopen_error: Option<&'a str>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub report_omitted: bool,
}

pub(super) struct SavedWarning {
    document_revision: u64,
    status: Status,
    cleanup: Option<SaveCleanupNotice>,
    owner: super::Draft,
    open: bool,
}

impl SavedWarning {
    pub(super) fn heap_bytes(&self) -> Option<usize> {
        self.status
            .heap_bytes()?
            .checked_add(super::table_name(&self.owner.tab)?.capacity())?
            .checked_add(self.owner.column.capacity())
    }

    pub(super) fn presented(&self) -> Option<&super::Draft> {
        self.open.then_some(&self.owner)
    }

    pub(super) fn discard(&mut self, tab: &crate::ObjectTab, revision: u64) -> bool {
        if !self.open || self.owner.tab != *tab || self.owner.revision != revision {
            return false;
        }
        self.open = false;
        true
    }
}

impl State {
    pub fn can_return_to_number_column_draft(&self, identity: NumberColumnSaveIdentity) -> bool {
        self.number_column_status.as_ref().is_some_and(|status| {
            status.identity == identity
                && status.phase == StatusPhase::ReopenFailed
                && status
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| !matches!(outcome, EditSaveOutcome::Retained { .. }))
        })
    }

    /// Host precondition: actual native actor ACK/release, no retained actor,
    /// known replacement, and terminal failed readonly whole-image reopen.
    /// Move existing status strings rather than allocating at this exit boundary.
    pub fn return_number_column_to_draft(
        &mut self,
        request: &NumberColumnRequest,
        identity: NumberColumnSaveIdentity,
        cleanup: Option<SaveCleanupNotice>,
    ) -> bool {
        if !self.can_return_to_number_column_draft(identity)
            || !self.matches_number_column_save(request, identity)
        {
            return false;
        }
        let status = self.number_column_status.take().unwrap();
        let mut owner = self.number_column.draft.take().unwrap();
        owner.phase = Phase::Naming;
        self.number_column.warning = Some(SavedWarning {
            document_revision: request.tab.target.document_revision,
            status,
            cleanup,
            owner,
            open: true,
        });
        true
    }

    pub fn number_column_source_stale(&self) -> bool {
        self.number_column.warning.as_ref().is_some_and(|warning| {
            self.database
                .as_ref()
                .is_some_and(|database| database.document_revision == warning.document_revision)
        })
    }

    /// Historical replacement facts remain visible even after explicit discard
    /// of the provisional name. They cannot become a rollback or retry grant.
    pub fn number_column_saved_warning(&self) -> Option<NumberColumnSavedWarning<'_>> {
        let warning = self
            .number_column
            .warning
            .as_ref()
            .filter(|_| self.number_column_source_stale())?;
        Some(NumberColumnSavedWarning {
            identity: warning.status.identity,
            outcome: warning.status.outcome.as_ref(),
            reopen_error: warning.status.error.as_deref(),
            cleanup: warning.cleanup,
            report_omitted: warning.status.omitted,
        })
    }

    pub fn number_column_saved_warning_name(&self) -> Option<(&crate::ObjectTab, &str)> {
        let warning = self
            .number_column
            .warning
            .as_ref()
            .filter(|_| self.number_column_source_stale())?;
        Some((&warning.owner.tab, &warning.owner.column))
    }

    pub(in crate::workspace) fn number_column_saved_warning_status(&self, ui: &mut egui::Ui) {
        let Some(warning) = self
            .number_column
            .warning
            .as_ref()
            .filter(|_| self.number_column_source_stale())
        else {
            return;
        };
        ui.group(|ui| {
            ui.label(tr(ui, "datasheet-number-saved-warning"));
            if let Some(table) = super::table_name(&warning.owner.tab) {
                ui.label(table);
            }
            ui.label(&warning.owner.column);
            egui::ScrollArea::vertical()
                .id_salt("number_saved_warning")
                .max_height(120.0)
                .show(ui, |ui| {
                    if let Some(outcome) = &warning.status.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    if warning.status.omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                    if let Some(error) = &warning.status.error {
                        diagnostics::show(ui, ui.id().with("number_saved_read_error"), error);
                    }
                    if warning.cleanup == Some(SaveCleanupNotice::RetainedPrivateJournal) {
                        ui.label(tr(ui, "editing-save-retained-private-journal"));
                    }
                });
        });
    }

    pub(in crate::workspace) fn retire_unowned_number_warning(&mut self, revision: Option<u64>) {
        if self.number_column.presented_draft().is_none()
            && self
                .number_column
                .warning
                .as_ref()
                .is_some_and(|warning| Some(warning.document_revision) != revision)
        {
            self.number_column.warning = None;
        }
    }
}

#[cfg(test)]
mod tests;
