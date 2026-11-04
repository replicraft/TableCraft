//! Shared database workspace presentation; hosts own files, tasks and services.
mod cells;
mod datasheet;
mod keyboard;
mod theme;
mod workspace;

pub use cells::{Cell, CellKind};
pub use theme::configure;
pub use workspace::{
    Action, CollectionKind, ContentsTarget, Data, Database, Field, Schema, State, Table,
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
                    document_revision: 1,
                    title: "Sample.accdb".into(),
                    format: "Ace2016".into(),
                    tables: vec![Table {
                        name: "Customers".into(),
                        system: false,
                        storage: "Local".into(),
                    }],
                });
                state.dismiss_backstage();
                state.data = Some(Data {
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
