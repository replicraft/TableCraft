//! Synthetic action decisions use the real UI owner without a database capability.
use super::{Preview, Scene, fixtures};
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{
    Action, EditSaveOutcome, QueryActionEffect, QueryActionKind, QueryActionReview,
    QuerySchemaEffect,
};

const UPDATE_SQL: &str =
    "UPDATE [Customers] SET [Balance] = [Balance] + 25 WHERE [City] = 'Madrid'";

const MAKE_TABLE_SQL: &str = "SELECT [CustomerName],[City],[Balance] INTO [CustomerArchive] FROM [Customers] ORDER BY [CustomerName]";

const INDEX_SQL: &str = "CREATE INDEX [ScoreIndex] ON [Archive] ([CopiedScore] DESC)";

const ADD_COLUMN_SQL: &str = "ALTER TABLE [Items] ADD COLUMN [Amount] CURRENCY";

const CREATE_SQL: &str = "CREATE TABLE [Contacts] ([Id] LONG, [Name] VARCHAR(80))";
const DROP_TABLE_SQL: &str = "DROP TABLE [Customers]";
const DROP_COLUMN_SQL: &str = "ALTER TABLE [Customers] DROP COLUMN [Balance]";

const DELETE_SQL: &str = "DELETE FROM [Customers] WHERE [City] = 'Madrid'";
const INSERT_SQL: &str = concat!(
    "INSERT INTO [Customers] ([CustomerName],[City],[Balance]) ",
    "SELECT [CustomerName],[City],[Balance] FROM [Prospects] WHERE [City] = 'Madrid'",
);

impl Preview {
    pub(super) fn prepare_query_action_scene(&mut self, scene: Scene) {
        if matches!(
            scene,
            Scene::QuerySchemaDropTableReview
                | Scene::QuerySchemaDropColumnReview
                | Scene::QuerySchemaReopenFailed
        ) {
            for tab in self.state.object_tabs() {
                if matches!(&tab.target.kind, ObjectKind::Table { name } if name == "Customers") {
                    assert!(self.state.close_object(&tab));
                }
            }
        }
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
            Scene::QueryMakeTableReview => MAKE_TABLE_SQL,
            Scene::QuerySchemaIndexReview => INDEX_SQL,
            Scene::QuerySchemaAddColumnReview => ADD_COLUMN_SQL,
            Scene::QuerySchemaCreateReview => CREATE_SQL,
            Scene::QuerySchemaDropTableReview => DROP_TABLE_SQL,
            Scene::QuerySchemaDropColumnReview | Scene::QuerySchemaReopenFailed => DROP_COLUMN_SQL,
            _ => UPDATE_SQL,
        };
        assert!(self.state.replace_query_sql(sql.into()));
        let review = self
            .mock_review_query_action()
            .expect("synthetic action review");
        if matches!(
            scene,
            Scene::QueryUpdateReview
                | Scene::QueryDeleteReview
                | Scene::QueryInsertReview
                | Scene::QuerySchemaIndexReview
                | Scene::QuerySchemaAddColumnReview
                | Scene::QueryMakeTableReview
                | Scene::QuerySchemaCreateReview
                | Scene::QuerySchemaDropTableReview
                | Scene::QuerySchemaDropColumnReview
        ) {
            return;
        }

