use super::*;
use crate::{Data, Database, Table};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_display::DateFormat;

fn state(value: String, native: bool) -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 1,
        title: "Zoom sample".into(),
        title_path: None,
        format: "ACE 16".into(),
        tables: vec![Table {
            local: true,
            name: "Items".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    let data = Data {
        row_ordinals: None,
        source_revision: native.then_some(1),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Memo".into()],
        rows: vec![vec![Cell::scalar(value)]],
        has_more: false,
    };
    if native {
        state.set_table_data(data);
    } else {
        state.set_query_data(None, data);
        state.select_view(DocumentView::Query);
    }
    state
}

fn harness(state: State) -> Harness<'static, (State, Vec<crate::Action>)> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, state: &mut (State, Vec<crate::Action>)| {
                state.1.extend(state.0.show(ui));
            },
            (state, Vec::new()),
        )
}

#[test]
fn virtual_zoom_reaches_the_large_suffix_and_copies_complete_original_without_full_frame_layout() {
    let suffix = "FINAL🙂";
    let text = format!(
        "{}{suffix}",
        "line\n".repeat((9 * 1024 * 1024 - suffix.len()) / 5)
    );
    let mut state = state(text, true);
    let original = state.data.as_ref().unwrap().rows[0][0].text.as_ptr();
    assert!(state.open_zoom_cell());
    assert_eq!(state.zoom.session.as_ref().unwrap().index.row_count(), 0);
    let mut harness = harness(state);
    harness.run_steps(3);
    // End is requested before the incremental index reaches EOF. Keep that
    // request live rather than treating the current prefix as the full value.
    harness.key_press(egui::Key::End);
    let mut frames = 0;
    while !harness
        .state()
        .0
        .zoom
        .session
        .as_ref()
        .unwrap()
        .index
        .complete()
    {
        assert!(frames < 4096, "bounded Zoom indexing did not finish");
        harness.run_steps(1);
        frames += 1;
    }
    harness.run_steps(4);
    let suffix_bounds = harness
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == suffix => {
                Some((text.pos, text.galley.size(), shape.clip_rect))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        harness
            .output()
            .shapes
            .iter()
            .any(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => {
                    text.galley.job.text == suffix
                        && shape
                            .clip_rect
                            .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => false,
            }),
        "suffix bounds: {suffix_bounds:?}"
    );
    let painted_bytes = harness
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.job.text.len()),
            _ => None,
        })
        .sum::<usize>();
    assert!(painted_bytes < 256 * 1024);
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().rows[0][0]
            .text
            .as_ptr(),
        original
    );

    harness
        .get_by_role_and_label(Role::Button, "Copy complete value")
        .click_accesskit();
    let mut published = false;
    for _ in 0..4096 {
        harness.run_steps(1);
        let expected = &harness.state().0.data.as_ref().unwrap().rows[0][0].text;
        assert!(expected.len() > 8 * 1024 * 1024);
        if harness.output().platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == expected),
        ) {
            published = true;
            break;
        }
    }
    assert!(
        published,
        "complete source-bound clipboard copy did not finish"
    );
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.zoom_dialog_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn native_edit_delegates_only_the_exact_target_and_query_zoom_remains_read_only() {
    for native in [true, false] {
        let mut state = state("€ 1.234,57\r\nLiteral display text".into(), native);
        assert!(state.open_zoom_cell());
        let mut harness = harness(state);
        harness.run_steps(3);
        if native {
            harness
                .get_by_role_and_label(Role::Button, "Edit through review…")
                .click_accesskit();
            harness.run_steps(1);
            assert!(
                matches!(harness.state().1.as_slice(), [crate::Action::BeginEdit { target }]
                if target.document_revision == 1 && target.table == "Items" && target.column == "Memo" && target.parent_ordinal == 0)
            );
            assert!(!harness.state().0.zoom_dialog_open());
        } else {
            assert!(
                harness
                    .query_all_by_role_and_label(Role::Button, "Edit through review…")
                    .next()
                    .is_none()
            );
            harness.get_by_label("Query results are read only.");
            harness
                .get_by_role_and_label(Role::Button, "Cancel")
                .click_accesskit();
            harness.run_steps(1);
            assert!(!harness.state().0.zoom_dialog_open());
            assert!(harness.state().1.is_empty());
        }
    }
}

#[test]
fn publication_and_format_changes_close_zoom_while_theme_and_language_keep_its_binding() {
    let mut state = state("full value".into(), true);
    state.synchronize_objects();
    let tab = state
        .active_object()
        .expect("the catalog-backed Items table is open");
    assert!(state.can_close_object(&tab));
    assert!(state.open_zoom_cell());
    assert!(!state.close_object(&tab));
    state.localizer = crate::Localizer::new(crate::Locale::Ja).unwrap();
    state.theme = crate::Theme::Dark;
    state.validate_zoom();
    assert!(state.zoom_dialog_open());
    let replacement = Data {
        row_ordinals: None,
        source_revision: Some(1),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Memo".into()],
        rows: vec![vec![Cell::scalar("replacement")]],
        has_more: false,
    };
    state.set_table_data(replacement);
    state.validate_zoom();
    assert!(!state.zoom_dialog_open());
    assert!(state.open_zoom_cell());
    state.formatting.date = DateFormat::Raw;
    state.validate_zoom();
    assert!(!state.zoom_dialog_open());
    assert_eq!(state.data.as_ref().unwrap().rows[0][0].text, "replacement");
}

#[test]
fn shift_f2_belongs_to_the_actual_cell_and_modal_zoom_suppresses_background_actions() {
    let state = state("selected text".into(), true);
    let mut harness = harness(state);
    let context = harness.ctx.clone();
    harness
        .state_mut()
        .0
        .table_selection
        .focus_selected(&context);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.run_steps(3);
    assert!(harness.state().0.zoom_dialog_open());
    let target = EditTarget {
        document_revision: 1,
        table: "Items".into(),
        column: "Memo".into(),
        parent_ordinal: 0,
    };
    assert!(!harness.state().0.can_edit_cell(&target));
    assert!(!harness.state().0.can_insert_row(&crate::InsertTarget {
        document_revision: 1,
        table: "Items".into(),
    }));
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::O);
    harness.key_press(egui::Key::F6);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.zoom_dialog_open());
}

#[test]
fn closing_or_replacing_a_large_source_discards_incomplete_clipboard_work() {
    for replace in [false, true] {
        let mut state = state("🦀\r\n".repeat(2 * 1024 * 1024), true);
        assert!(state.open_zoom_cell());
        let mut harness = harness(state);
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Copy complete value")
            .click_accesskit();
        harness.run_steps(1);
        assert!(
            harness
                .state()
                .0
                .zoom
                .session
                .as_ref()
                .unwrap()
                .pending_copy
                .is_some()
        );
        if replace {
            harness.state_mut().0.set_table_data(Data {
                row_ordinals: None,
                source_revision: Some(2),
                table: "Items".into(),
                offset: 0,
                columns: vec!["Memo".into()],
                rows: vec![vec![Cell::scalar("replacement")]],
                has_more: false,
            });
        } else {
            harness.key_press(egui::Key::Escape);
        }
        harness.run_steps(2);
        assert!(!harness.state().0.zoom_dialog_open());
        assert!(
            !harness
                .output()
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        assert!(harness.state().1.is_empty());
    }
}
