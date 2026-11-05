//! Deterministic synthetic responses for reviewing the existing workspace UI.
//!
//! No file, database, query evaluator or task service is available to this host.
mod actions;
mod fixtures;

use super::{DocumentView, State};
use crate::{CollectionKind, ContentsTarget, EditInput, EditSaveOutcome, EditTarget};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    Datasheet,
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
}

impl Scene {
    pub const ALL: [Self; 12] = [
        Self::Datasheet,
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
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Datasheet => "datasheet",
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
        }
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error(
    "unknown mock view; use datasheet, design, query, saved-query, attachments, multivalue, history, edit, review, recovery, backstage or options"
)]
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
            Scene::Backstage => self.state.show_open(),
            Scene::Options => self.state.backstage = super::backstage::Backstage::Options,
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
