//! Deterministic synthetic responses for reviewing the existing workspace UI.
//!
//! No file, database, query evaluator or task service is available to this host.
mod actions;
mod collections;
mod deletion;
mod editing;
mod errors;
mod field_filter;
mod filtering;
mod find;
mod fixtures;
mod insertion;
mod object_status;
mod queries;
mod query_design;
mod schema;
mod totals;

mod zoom;

use super::objects::{ObjectKind, ObjectTarget};
use super::{State, ribbon::Tab};
use crate::{CollectionKind, ContentsTarget, EditInput, EditSaveOutcome, EditTarget};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    Datasheet,
    RibbonCollapsed,
    RibbonTemporary,
    RibbonCreate,
    RibbonExternalData,
    RibbonDatabaseTools,
    RibbonHelp,
    RibbonTableFields,
    RibbonTable,
    Objects,
    ObjectLoading,
    ObjectError,
    FieldFilter,
    FieldFilterError,
    Totals,
    TotalsError,
    QueryDesign,
    QueryDesignTables,
    QueryDesignError,
    QueryDesignWide,
    QueryDesignEmpty,
    LongNames,
    LongReadError,
    LongOpenError,
    LongQueryError,
    LongSavedError,
    Empty,
    Loading,
    Error,
    Design,
    DesignLong,
    DesignUninspected,
    Query,
    SavedQuery,
    Attachments,
    Multivalue,
    History,
    Edit,
    NewRecord,
    InsertReview,
    DeleteChecking,
    DeleteReview,
    Find,
    FindSearching,
    Replace,
    ReplaceError,
    Filtered,
    FilterEmpty,
    FilterDisabled,
    FilterError,
    Sorted,
    Zoom,
    Review,
    Recovery,
    Startup,
    StartupOpen,
    Templates,
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
    pub const ALL: [Self; 83] = [
        Self::Datasheet,
        Self::RibbonCollapsed,
        Self::RibbonTemporary,
        Self::RibbonCreate,
        Self::RibbonExternalData,
        Self::RibbonDatabaseTools,
        Self::RibbonHelp,
        Self::RibbonTableFields,
        Self::RibbonTable,
        Self::Objects,
        Self::ObjectLoading,
        Self::ObjectError,
        Self::FieldFilter,
        Self::FieldFilterError,
        Self::Totals,
        Self::TotalsError,
        Self::QueryDesign,
        Self::QueryDesignTables,
        Self::QueryDesignError,
        Self::QueryDesignWide,
        Self::QueryDesignEmpty,
        Self::LongNames,
        Self::LongReadError,
        Self::LongOpenError,
        Self::LongQueryError,
        Self::LongSavedError,
        Self::Empty,
        Self::Loading,
        Self::Error,
        Self::Design,
        Self::DesignLong,
        Self::DesignUninspected,
        Self::Query,
        Self::SavedQuery,
        Self::Attachments,
        Self::Multivalue,
        Self::History,
        Self::Edit,
        Self::NewRecord,
        Self::InsertReview,
        Self::DeleteChecking,
        Self::DeleteReview,
        Self::Find,
        Self::FindSearching,
        Self::Replace,
        Self::ReplaceError,
        Self::Filtered,
        Self::FilterEmpty,
        Self::FilterDisabled,
        Self::FilterError,
        Self::Sorted,
        Self::Zoom,
        Self::Review,
        Self::Recovery,
        Self::Startup,
        Self::StartupOpen,
        Self::Templates,
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

    /// Pending spinners and focused input carets advance through bounded frames.
    pub const fn is_animated(self) -> bool {
        matches!(
            self,
            Self::Loading
                | Self::ObjectLoading
                | Self::QueryRunning
                | Self::SavedInspecting
                | Self::SavedRunning
                | Self::CollectionLoading
                | Self::Find
                | Self::FindSearching
                | Self::Replace
                | Self::ReplaceError
                | Self::DeleteChecking
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
            Self::RibbonCollapsed => "ribbon-collapsed",
            Self::RibbonTemporary => "ribbon-temporary",
            Self::RibbonCreate => "ribbon-create",
            Self::RibbonExternalData => "ribbon-external-data",
            Self::RibbonDatabaseTools => "ribbon-database-tools",
            Self::RibbonHelp => "ribbon-help",
            Self::RibbonTableFields => "ribbon-table-fields",
            Self::RibbonTable => "ribbon-table",
            Self::Objects => "objects",
            Self::ObjectLoading => "object-loading",
            Self::ObjectError => "object-error",
            Self::FieldFilter => "field-filter",
            Self::FieldFilterError => "field-filter-error",
            Self::Totals => "totals",
            Self::TotalsError => "totals-error",
            Self::QueryDesign => "query-design",
            Self::QueryDesignTables => "query-design-tables",
            Self::QueryDesignError => "query-design-error",
            Self::QueryDesignWide => "query-design-wide",
            Self::QueryDesignEmpty => "query-design-empty",
            Self::LongNames => "long-names",
            Self::LongReadError => "long-read-error",
            Self::LongOpenError => "long-open-error",
            Self::LongQueryError => "long-query-error",
            Self::LongSavedError => "long-saved-error",
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Error => "error",
            Self::Design => "design",
            Self::DesignLong => "design-long",
            Self::DesignUninspected => "design-uninspected",
            Self::Query => "query",
            Self::SavedQuery => "saved-query",
            Self::Attachments => "attachments",
            Self::Multivalue => "multivalue",
            Self::History => "history",
            Self::Edit => "edit",
            Self::NewRecord => "new-record",
            Self::InsertReview => "insert-review",
            Self::DeleteChecking => "delete-checking",
            Self::DeleteReview => "delete-review",
            Self::Find => "find",
            Self::FindSearching => "find-searching",
            Self::Replace => "replace",
            Self::ReplaceError => "replace-error",
            Self::Filtered => "filtered",
            Self::FilterEmpty => "filter-empty",
            Self::FilterDisabled => "filter-disabled",
            Self::FilterError => "filter-error",
            Self::Sorted => "sorted",
            Self::Zoom => "zoom",
            Self::Review => "review",
            Self::Recovery => "recovery",
            Self::Startup => "startup",
            Self::StartupOpen => "startup-open",
            Self::Templates => "templates",
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

    fn prepare_objects_scene(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Table {
                name: "Orders".into(),
            },
        });
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::SavedQuery {
                object_id: 42,
                name: "Customers by city".into(),
            },
        });
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Table {
                name: "Customers".into(),
            },
        });
    }

    pub fn set_scene(&mut self, scene: Scene) {
        let mut state = fixtures::workspace();
        std::mem::swap(&mut state.localizer, &mut self.state.localizer);
        std::mem::swap(&mut state.theme, &mut self.state.theme);
        std::mem::swap(&mut state.formatting, &mut self.state.formatting);
        std::mem::swap(&mut state.font_families, &mut self.state.font_families);
        self.state = state;
        self.next_request = 1;
        self.scene = scene;

        match scene {
            Scene::Datasheet => {}
            Scene::Objects => self.prepare_objects_scene(),
            Scene::ObjectLoading => self.prepare_unloaded_object(true),
            Scene::ObjectError => self.prepare_unloaded_object(false),
            Scene::FieldFilter => self.prepare_field_filter_scene(false),
            Scene::FieldFilterError => self.prepare_field_filter_scene(true),
            Scene::Totals => self.prepare_totals_scene(false),
            Scene::TotalsError => self.prepare_totals_scene(true),
            Scene::QueryDesign => self.prepare_query_design(false, false),
            Scene::QueryDesignTables => self.prepare_query_design(true, false),
            Scene::QueryDesignError => self.prepare_query_design(false, true),
            Scene::QueryDesignWide => self.prepare_wide_query_design(),
            Scene::QueryDesignEmpty => self.prepare_empty_query_design(),
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
            Scene::DesignLong => self.prepare_long_design(),
            Scene::DesignUninspected => self.prepare_uninspected_design(),
            Scene::Design => {
                self.state.prepare_schema_view();
                self.state.set_schema(fixtures::schema("Customers"));
            }
            Scene::Query => {
                self.mock_open_object(ObjectTarget {
                    document_revision: fixtures::REVISION,
                    kind: ObjectKind::Sql,
                });
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
            Scene::NewRecord => self.prepare_insert_scene(false),
            Scene::InsertReview => self.prepare_insert_scene(true),
            Scene::DeleteChecking => self.prepare_delete_scene(false),
            Scene::DeleteReview => self.prepare_delete_scene(true),
            Scene::Find => self.prepare_find_scene(false),
            Scene::FindSearching => self.prepare_find_scene(true),
            Scene::RibbonCollapsed => self.state.set_ribbon_collapsed(true),
            Scene::RibbonTemporary => {
                self.state.set_ribbon_collapsed(true);
                self.state.show_ribbon_temporarily();
            }
            Scene::RibbonCreate => self.state.ribbon_tab = Tab::Create,
            Scene::RibbonExternalData => self.state.ribbon_tab = Tab::ExternalData,
            Scene::RibbonDatabaseTools => self.state.ribbon_tab = Tab::DatabaseTools,
            Scene::RibbonHelp => self.state.ribbon_tab = Tab::Help,
            Scene::RibbonTableFields => self.state.ribbon_tab = Tab::TableFields,
            Scene::RibbonTable => self.state.ribbon_tab = Tab::Table,
            Scene::Replace => self.state.prepare_mock_replace(false),
            Scene::ReplaceError => self.state.prepare_mock_replace(true),
            Scene::Filtered => self.prepare_filter_scene(filtering::FilterScene::Applied),
            Scene::FilterEmpty => self.prepare_filter_scene(filtering::FilterScene::Empty),
            Scene::FilterDisabled => self.prepare_filter_scene(filtering::FilterScene::Disabled),
            Scene::FilterError => self.prepare_filter_scene(filtering::FilterScene::Error),
            Scene::Sorted => self.prepare_sorted_scene(),
            Scene::Zoom => self.prepare_zoom_scene(),
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
            Scene::Startup | Scene::StartupOpen => {
                let mut startup = State {
                    mock_preview: true,
                    file_picker_available: true,
                    ..State::default()
                };
                std::mem::swap(&mut startup.localizer, &mut self.state.localizer);
                std::mem::swap(&mut startup.theme, &mut self.state.theme);
                std::mem::swap(&mut startup.formatting, &mut self.state.formatting);
                std::mem::swap(&mut startup.font_families, &mut self.state.font_families);
                self.state = startup;
                if scene == Scene::StartupOpen {
                    self.state.show_open();
                }
            }
            Scene::Templates => self.state.backstage = super::backstage::Backstage::New,
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
