//! Synthetic row-insertion responses; no source is opened or persisted.
use super::{Preview, Scene};
use crate::workspace::insertion::Dialog;
use crate::{
    Action, EditSaveOutcome, InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget,
};

const MOCK_FIELD_COUNT: usize = 3;
const MOCK_AUTO_NUMBER_POSITION: usize = 0;

impl Preview {
    pub(super) fn prepare_insert_scene(&mut self, reviewed: bool) {
        let target = self
            .state
            .current_insert_target()
            .expect("synthetic native table");
        self.mock_begin_insert(target);
        if reviewed {
            let (draft_id, proposed) = match &self.state.insertion {
                Some(Dialog::Draft { draft, .. }) => (
                    draft.id,
                    draft.fields.iter().map(|_| InsertInput::Omitted).collect(),
                ),
                _ => unreachable!("synthetic insertion draft was published"),
            };
            self.mock_review_insert(draft_id, proposed);
        }
    }

    pub(super) fn prepare_retained_insert_scene(&mut self, scene: Scene) {
        self.prepare_insert_scene(false);
        let (draft_id, proposed) = self
            .state
            .prepare_retained_insert_preview()
            .expect("bounded synthetic insertion input");
        self.mock_review_insert(draft_id, proposed);
        let review_id = match &self.state.insertion {
            Some(Dialog::Draft {
                review: Some(review),
                ..
            }) => review.id,
            _ => unreachable!("synthetic insertion was reviewed"),
        };
        let save_id = self.request();
        self.state.begin_insert_save(review_id, save_id);
        self.state.set_insert_save_outcome(
            save_id,
            EditSaveOutcome::Failed(self.state.localizer.text("insertion-mock-help")),
        );
        let permission = if scene == Scene::RetainedInsertRetry {
            crate::InsertRetryPermission::OriginalImage
        } else {
            crate::InsertRetryPermission::Blocked
        };
        assert!(self.state.acknowledge_insert_save(save_id, permission));
    }

    pub(super) fn mock_begin_insert(&mut self, target: InsertTarget) {
        if !self.state.can_insert_row(&target) {
            return;
        }
        let generated = self.scene() == Scene::GeneratedNewRecord;
        let fields = self
            .state
            .data
            .as_ref()
            .expect("admitted table")
            .columns
            .iter()
            .take(MOCK_FIELD_COUNT)
            .enumerate()
            .map(|(position, column)| InsertField {
                name: column.name.clone(),
                input_hint: if generated && position == MOCK_AUTO_NUMBER_POSITION {
                    crate::InputHint::Int32
                } else {
                    crate::InputHint::Text
                },
                nullable: !(generated && position == MOCK_AUTO_NUMBER_POSITION),
                auto_number: generated && position == MOCK_AUTO_NUMBER_POSITION,
            })
            .collect();
        let request = self.request();
        self.state.begin_insert(target.clone(), request);
        self.state.set_insert_draft(
            request,
            InsertDraft {
                id: request,
                target,
                fields,
            },
        );
    }

    pub(super) fn mock_review_insert(&mut self, draft_id: u64, proposed: Vec<InsertInput>) {
        if !self.state.can_review_insert(draft_id, &proposed) {
            return;
        }
        let target = match &self.state.insertion {
            Some(Dialog::Draft { draft, .. }) => draft.target.clone(),
            _ => return,
        };
        let request = self.request();
        self.state
            .begin_insert_review(draft_id, proposed.clone(), request);
        self.state.set_insert_review(
            request,
            InsertReview {
                id: request,
                draft_id,
                target,
                inputs: proposed,
                inserted_rows: 1,
            },
        );
    }

    pub(super) fn mock_apply_insert(&mut self, review_id: u64) {
        if !self.state.can_apply_insert(review_id) {
            return;
        }
        let save_id = self.request();
        self.state.begin_insert_save(review_id, save_id);
        self.state.set_insert_save_outcome(
            save_id,
            EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
        );
    }

    pub(super) fn mock_close_insert(&mut self, request_id: u64, draft_id: Option<u64>) {
        let matches = match &self.state.insertion {
            Some(Dialog::Checking {
                request_id: current,
                ..
            }) => *current == request_id && draft_id.is_none(),
            Some(Dialog::Draft {
                request_id: current,
                draft,
                ..
            }) => *current == request_id && draft_id == Some(draft.id),
            _ => false,
        };
        if matches {
            self.state.close_insert();
        }
    }

