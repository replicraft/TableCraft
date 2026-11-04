use super::*;
use crate::{Cell, Database, Schema};

fn state() -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title: "Sample".into(),
            format: "ACE16".into(),
            tables: vec![],
        }),
        ..Default::default()
    };
    state.set_table_data(Data {
        source_revision: Some(7),
        table: "Original table".into(),
        offset: 100,
        columns: vec!["Original column".into()],
        rows: vec![vec![Cell {
            text: "Not an identity".into(),
            kind: CellKind::Multivalue {
                assigned: true,
                members: 1,
            },
        }]],
        has_more: false,
    });
    state.set_query_data(data());
    state.schema = Some(Schema {
        table: "Original table".into(),
        declared_rows: 101,
        fields: vec![],
    });
    state.dismiss_backstage();
    state
}

fn data() -> Data {
    Data {
        source_revision: None,
        table: "Contents".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell {
            text: "NULL".into(),
            kind: CellKind::Scalar {
                is_null: true,
                stored_calculation: false,
            },
        }]],
        has_more: false,
    }
}

fn target(state: &State) -> ContentsTarget {
    state.data.as_ref().unwrap().contents_target(0, 0).unwrap()
}

#[test]
fn target_binds_original_names_revision_and_global_parent_not_summary_text() {
    let mut state = state();
    let original = target(&state);
    assert_eq!(original.document_revision, 7);
    assert_eq!(original.table, "Original table");
    assert_eq!(original.column, "Original column");
    assert_eq!(original.parent_ordinal, 100);
    assert!(state.can_view_contents(&original));
    state.data.as_mut().unwrap().rows[0][0].text = "Different summary".into();
    assert_eq!(target(&state), original);
    state.data.as_mut().unwrap().source_revision = None;
    assert!(state.data.as_ref().unwrap().contents_target(0, 0).is_none());
    assert!(!state.can_view_contents(&original));
    state.data.as_mut().unwrap().source_revision = Some(7);
    state.data.as_mut().unwrap().rows[0][0].kind = CellKind::Scalar {
        is_null: false,
        stored_calculation: true,
    };
    assert!(state.data.as_ref().unwrap().contents_target(0, 0).is_none());
    assert!(!state.can_view_contents(&original));
}

#[test]
fn contents_publication_preserves_background_and_cannot_reopen_closed_or_stale_dialogs() {
    let mut state = state();
    let target = target(&state);
    state.begin_contents(target.clone(), 11);
    state.set_contents(11, target.clone(), CollectionKind::Multivalue, true, data());
    assert!(matches!(
        state.contents.as_ref().unwrap().result,
        ContentsResult::Ready { .. }
    ));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(state.query_data.as_ref().unwrap().rows[0][0].text, "NULL");
    assert!(state.schema.is_some());
    state.close_contents();
    state.set_contents(11, target.clone(), CollectionKind::Multivalue, true, data());
    assert!(state.contents.is_none());

    state.begin_contents(target.clone(), 12);
    state.fail_contents(11, target.clone(), "Old error".into());
    assert!(matches!(
        state.contents.as_ref().unwrap().result,
        ContentsResult::Loading
    ));
    state.select_view(DocumentView::Query);
    state.set_contents(12, target.clone(), CollectionKind::Multivalue, true, data());
    assert!(state.contents.is_none());
    assert!(state.view == DocumentView::Query);

    state.select_view(DocumentView::Table);
    state.begin_contents(target.clone(), 13);
    state.database.as_mut().unwrap().document_revision = 8;
    state.set_contents(13, target, CollectionKind::Multivalue, true, data());
    assert!(matches!(
        state.contents.as_ref().unwrap().result,
        ContentsResult::Loading
    ));
    state.clear_schema();
    assert!(state.contents.is_none());
}

#[test]
fn modal_owns_parent_shortcuts_and_escape_closes_only_its_request() {
    let context = egui::Context::default();
    let mut state = state();
    state.file_picker_available = true;
    state.busy = true;
    let target = target(&state);
    state.begin_contents(target, 17);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        assert!(state.show(ui).is_empty());
    });
    output.textures_delta.clear();
    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let mut output = context.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::Key {
                    key: egui::Key::O,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                },
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                },
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ui| {
            let actions = state.show(ui);
            assert!(matches!(
                actions.as_slice(),
                [Action::CloseContents { request_id: 17 }]
            ));
        },
    );
    output.textures_delta.clear();
    assert!(state.contents.is_none());
}

#[test]
fn dialog_renders_assigned_empty_absent_and_null_member_states_without_actions() {
    assert!(membership(CollectionKind::Attachments, false, 0).starts_with("No assigned"));
    assert!(membership(CollectionKind::Attachments, true, 0).contains("0 complete members"));
    let context = egui::Context::default();
    let mut state = state();
    let target = target(&state);
    for (kind, assigned, empty) in [
        (CollectionKind::Multivalue, true, false),
        (CollectionKind::Attachments, false, true),
        (CollectionKind::Attachments, true, true),
        (CollectionKind::VersionHistory, true, false),
    ] {
        state.begin_contents(target.clone(), 19);
        let mut data = data();
        if empty {
            data.rows.clear();
        }
        state.set_contents(19, target.clone(), kind, assigned, data);
        let mut output = context.run_ui(
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

#[test]
fn settled_contents_datasheet_starts_below_modal_controls_and_metadata() {
    let context = egui::Context::default();
    let mut state = state();
    state.data.as_mut().unwrap().rows[0][0].kind = CellKind::Attachments {
        assigned: true,
        members: 1,
    };
    let target = target(&state);
    state.begin_contents(target.clone(), 23);
    let mut contents = data();
    contents.columns[0] = "Child metadata header".into();
    state.set_contents(23, target, CollectionKind::Attachments, true, contents);

    for frame in 0..4 {
        let mut output = context.run_ui(
            egui::RawInput {
                time: Some(f64::from(frame) * 0.25),
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
        if frame >= 2 {
            let texts: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::epaint::Shape::Text(text) = &shape.shape {
                        Some((
                            text.galley.job.text.as_str(),
                            text.pos.y,
                            text.pos.y + text.galley.size().y,
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            let header_top = texts
                .iter()
                .find(|(text, _, _)| *text == "Child metadata header")
                .expect("actual datasheet header painted")
                .1;
            for label in [
                "Collection contents",
                "Close",
                "Original table · Original column · parent 101",
                "Assigned attachment collection",
                "Stored metadata only.",
            ] {
                let bottom = texts
                    .iter()
                    .filter(|(text, _, _)| text.starts_with(label))
                    .map(|(_, _, bottom)| *bottom)
                    .reduce(f32::max)
                    .unwrap_or_else(|| panic!("modal label not painted: {label}"));
                assert!(
                    header_top >= bottom,
                    "datasheet overlaps {label}: header={header_top}, labelbottom={bottom}"
                );
            }
        }
        output.textures_delta.clear();
    }
}
