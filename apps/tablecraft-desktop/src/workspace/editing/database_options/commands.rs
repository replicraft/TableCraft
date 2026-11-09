//! Exact options actor commands, retaining known AFTER through failed reads.
use super::*;
use tablecraft_ui::DatabaseOptionsSaveCommand as Action;

impl Desktop {
    pub(in crate::workspace) fn cancel_current_database_options_operation(&mut self) -> bool {
        let Some(host) = &self.editing.database_options else {
            return false;
        };
        if let Some(identity) = host.identity {
            self.state.cancel_database_options_operation(identity);
            self.database_options_save_command(identity, Action::Cancel);
        } else {
            self.cancel_database_options_review(host.request_id);
        }
        true
    }

    pub(in crate::workspace) fn database_options_save_command(
        &mut self,
        identity: Identity,
        command: Action,
    ) {
        let Some(host) = &self.editing.database_options else {
            return;
        };
        if host.identity != Some(identity) || !self.state.matches_database_options_save(identity) {
            return;
        }
        match command {
            Action::Cancel => {
                let reopening = host.reopening;
                let request_id = host.request_id;
                self.editing.database_options.as_mut().unwrap().cancelled = true;
                if reopening {
                    self.worker.cancel();
                    self.database_options_reopen_failed(
                        request_id,
                        self.state
                            .localizer
                            .text("database-options-read-interrupted"),
                    );
                    self.state.busy = false;
                    self.pending = PendingOperation::Read;
                } else {
                    self.editing.worker.cancel(identity.save_id);
                }
            }
            Action::Recover | Action::Release => {
                let recover = command == Action::Recover;
                let Some(prepared) = self
                    .state
                    .prepare_database_options_resolution(identity, recover)
                else {
                    return;
                };
                let result = if recover {
                    self.editing.worker.recover(identity.save_id)
                } else {
                    self.editing.worker.release(identity.save_id)
                };
                match result {
                    Ok(save_id) => {
                        let next = prepared.bind(save_id);
                        let host = self.editing.database_options.as_mut().unwrap();
                        host.identity = Some(next);
                        host.terminal = None;
                        if let Some(save) = &mut self.editing.save {
                            save.id = save_id;
                            save.report = None;
                        }
                    }
                    Err(error) => {
                        drop(prepared);
                        self.state.error = Some(error.to_string());
                    }
                }
            }
            Action::Acknowledge => self.acknowledge_database_options(identity),
            Action::RetryReopen => self.reopen_database_options(identity),
            Action::ReturnToSettings => {
                if host.reopening
                    || self.editing.save.is_some()
                    || !host.terminal.is_some_and(|outcome| {
                        outcome
                            .database_options_reopen_facts()
                            .is_ok_and(|facts| facts.replacement)
                    })
                {
                    return;
                }
                let cleanup = host.cleanup;
                if self
                    .state
                    .return_from_database_options_reopen(identity, cleanup)
                {
                    self.editing.database_options = None;
                    self.state.database_options_restore_settings();
                    self.state.busy = false;
                    self.pending = PendingOperation::Read;
                }
            }
        }
    }
}
