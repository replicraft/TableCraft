//! Synthetic action decisions use the real UI owner without a database capability.
use super::{Preview, Scene, fixtures};
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{Action, EditSaveOutcome, QueryActionEffect, QueryActionKind, QueryActionReview};

const UPDATE_SQL: &str =
    "UPDATE [Customers] SET [Balance] = [Balance] + 25 WHERE [City] = 'Madrid'";

const DELETE_SQL: &str = "DELETE FROM [Customers] WHERE [City] = 'Madrid'";
const INSERT_SQL: &str = concat!(
    "INSERT INTO [Customers] ([CustomerName],[City],[Balance]) ",
    "SELECT [CustomerName],[City],[Balance] FROM [Prospects] WHERE [City] = 'Madrid'",
);

impl Preview {
    pub(super) fn prepare_query_action_scene(&mut self, scene: Scene) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        assert!(
            self.state
                .set_query_data(Some(fixtures::SQL.into()), fixtures::query())
        );
        let sql = match scene {
            Scene::QueryDeleteReview | Scene::QueryDeleteSaving | Scene::QueryDeleteResult => {
                DELETE_SQL
            }
            Scene::QueryInsertReview | Scene::QueryInsertSaving | Scene::QueryInsertResult => {
                INSERT_SQL
            }
            _ => UPDATE_SQL,
        };
        assert!(self.state.replace_query_sql(sql.into()));
        let review = self
            .mock_review_query_action()
            .expect("synthetic action review");
        if matches!(
            scene,
            Scene::QueryUpdateReview | Scene::QueryDeleteReview | Scene::QueryInsertReview
        ) {
            return;
        }

        let save = self.request();
        assert!(self.state.begin_query_action_save(review, save));
        if matches!(
            scene,
            Scene::QueryUpdateResult | Scene::QueryDeleteResult | Scene::QueryInsertResult
        ) {
            assert!(self.state.set_query_action_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
            ));
        }
    }

    pub(super) fn mock_review_query_action(&mut self) -> Option<u64> {
        if !self.state.can_inspect_query_parameters() {
            return None;
        }
        let sql = self.state.capture_query_draft()?;
        let kind = match sql.as_str() {
            UPDATE_SQL => QueryActionKind::Update,
            DELETE_SQL => QueryActionKind::Delete,
            INSERT_SQL => QueryActionKind::Insert,
            _ => return None,
        };
        let inspection = self.request();
        // NO_CLONE: Parameter acquisition keeps exact SQL snapshot while subsequent details and
        // action-review admission compare original spelling; separate owned pending snapshot is
        // intentional.
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
            .begin_query_action_review(inspection, &sql, request, parameters, kind)
        {
            return None;
        }
        let review = QueryActionReview {
            id: request,
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            effect: match kind {
                QueryActionKind::Update => QueryActionEffect::Update {
                    matched_rows: 2,
                    changed_rows: 2,
                },
                QueryActionKind::Delete => QueryActionEffect::Delete {
                    matched_rows: 2,
                    deleted_rows: 2,
                },
                QueryActionKind::Insert => QueryActionEffect::Insert { inserted_rows: 2 },
            },
        };
        self.state
            .set_query_action_review(request, review)
            .then_some(request)
    }

    pub(super) fn mock_apply_query_action(&mut self, review_id: u64) {
        let save = self.request();
        if self.state.begin_query_action_save(review_id, save) {
            self.state.set_query_action_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
            );
        }
    }

    pub(super) fn mock_cancel_query_action_review(&mut self, request_id: u64) {
        if self
            .state
            .query_action_review_current(request_id, &self.state.sql)
        {
            self.state.close_query_action();
        }
    }

    pub(super) fn query_action_save_action(&mut self, action: &Action) -> bool {
        if !self.state.query_action_save_pending() {
            return false;
        }
        match action {
            Action::CancelEditSave { save_id } => {
                self.state.request_query_action_save_cancellation(*save_id);
            }
            Action::Cancel => {
                if let Some(save) = self.state.query_action_save_id() {
                    self.state.request_query_action_save_cancellation(save);
                }
            }
            Action::AcknowledgeEditSave { save_id } => {
                self.state.acknowledge_query_action_save(*save_id);
            }
            Action::RecoverEditSave { save_id } => {
                self.resolve_mock_query_action(*save_id, true);
            }
            Action::ReleaseEditSave { save_id } => {
                self.resolve_mock_query_action(*save_id, false);
            }
            _ => {}
        }
        true
    }

    fn resolve_mock_query_action(&mut self, save_id: u64, recover: bool) {
        if !self.state.can_resolve_query_action_save(save_id, recover) {
            return;
        }
        let operation = self.request();
        if !self.state.begin_query_action_resolution(save_id, operation) {
            return;
        }
        let message = self.state.localizer.text("settings-mock-resolved");
        let outcome = if recover {
            EditSaveOutcome::Saved(message)
        } else {
            EditSaveOutcome::Failed(message)
        };
        self.state.set_query_action_save_outcome(operation, outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_scenes_use_source_bound_review_and_nonreplaceable_save_owners() {
        for (review_scene, saving_scene, result_scene, sql) in [
            (
                Scene::QueryUpdateReview,
                Scene::QueryUpdateSaving,
                Scene::QueryUpdateResult,
                UPDATE_SQL,
            ),
            (
                Scene::QueryDeleteReview,
                Scene::QueryDeleteSaving,
                Scene::QueryDeleteResult,
                DELETE_SQL,
            ),
            (
                Scene::QueryInsertReview,
                Scene::QueryInsertSaving,
                Scene::QueryInsertResult,
                INSERT_SQL,
            ),
        ] {
            let mut review = Preview::new(review_scene);
            assert!(review.state.query_action_dialog_open());
            assert!(!review.state.query_action_save_pending());
            assert!(review.state.query_data.is_some());
            let request = review.next_request - 1;
            assert!(review.state.can_apply_query_action(request));
            review.dispatch(Action::CancelQueryActionReview {
                request_id: request,
            });
            assert!(!review.state.query_action_dialog_open());
            assert_eq!(review.state.sql, sql);
            assert!(review.state.query_data.is_some());

            let mut saving = Preview::new(saving_scene);
            let save = saving.state.query_action_save_id().unwrap();
            assert!(saving.state.query_action_save_pending());
            saving.dispatch(Action::CancelEditSave { save_id: save });
            assert_eq!(saving.state.query_action_save_id(), Some(save));
            assert!(!saving.state.close_query_action());

            let mut result = Preview::new(result_scene);
            let save = result.state.query_action_save_id().unwrap();
            assert!(result.state.can_acknowledge_query_action_save(save));
            result.dispatch(Action::AcknowledgeEditSave { save_id: save });
            assert!(!result.state.query_action_dialog_open());
            assert!(result.state.query_data.is_some());
        }
    }
}
