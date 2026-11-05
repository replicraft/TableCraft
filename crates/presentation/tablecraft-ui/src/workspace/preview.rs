//! Deterministic synthetic responses for reviewing the existing workspace UI.
//!
//! No file, database, query evaluator or task service is available to this host.
mod actions;
mod collections;
mod editing;
mod errors;
mod fixtures;
mod queries;

use super::{DocumentView, State};
use crate::{CollectionKind, ContentsTarget, EditInput, EditSaveOutcome, EditTarget};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    Datasheet,
    LongNames,
    LongReadError,
    LongOpenError,
    LongQueryError,
    LongSavedError,
    Empty,
    Loading,
    Error,
    Design,
    Query,
    SavedQuery,
    Attachments,
    Multivalue,
    History,
    Edit,
    Review,
    Recovery,
    Backstage,
    Options,
    About,
    QueryRunning,
    QueryError,
    SavedInspecting,
    SavedRunning,
    SavedError,
    SavedParameters,
    CollectionLoading,
    CollectionError,
    CollectionUnassigned,
    CollectionEmpty,
    CollectionNull,
    AttachmentNull,
    HistoryNull,
    EditChecking,
    EditValidating,
    EditRejected,
    SaveApplying,
    SaveCancelling,
    SaveResolving,
    SaveSaved,
    SaveCancelled,
    SaveFailed,
    RecoveryDiagnosis,
}

impl Scene {
    pub const ALL: [Self; 44] = [
        Self::Datasheet,
        Self::LongNames,
        Self::LongReadError,
        Self::LongOpenError,
        Self::LongQueryError,
        Self::LongSavedError,
        Self::Empty,
        Self::Loading,
        Self::Error,
        Self::Design,
        Self::Query,
        Self::SavedQuery,
        Self::Attachments,
        Self::Multivalue,
        Self::History,
        Self::Edit,
        Self::Review,
        Self::Recovery,
        Self::Backstage,
        Self::Options,
        Self::About,
        Self::QueryRunning,
        Self::QueryError,
        Self::SavedInspecting,
        Self::SavedRunning,
        Self::SavedError,
        Self::SavedParameters,
        Self::CollectionLoading,
        Self::CollectionError,
        Self::CollectionUnassigned,
        Self::CollectionEmpty,
        Self::CollectionNull,
        Self::AttachmentNull,
        Self::HistoryNull,
        Self::EditChecking,
        Self::EditValidating,
        Self::EditRejected,
        Self::SaveApplying,
        Self::SaveCancelling,
        Self::SaveResolving,
        Self::SaveSaved,
        Self::SaveCancelled,
        Self::SaveFailed,
        Self::RecoveryDiagnosis,
    ];

    /// Animated mock states cannot settle while their existing spinner is visible.
    pub const fn is_animated(self) -> bool {
        matches!(
            self,
            Self::Loading
                | Self::QueryRunning
                | Self::SavedInspecting
                | Self::SavedRunning
                | Self::CollectionLoading
                | Self::EditChecking
                | Self::EditValidating
                | Self::SaveApplying
                | Self::SaveCancelling
                | Self::SaveResolving
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Datasheet => "datasheet",
            Self::LongNames => "long-names",
            Self::LongReadError => "long-read-error",
            Self::LongOpenError => "long-open-error",
            Self::LongQueryError => "long-query-error",
            Self::LongSavedError => "long-saved-error",
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Error => "error",
            Self::Design => "design",
            Self::Query => "query",
            Self::SavedQuery => "saved-query",
            Self::Attachments => "attachments",
            Self::Multivalue => "multivalue",
            Self::History => "history",
            Self::Edit => "edit",
            Self::Review => "review",
            Self::Recovery => "recovery",
            Self::Backstage => "backstage",
            Self::Options => "options",
            Self::About => "about",
            Self::QueryRunning => "query-running",
            Self::QueryError => "query-error",
            Self::SavedInspecting => "saved-inspecting",
            Self::SavedRunning => "saved-running",
            Self::SavedError => "saved-error",
            Self::SavedParameters => "saved-parameters",
            Self::CollectionLoading => "collection-loading",
            Self::CollectionError => "collection-error",
            Self::CollectionUnassigned => "collection-unassigned",
            Self::CollectionEmpty => "collection-empty",
            Self::CollectionNull => "collection-null",
            Self::AttachmentNull => "attachment-null",
            Self::HistoryNull => "history-null",
            Self::EditChecking => "edit-checking",
            Self::EditValidating => "edit-validating",
            Self::EditRejected => "edit-rejected",
            Self::SaveApplying => "save-applying",
            Self::SaveCancelling => "save-cancelling",
            Self::SaveResolving => "save-resolving",
            Self::SaveSaved => "save-saved",
            Self::SaveCancelled => "save-cancelled",
            Self::SaveFailed => "save-failed",
            Self::RecoveryDiagnosis => "recovery-diagnosis",
        }
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("unknown mock view; use {}", Scene::ALL.map(Scene::as_str).join(", "))]
pub struct UnknownScene;

impl FromStr for Scene {
    type Err = UnknownScene;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|scene| scene.as_str() == value)
            .ok_or(UnknownScene)
    }
}

