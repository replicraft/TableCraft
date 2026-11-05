use super::*;
use crate::{Cell, Database, Schema};

fn state() -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title: "Sample".into(),
            format: "ACE16".into(),
            queries: vec![],
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
                boolean: None,
                display_control: crate::DisplayControl::Uninspected,
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
        boolean: None,
        display_control: crate::DisplayControl::Uninspected,
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
    assert!(
        crate::cells::membership(
            &crate::Localizer::default(),
            CollectionKind::Attachments,
            false,
            0,
        )
        .starts_with("No assigned")
    );
    assert!(
        crate::cells::membership(
            &crate::Localizer::default(),
            CollectionKind::Attachments,
            true,
            0,
        )
        .replace(['\u{2068}', '\u{2069}'], "")
        .contains("0 complete members")
    );
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
        for _ in 0..3 {
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
            output.textures_delta.clear();
            assert!(!output.shapes.is_empty());
            if empty {
                let header = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.job.text == "Value" => {
                            Some((
                                egui::Rect::from_min_size(text.pos, text.galley.size()),
                                shape.clip_rect,
                            ))
                        }
                        _ => None,
                    })
                    .expect("ready empty collections retain their declared header");
                assert!(header.1.contains_rect(header.0));
                assert!(!state.can_copy_contents_selection());
                assert!(state.selected_contents_text().is_none());
                let dialog = state.contents.as_ref().unwrap();
                let ContentsResult::Ready {
                    assigned: actual_assigned,
                    data,
                    ..
                } = &dialog.result
                else {
                    panic!("expected ready empty collection");
                };
                assert_eq!(*actual_assigned, assigned);
                assert!(data.rows.is_empty(), "headers do not create a member");
                assert_eq!(data.columns[0].name, "Value");
                assert_eq!(data.source_revision, None);
                assert!(dialog.selection.position().is_none());
            }
        }
    }
}

#[test]
fn settled_contents_datasheet_starts_below_modal_controls_and_metadata() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = state();
    state.data.as_mut().unwrap().rows[0][0].kind = CellKind::Attachments {
        assigned: true,
        members: 1,
    };
    let target = target(&state);
    state.begin_contents(target.clone(), 23);
    let mut contents = data();
    contents.columns = vec![
        "Child metadata header".into(),
        "FileType".into(),
        "Encoded bytes".into(),
    ];
    contents.rows = vec![vec![
        Cell::scalar("example.bin"),
        Cell::scalar("application/octet-stream"),
        Cell::scalar("128,400"),
    ]];
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
                            text.galley.job.text.replace(['\u{2068}', '\u{2069}'], ""),
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
                .find(|(text, _, _)| text == "Child metadata header")
                .expect("actual datasheet header painted")
                .1;
            let mime = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text)
                        if text.galley.job.text == "application/octet-stream" =>
                    {
                        Some((text, shape.clip_rect))
                    }
                    _ => None,
                })
                .expect("literal attachment media type");
            assert!(
                !mime.0.galley.elided,
                "bounded initial sizing fits the MIME value"
            );
            assert!(
                mime.1
                    .contains_rect(egui::Rect::from_min_size(mime.0.pos, mime.0.galley.size()))
            );
            let tree = output.platform_output.accesskit_update.as_ref().unwrap();
            let last_column = tree
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    let value = node.value()?.replace(['\u{2068}', '\u{2069}'], "");
                    value
                        .starts_with("Encoded bytes, row 1:")
                        .then(|| node.bounds())
                        .flatten()
                })
                .expect("actual remaining-width collection field");
            assert!(
                last_column.x1 - last_column.x0 > 400.0,
                "the final collection column still consumes remaining width"
            );
            assert_eq!(state.selected_contents_text(), Some("example.bin"));
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

#[test]
fn collection_widths_are_shared_by_parent_rows_but_isolated_between_fields() {
    fn frame(context: &egui::Context, state: &mut State, events: Vec<egui::Event>) -> egui::Rect {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 820.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                assert!(state.show(ui).is_empty());
            },
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let bounds = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                let value = node.value()?.replace(['\u{2068}', '\u{2069}'], "");
                value
                    .starts_with("Metadata, row 1:")
                    .then(|| node.bounds())
                    .flatten()
            })
            .expect("visible child metadata field");
        egui::Rect::from_min_max(
            egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
            egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    fn open(state: &mut State, row: usize, column: usize, request: u64) {
        state.close_contents();
        let target = state
            .data
            .as_ref()
            .unwrap()
            .contents_target(row, column)
            .unwrap();
        state.begin_contents(target.clone(), request);
        let mut contents = data();
        contents.columns = vec!["Metadata".into(), "Value".into()];
        contents.rows = vec![vec![Cell::scalar("short"), Cell::scalar("tail")]];
        state.set_contents(request, target, CollectionKind::Multivalue, true, contents);
    }

    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = state();
    let parent = state.data.as_mut().unwrap();
    parent.columns.push("Other column".into());
    for _ in 0..2 {
        let cells = (0..2)
            .map(|_| Cell {
                text: "One member".into(),
                kind: CellKind::Multivalue {
                    assigned: true,
                    members: 1,
                },
            })
            .collect();
        parent.rows.push(cells);
    }
    parent.rows.remove(0);

    open(&mut state, 0, 0, 31);
    for _ in 0..3 {
        frame(&context, &mut state, vec![]);
    }
    let original = frame(&context, &mut state, vec![]);
    let start = egui::pos2(original.right(), original.center().y);
    frame(&context, &mut state, vec![egui::Event::PointerMoved(start)]);
    frame(
        &context,
        &mut state,
        vec![egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let end = start + egui::vec2(90.0, 0.0);
    frame(&context, &mut state, vec![egui::Event::PointerMoved(end)]);
    frame(
        &context,
        &mut state,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let resized = frame(&context, &mut state, vec![]);
    assert!(resized.width() > original.width() + 60.0);

    open(&mut state, 0, 1, 32);
    for _ in 0..3 {
        frame(&context, &mut state, vec![]);
    }
    let other = frame(&context, &mut state, vec![]);
    assert!((other.width() - original.width()).abs() < 1.0);

    open(&mut state, 1, 0, 33);
    for _ in 0..3 {
        frame(&context, &mut state, vec![]);
    }
    let revisited = frame(&context, &mut state, vec![]);
    assert!((revisited.width() - resized.width()).abs() < 1.0);
}