    pub(super) fn insertion_save_action(&mut self, action: &Action) -> bool {
        let Some(dialog) = &mut self.state.insertion else {
            return false;
        };
        match (action, dialog) {
            (
                Action::CancelEditSave { save_id },
                Dialog::Saving {
                    save_id: current,
                    cancel_requested,
                    ..
                },
            ) => {
                if save_id == current {
                    *cancel_requested = true;
                }
                true
            }
            (
                Action::Cancel,
                Dialog::Saving {
                    cancel_requested, ..
                },
            ) => {
                *cancel_requested = true;
                true
            }
            (
                Action::AcknowledgeEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome,
                    ..
                },
            ) => {
                if save_id == current && !matches!(outcome, EditSaveOutcome::Retained { .. }) {
                    self.state
                        .acknowledge_insert_save(*save_id, crate::InsertRetryPermission::Blocked);
                }
                true
            }
            (
                Action::RecoverEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome: EditSaveOutcome::Retained { can_recover, .. },
                    ..
                },
            ) if save_id == current && *can_recover => {
                let original = *save_id;
                let operation = self.request();
                self.state.begin_insert_resolution(original, operation);
                self.state.set_insert_save_outcome(
                    operation,
                    EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-resolved")),
                );
                true
            }
            (
                Action::ReleaseEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome: EditSaveOutcome::Retained { can_release, .. },
                    ..
                },
            ) if save_id == current && *can_release => {
                let original = *save_id;
                let operation = self.request();
                self.state.begin_insert_resolution(original, operation);
                self.state.set_insert_save_outcome(
                    operation,
                    EditSaveOutcome::Failed(self.state.localizer.text("settings-mock-resolved")),
                );
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Locale, Localizer, Theme};

    #[test]
    fn generated_insert_scene_renders_in_all_locales_and_themes_without_a_file_host() {
        let context = egui::Context::default();
        context.enable_accesskit();
        crate::configure(&context);
        for locale in Locale::ALL {
            for theme in [Theme::Light, Theme::Dark] {
                let mut preview = Preview::new(Scene::GeneratedNewRecord);
                preview.state_mut().localizer = Localizer::new(locale).unwrap();
                preview.state_mut().theme = theme;
                assert!(
                    matches!(&preview.state().insertion, Some(Dialog::Draft { draft, .. })
                    if draft.fields[0].auto_number && draft.fields[1..].iter().all(|field| !field.auto_number))
                );
                for _ in 0..2 {
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(900.0, 580.0),
                            )),
                            ..Default::default()
                        },
                        |ui| preview.show(ui),
                    );
                    assert!(!output.shapes.is_empty());
                    output.textures_delta.clear();
                }
                assert_eq!(
                    preview.state().localizer.diagnostics(),
                    crate::localization::LocalizationDiagnostics::default()
                );
                assert!(preview.state().data.as_ref().unwrap().rows.len() > 1);
            }
        }
        let ordinary = Preview::new(Scene::NewRecord);
        assert!(
            matches!(&ordinary.state().insertion, Some(Dialog::Draft { draft, .. })
            if draft.fields.iter().all(|field| !field.auto_number))
        );
    }
}

#[cfg(test)]
mod retained_tests {
    use super::*;
    use egui::accesskit::Role;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    #[test]
    fn retained_scenes_keep_raw_input_without_creating_rows_or_retry_authority() {
        for scene in [Scene::RetainedInsertInput, Scene::RetainedInsertRetry] {
            let preview = Preview::new(scene);
            let rows = preview.state().data.as_ref().unwrap().rows.len();
            let field = preview.state().data.as_ref().unwrap().columns[1]
                .name
                .clone();
            assert!(preview.state().insert_continuation_pending());
            assert!(!preview.state().insertion_dialog_open());
            assert!(preview.state().current_insert_target().is_none());
            let mut harness = Harness::builder()
                .with_size(egui::vec2(900.0, 580.0))
                .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
            crate::configure(&harness.ctx);
            harness.run_steps(2);
            let retry = harness
                .state()
                .state()
                .localizer
                .text("insertion-retry-original");
            let button = harness.get_by_role_and_label(Role::Button, &retry);
            assert_eq!(
                button.accesskit_node().is_disabled(),
                scene == Scene::RetainedInsertInput
            );
            assert!(
                harness
                    .get_by_role_and_label(Role::MultilineTextInput, &field)
                    .accesskit_node()
                    .data()
                    .is_read_only()
            );
            if scene == Scene::RetainedInsertRetry {
                button.click_accesskit();
                harness.run_steps(3);
                assert!(harness.state().state().insert_continuation_pending());
                assert!(!harness.state().state().insertion_dialog_open());
                assert!(harness.state().state().current_insert_target().is_none());
            }
            assert_eq!(
                harness.state().state().data.as_ref().unwrap().rows.len(),
                rows
            );
        }
    }
}