        let save = self.request();
        assert!(self.state.begin_query_action_save(review, save));
        if scene == Scene::QuerySchemaReopenFailed {
            assert!(self.state.set_query_action_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved"))
            ));
            let _publication = self
                .state
                .take_query_schema_publication(save)
                .expect("synthetic saved schema origin");
            self.state.fail_query_schema_reopen(fixtures::REVISION);
            return;
        }
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
            INDEX_SQL | ADD_COLUMN_SQL | MAKE_TABLE_SQL | CREATE_SQL | DROP_TABLE_SQL
            | DROP_COLUMN_SQL => QueryActionKind::Schema,
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
            table: if sql == ADD_COLUMN_SQL {
                "Items"
            } else if sql == INDEX_SQL {
                "Archive"
            } else if sql == MAKE_TABLE_SQL {
                "CustomerArchive"
            } else if sql == CREATE_SQL {
                "Contacts"
            } else {
                "Customers"
            }
            .into(),
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
                QueryActionKind::Schema => QueryActionEffect::Schema(match sql.as_str() {
                    ADD_COLUMN_SQL => QuerySchemaEffect::AddColumn {
                        column: "Amount".into(),
                        scalar_type: tablecraft_schema::ScalarType::Currency,
                    },
                    INDEX_SQL => QuerySchemaEffect::CreateIndex {
                        index: "ScoreIndex".into(),
                        column: "CopiedScore".into(),
                        native_number: 0,
                        descending: true,
                        indexed_rows: fixtures::TOTAL_ROWS as usize,
                    },
                    MAKE_TABLE_SQL => QuerySchemaEffect::MakeTable {
                        column_count: 3,
                        inserted_rows: 900,
                    },
                    CREATE_SQL => QuerySchemaEffect::CreateTable { column_count: 2 },
                    DROP_TABLE_SQL => QuerySchemaEffect::DropTable {
                        declared_rows: fixtures::TOTAL_ROWS,
                    },
                    DROP_COLUMN_SQL => QuerySchemaEffect::DropColumn {
                        column: "CopiedScore".into(),
                        native_number: 0,
                    },
                    _ => return None,
                }),
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

    #[test]
    fn schema_scenes_keep_actual_sql_origins_and_no_native_retry_authority() {
        for scene in [
            Scene::QuerySchemaCreateReview,
            Scene::QuerySchemaDropTableReview,
            Scene::QuerySchemaDropColumnReview,
        ] {
            let preview = Preview::new(scene);
            assert!(preview.state.query_action_dialog_open());
            assert!(!preview.state.query_action_save_pending());
            assert!(preview.state.query_schema_reopen_retry_revision().is_none());
            assert!(!preview.state.native_source_stale());
        }
        let mut failed = Preview::new(Scene::QuerySchemaReopenFailed);
        assert!(!failed.state.query_action_dialog_open());
        assert!(failed.state.native_source_stale());
        assert!(!failed.state.can_inspect_query_parameters());
        assert_eq!(failed.state.sql, DROP_COLUMN_SQL);
        assert_eq!(
            failed.state.query_schema_reopen_retry_revision(),
            Some(fixtures::REVISION)
        );
        failed.dispatch(Action::RetryQuerySchemaReopen {
            document_revision: fixtures::REVISION,
        });
        assert_eq!(failed.state.sql, DROP_COLUMN_SQL);
        assert_eq!(
            failed.state.query_schema_reopen_retry_revision(),
            Some(fixtures::REVISION)
        );
    }
}

#[cfg(test)]
mod make_table_scene_tests {
    use super::*;
    use crate::{FluentArgs, Locale, Localizer, Theme};
    use egui::accesskit::Role;

    #[test]
    fn make_table_review_scene_shows_destination_counts_and_localized_decisions_offscreen() {
        for locale in [Locale::EnUs, Locale::De] {
            for theme in [Theme::Light, Theme::Dark] {
                let context = egui::Context::default();
                context.enable_accesskit();
                crate::configure(&context);
                let mut preview = Preview::new(Scene::QueryMakeTableReview);
                preview.state.localizer = Localizer::new(locale).unwrap();
                preview.state.theme = theme;
                assert_eq!(preview.state.sql, MAKE_TABLE_SQL);
                for frame in 0..2 {
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1280.0, 820.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            preview.show(ui);
                        },
                    );
                    output.textures_delta.clear();
                    if frame == 0 {
                        continue;
                    }
                    let painted = output
                        .shapes
                        .iter()
                        .filter_map(|shape| {
                            if let egui::epaint::Shape::Text(text) = &shape.shape {
                                Some(text.galley.job.text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    let mut args = FluentArgs::new();
                    args.set("count", 3);
                    let fields = preview
                        .state
                        .localizer
                        .format("query-action-schema-create-fields", &args);
                    args.set("count", 900);
                    let records = preview
                        .state
                        .localizer
                        .format("query-action-schema-make-table-records", &args);
                    assert!(painted.contains(&fields.as_str()));
                    assert!(painted.contains(&records.as_str()));
                    assert!(painted.iter().any(|text| text.contains("CustomerArchive")));
                    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
                    for key in ["query-action-no", "query-action-yes"] {
                        let label = preview.state.localizer.text(key);
                        assert!(
                            tree.nodes
                                .iter()
                                .any(|(_, node)| node.role() == Role::Button
                                    && node.label() == Some(label.as_str()))
                        );
                    }
                    let matched = preview
                        .state
                        .localizer
                        .format("query-action-matched", &args);
                    assert!(!painted.contains(&matched.as_str()));
                }
            }
        }
    }
}
