//! Shared database workspace presentation; hosts own files, tasks and services.
mod application_icon;
mod cells;
mod columns;
mod datasheet;
mod fonts;
mod formatting;
mod keyboard;
pub mod localization;
mod text_input;
mod theme;
mod workspace;

pub use application_icon::{ApplicationIconError, application_icon};
pub use cells::{Cell, CellKind, DisplayControl, HorizontalAlignment};
pub use columns::Column;
pub use fluent_bundle::FluentArgs;
pub use fonts::{FontFamilies, FontInventoryError};
pub use localization::{Locale, Localizer};
pub use theme::{Theme, configure};
pub use workspace::preview;
pub use workspace::{
    Action, CollectionKind, ContentsTarget, CreationOutcome, CreationRequest, Data, Database,
    DeleteReview, DeleteTarget, EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget,
    Field, FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
    FieldProperty, FieldPropertyValue, FilterComparison, FilterPublication, FilterRestore,
    FilterSelectionTarget, FilterTarget, FindDirection, FindMatch, FindPosition, FindRequest,
    FindScope, FindSpan, FindTarget, InputHint, InsertDraft, InsertField, InsertInput,
    InsertReview, InsertTarget, ObjectKind, ObjectOpen, ObjectTab, ObjectTarget, ParameterInput,
    ParameterSpec, QueryDesignCompletion, QueryDesignRequest, QueryDesignTarget, RecordTarget,
    RecoveryOutcome, ReplaceFindRequest, RowOrdinalError, RowOrdinals, SaveCleanupNotice,
    SavedQuery, SavedQueryTarget, Schema, SortSpec, SortTarget, State, Table,
    TableDraftSaveCompletion, TableSaveRequest, TotalAggregate, TotalSpec, TotalsData,
    TotalsDescription, TotalsField, TotalsTarget, WorkspaceLayout,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_workspace_renders_empty_and_populated_states_without_host_resources() {
        let ctx = egui::Context::default();
        configure(&ctx);
        let mut state = State::default();
        for populated in [false, true] {
            if populated {
                state.database = Some(Database {
                    title_path: None,
                    document_revision: 1,
                    title: "Sample.accdb".into(),
                    format: "Ace2016".into(),
                    queries: vec![],
                    tables: vec![Table {
                        local: true,
                        name: "Customers".into(),
                        system: false,
                        storage: "Local".into(),
                    }],
                });
                state.dismiss_backstage();
                state.data = Some(Data {
                    row_ordinals: None,
                    source_revision: None,
                    table: "Customers".into(),
                    offset: 0,
                    columns: vec!["Name".into()],
                    rows: vec![vec!["Alice".into()]],
                    has_more: false,
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    assert!(state.show(ui).is_empty());
                },
            );
            assert!(!output.shapes.is_empty());
            output.textures_delta.clear();
        }
    }
}
