//! Resource-free restart presentation, with no fabricated open database or edit.
use super::Preview;
use crate::{FluentArgs, RecoveryOutcome};

impl Preview {
    /// Representative localized report facts, not a native receipt or backend run.
    pub(super) fn restart_result_message(&self) -> String {
        let localizer = &self.state.localizer;
        let mut arguments = FluentArgs::new();
        arguments.set("outcome", localizer.text("editing-save-known-rolled-back"));
        let mut report = localizer.format("editing-save-current", &arguments);
        arguments.set(
            "completion",
            localizer.text("editing-save-completion-recovered"),
        );
        report.push(' ');
        report.push_str(&localizer.format("editing-save-prior", &arguments));
        report
    }

    pub(super) fn prepare_restart_scene(&mut self, finished: bool) {
        self.state.path = "Example.accdb".into();
        self.state.file_picker_available = true;
        self.state.show_open();
        let attempt = self.request();
        self.state
            .show_recovery_prompt(attempt, "Example.accdb".into());
        if finished {
            let operation = self.request();
            assert!(self.state.begin_recovery(attempt, operation));
            let message = self.restart_result_message();
            self.state
                .set_recovery_outcome(operation, RecoveryOutcome::Finished(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::Scene;

    #[test]
    fn restart_scenes_do_not_construct_database_edit_or_row_identities() {
        for (name, scene) in [
            ("restart-prompt", Scene::RestartPrompt),
            ("restart-result", Scene::RestartResult),
        ] {
            assert_eq!(name.parse::<Scene>().unwrap(), scene);
            let preview = Preview::new(scene);
            let state = preview.state();
            assert!(state.database.is_none());
            assert!(state.data.is_none());
            assert!(state.query_data.is_none());
            assert!(state.active_object().is_none());
            assert!(!state.insertion_dialog_open());
            assert!(state.recovery_prompt_open());
            assert!(!state.recovery_pending());
        }
    }
}
