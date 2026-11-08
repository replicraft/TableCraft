//! Synthetic UPDATE decisions use the real UI owner without a database capability.
use super::{Preview, Scene, fixtures};
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{Action, EditSaveOutcome, QueryUpdateReview};

const UPDATE_SQL: &str =
    "UPDATE [Customers] SET [Balance] = [Balance] + 25 WHERE [City] = 'Madrid'";

impl Preview {
    pub(super) fn prepare_query_update_scene(&mut self, scene: Scene) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        assert!(
            self.state
                .set_query_data(Some(fixtures::SQL.into()), fixtures::query())
        );
        assert!(self.state.replace_query_sql(UPDATE_SQL.into()));
        let review = self
            .mock_review_query_update()
            .expect("synthetic UPDATE review");
        if scene == Scene::QueryUpdateReview {
            return;
        }

        let save = self.request();
        assert!(self.state.begin_query_update_save(review, save));
        if scene == Scene::QueryUpdateResult {
            assert!(self.state.set_query_update_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
            ));
        }
    }

    pub(super) fn mock_review_query_update(&mut self) -> Option<u64> {
        if !self.state.can_inspect_query_parameters() {
            return None;
        }
        let sql = self.state.capture_query_draft()?;
        if sql != UPDATE_SQL {
            return None;
        }
        let inspection = self.request();
        if !self
            .state
            .begin_query_parameter_inspection(inspection, sql.clone())
        {
            return None;
        }
        let Some(Action::RunParameterizedQuery { parameters, .. }) = self
            .state
            .set_query_parameter_details(inspection, &sql, Vec::new())
        else {
            return None;
        };
        let request = self.request();
        if !self
            .state
            .begin_query_update_review(inspection, &sql, request, parameters)
        {
            return None;
        }
        let review = QueryUpdateReview {
            id: request,
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            matched_rows: 2,
            changed_rows: 2,
        };
        self.state
            .set_query_update_review(request, review)
            .then_some(request)
    }

    pub(super) fn mock_apply_query_update(&mut self, review_id: u64) {
        let save = self.request();
        if self.state.begin_query_update_save(review_id, save) {
            self.state.set_query_update_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
            );
        }
    }

    pub(super) fn mock_cancel_query_update_review(&mut self, request_id: u64) {
        if self
            .state
            .query_update_review_current(request_id, &self.state.sql)
        {
            self.state.close_query_update();
        }
    }

    pub(super) fn query_update_save_action(&mut self, action: &Action) -> bool {
        if !self.state.query_update_save_pending() {
            return false;
        }
        match action {
            Action::CancelEditSave { save_id } => {
                self.state.request_query_update_save_cancellation(*save_id);
            }
            Action::Cancel => {
                if let Some(save) = self.state.query_update_save_id() {
                    self.state.request_query_update_save_cancellation(save);
                }
            }
            Action::AcknowledgeEditSave { save_id } => {
                self.state.acknowledge_query_update_save(*save_id);
            }
            Action::RecoverEditSave { save_id } => {
                self.resolve_mock_query_update(*save_id, true);
            }
            Action::ReleaseEditSave { save_id } => {
                self.resolve_mock_query_update(*save_id, false);
            }
            _ => {}
        }
        true
    }

    fn resolve_mock_query_update(&mut self, save_id: u64, recover: bool) {
        if !self.state.can_resolve_query_update_save(save_id, recover) {
            return;
        }
        let operation = self.request();
        if !self.state.begin_query_update_resolution(save_id, operation) {
            return;
        }
        let message = self.state.localizer.text("settings-mock-resolved");
        let outcome = if recover {
            EditSaveOutcome::Saved(message)
        } else {
            EditSaveOutcome::Failed(message)
        };
        self.state.set_query_update_save_outcome(operation, outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_scenes_use_source_bound_review_and_nonreplaceable_save_owners() {
        let mut review = Preview::new(Scene::QueryUpdateReview);
        assert!(review.state.query_update_dialog_open());
        assert!(!review.state.query_update_save_pending());
        assert!(review.state.query_data.is_some());
        let request = review.next_request - 1;
        assert!(review.state.can_apply_query_update(request));
        review.dispatch(Action::CancelQueryUpdateReview {
            request_id: request,
        });
        assert!(!review.state.query_update_dialog_open());
        assert_eq!(review.state.sql, UPDATE_SQL);
        assert!(review.state.query_data.is_some());

        let mut saving = Preview::new(Scene::QueryUpdateSaving);
        let save = saving.state.query_update_save_id().unwrap();
        assert!(saving.state.query_update_save_pending());
        saving.dispatch(Action::CancelEditSave { save_id: save });
        assert_eq!(saving.state.query_update_save_id(), Some(save));
        assert!(!saving.state.close_query_update());

        let mut result = Preview::new(Scene::QueryUpdateResult);
        let save = result.state.query_update_save_id().unwrap();
        assert!(result.state.can_acknowledge_query_update_save(save));
        result.dispatch(Action::AcknowledgeEditSave { save_id: save });
        assert!(!result.state.query_update_dialog_open());
        assert!(result.state.query_data.is_some());
    }
}
