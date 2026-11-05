use super::*;
use crate::{Cell, CellKind, Database, RecordTarget, State, datasheet};

fn data() -> Data {
    Data {
        source_revision: Some(7),
        table: "Items".into(),
        offset: 0,
        columns: vec![
            Column {
                name: "RawValue".into(),
                caption: Some("Friendly".into()),
            },
            Column {
                name: "RawItems".into(),
                caption: Some("Friendly".into()),
            },
        ],
        rows: vec![vec![
            Cell::scalar("stored value"),
            Cell {
                alignment: crate::HorizontalAlignment::Left,
                text: "2 items".into(),
                kind: CellKind::Multivalue {
                    assigned: true,
                    members: 2,
                },
            },
        ]],
        has_more: false,
    }
}

#[test]
fn absent_and_empty_captions_fall_back_without_rewriting_retained_metadata() {
    let mut column: Column = "RawName".into();
    assert_eq!(column.label(), "RawName");
    column.caption = Some(String::new());
    assert_eq!(column.label(), "RawName");
    assert_eq!(column.caption.as_deref(), Some(""));
    column.caption = Some(" ".into());
    assert_eq!(column.label(), " ");
    column.caption = Some("Betrag €".into());
    assert_eq!(column.label(), "Betrag €");
    assert_eq!(column.name, "RawName");
}

#[test]
fn duplicate_captions_never_bind_edit_contents_or_record_targets() {
    let mut data = data();
    assert_eq!(data.column_index("RawValue"), Some(0));
    assert_eq!(data.column_index("RawItems"), Some(1));
    assert_eq!(data.column_index("Friendly"), None);
    data.columns[0].caption = Some("RawItems".into());
    assert_eq!(data.column_index("RawItems"), Some(1));
    let mut edit = data.edit_target(0, 0).unwrap();
    let mut contents = data.contents_target(0, 1).unwrap();
    assert_eq!(edit.column, "RawValue");
    assert_eq!(contents.column, "RawItems");
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![],
    });
    state.set_table_data(data);
    assert!(state.can_edit_cell(&edit));
    assert!(state.can_view_contents(&contents));
    let mut record = RecordTarget {
        document_revision: 7,
        table: "Items".into(),
        parent_ordinal: 0,
        column: "RawValue".into(),
    };
    assert!(state.can_navigate_record(&record));
    edit.column = "Friendly".into();
    contents.column = "Friendly".into();
    record.column = "Friendly".into();
    assert!(!state.can_edit_cell(&edit));
    assert!(!state.can_view_contents(&contents));
    assert!(!state.can_navigate_record(&record));
}

#[test]
fn actual_headers_show_captions_and_accessible_context_keeps_original_names() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let data = data();
    let mut selection = datasheet::Selection::default();
    selection.reset(&data);
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 400.0),
            )),
            ..Default::default()
        },
        |ui| {
            assert!(
                datasheet::show(
                    ui,
                    &data,
                    &mut selection,
                    crate::workspace::focus::Pane::Document
                )
                .is_empty()
            );
        },
    );
    output.textures_delta.clear();
    assert_eq!(output.shapes.iter().filter(|shape| {
        matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == "Friendly")
    }).count(), 2);
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual accessible tree");
    for name in ["RawValue", "RawItems"] {
        let description = format!("Field name: \u{2068}{name}\u{2069}");
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.value() == Some("Friendly") && node.description() == Some(description.as_str())
        }));
        let cell = format!("\u{2068}{name}\u{2069}, row \u{2068}1\u{2069}:");
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| { node.value().is_some_and(|value| value.starts_with(&cell)) })
        );
    }
}