/// Reuses the production presentation while replacing every host action locally.
pub struct Preview {
    state: State,
    next_request: u64,
    scene: Scene,
}

impl Preview {
    pub fn new(scene: Scene) -> Self {
        let mut preview = Self {
            state: fixtures::workspace(),
            next_request: 1,
            scene: Scene::Datasheet,
        };
        preview.set_scene(scene);
        preview
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    pub const fn scene(&self) -> Scene {
        self.scene
    }

    pub fn set_scene(&mut self, scene: Scene) {
        let mut state = fixtures::workspace();
        std::mem::swap(&mut state.localizer, &mut self.state.localizer);
        std::mem::swap(&mut state.theme, &mut self.state.theme);
        self.state = state;
        self.next_request = 1;
        self.scene = scene;

        match scene {
            Scene::Datasheet => {}
            Scene::LongNames => fixtures::long_names(&mut self.state),
            Scene::LongReadError
            | Scene::LongOpenError
            | Scene::LongQueryError
            | Scene::LongSavedError => self.prepare_long_error(scene),
            Scene::Empty => {
                let mut data = fixtures::page("Customers", 0);
                data.rows.clear();
                data.has_more = false;
                self.state.set_table_data(data);
            }
            Scene::Loading => self.state.busy = true,
            Scene::Error => {
                self.state.error = Some(self.state.localizer.text("settings-mock-read-error"));
            }
            Scene::Design => {
                self.state.prepare_schema_view();
                self.state.set_schema(fixtures::schema("Customers"));
            }
            Scene::Query => {
                self.state.view = DocumentView::Query;
                self.state.set_query_data(fixtures::query());
            }
            Scene::SavedQuery => {
                let target = fixtures::saved_target();
                self.inspect_saved(target);
            }
            Scene::Attachments => self.open_collection("Documents", CollectionKind::Attachments),
            Scene::Multivalue => self.open_collection("Categories", CollectionKind::Multivalue),
            Scene::History => self.open_collection("Notes", CollectionKind::VersionHistory),
            Scene::Edit => self.open_edit(),
            Scene::Review | Scene::Recovery => {
                self.open_edit();
                let input = EditInput {
                    text: "1495.50".into(),
                    is_null: false,
                };
                let review = self.preview_review(input);
                if scene == Scene::Recovery
                    && let Some(review) = review
                {
                    let save = self.request();
                    self.state.begin_edit_save(review, save);
                    self.state.set_edit_save_outcome(
                        save,
                        EditSaveOutcome::Retained {
                            message: self.state.localizer.text("settings-mock-retained"),
                            can_recover: true,
                            can_release: true,
                        },
                    );
                }
            }
            Scene::QueryRunning => self.prepare_query_running(),
            Scene::QueryError => self.prepare_query_error(),
            Scene::SavedInspecting => self.prepare_saved_inspecting(),
            Scene::SavedRunning => self.prepare_saved_running(),
            Scene::SavedError => self.prepare_saved_error(),
            Scene::SavedParameters => self.prepare_saved_parameters(),
            Scene::CollectionLoading => {
                self.prepare_collection_scene(collections::CollectionScene::Loading)
            }
            Scene::CollectionError => {
                self.prepare_collection_scene(collections::CollectionScene::Error)
            }
            Scene::CollectionUnassigned => {
                self.prepare_collection_scene(collections::CollectionScene::Unassigned)
            }
            Scene::CollectionEmpty => {
                self.prepare_collection_scene(collections::CollectionScene::Empty)
            }
            Scene::CollectionNull => {
                self.prepare_collection_scene(collections::CollectionScene::Null)
            }
            Scene::AttachmentNull => {
                self.prepare_collection_scene(collections::CollectionScene::AttachmentNull)
            }
            Scene::HistoryNull => {
                self.prepare_collection_scene(collections::CollectionScene::HistoryNull)
            }
            Scene::EditChecking => self.show_editing_scene(scene),
            Scene::EditValidating => self.show_editing_scene(scene),
            Scene::EditRejected => self.show_editing_scene(scene),
            Scene::SaveApplying => self.show_editing_scene(scene),
            Scene::SaveCancelling => self.show_editing_scene(scene),
            Scene::SaveResolving => self.show_editing_scene(scene),
            Scene::SaveSaved => self.show_editing_scene(scene),
            Scene::SaveCancelled => self.show_editing_scene(scene),
            Scene::SaveFailed => self.show_editing_scene(scene),
            Scene::RecoveryDiagnosis => self.show_editing_scene(scene),
            Scene::Backstage => self.state.show_open(),
            Scene::Options => self.state.backstage = super::backstage::Backstage::Options,
            Scene::About => self.state.backstage = super::backstage::Backstage::About,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        for action in self.state.show(ui) {
            self.dispatch(action);
        }
    }

    fn request(&mut self) -> u64 {
        let request = self.next_request;
        self.next_request += 1;
        request
    }

    fn open_collection(&mut self, column: &str, kind: CollectionKind) {
        let target = ContentsTarget {
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            column: column.into(),
            parent_ordinal: 0,
        };
        self.collection(target, kind);
    }

    fn open_edit(&mut self) {
        self.edit(EditTarget {
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            column: "Balance".into(),
            parent_ordinal: 0,
        });
    }
}

#[cfg(test)]
mod tests;
