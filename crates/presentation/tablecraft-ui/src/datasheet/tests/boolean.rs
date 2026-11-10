//! Default checkbox artwork proposes edits without mutating stored cell facts.
use super::*;
use crate::workspace::row_identity::RowOrdinals;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn pointer_space_and_assistive_click_emit_the_same_source_bound_boolean_proposal() {
    for gesture in 0..3 {
        for control in [
            crate::DisplayControl::Absent,
            crate::DisplayControl::Native(106),
        ] {
            let mut cell = Cell::scalar("0");
            cell.kind = CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: Some(false),
                display_control: control,
            };
            let data = Data {
                row_ordinals: Some(RowOrdinals::new(vec![42]).unwrap()),
                source_revision: Some(7),
                table: "Flags".into(),
                offset: 0,
                columns: vec!["Enabled".into()],
                rows: vec![vec![cell]],
                has_more: false,
            };
            let mut selection = Selection::default();
            selection.reset(&data);
            let mut harness = Harness::builder()
                .with_size(egui::vec2(500.0, 240.0))
                .build_ui_state(
                    |ui, state: &mut (Data, Selection, Vec<Action>)| {
                        state.2.extend(
                            show(
                                ui,
                                &state.0,
                                &mut state.1,
                                Pane::Document,
                                Features {
                                    boolean_toggles: true,
                                    ..Features::default()
                                },
                            )
                            .actions,
                        );
                    },
                    (data, selection, Vec::new()),
                );
            crate::configure(&harness.ctx);
            harness.run();
            if gesture == 2 {
                harness
                    .input_mut()
                    .events
                    .push(egui::Event::PointerMoved(egui::pos2(490.0, 230.0)));
                harness.run_steps(1);
            }
            let checkbox = harness.get_by_role(Role::CheckBox);
            assert!(!checkbox.accesskit_node().is_read_only());
            assert!(
                checkbox
                    .accesskit_node()
                    .data()
                    .supports_action(AccessibleAction::Click)
            );
            match gesture {
                0 => checkbox.click(),
                1 => {
                    checkbox.focus();
                    harness.run_steps(1);
                    harness.key_press(egui::Key::Space);
                }
                2 => checkbox.click_accesskit(),
                _ => unreachable!(),
            }
            harness.run_steps(1);
            let [Action::BeginSeededEdit { target, seed }] = harness.state().2.as_slice() else {
                panic!("one Boolean gesture must dispatch one checked edit");
            };
            assert_eq!(target.document_revision, 7);
            assert_eq!(target.table, "Flags");
            assert_eq!(target.column, "Enabled");
            assert_eq!(target.parent_ordinal, 42);
            assert_eq!(
                seed,
                &crate::EditSeed::Boolean(crate::BooleanToggle::opposite(false))
            );
            assert_eq!(harness.state().0.rows[0][0].toggle_value(), Some(false));
            assert_eq!(harness.state().0.rows[0][0].text, "0");
        }
    }
}

#[test]
fn query_and_calculated_checkboxes_remain_readonly_without_toggle_actions() {
    for calculated in [false, true] {
        let data = Data {
            row_ordinals: None,
            source_revision: Some(7),
            table: "Flags".into(),
            offset: 0,
            columns: vec!["Enabled".into()],
            rows: vec![vec![Cell {
                presentation: None,
                alignment: HorizontalAlignment::Left,
                text: "-1".into(),
                kind: CellKind::Scalar {
                    is_null: false,
                    stored_calculation: calculated,
                    boolean: Some(true),
                    display_control: crate::DisplayControl::Absent,
                },
            }]],
            has_more: false,
        };
        let mut selection = Selection::default();
        selection.reset(&data);
        let pane = if calculated {
            Pane::Document
        } else {
            Pane::Results
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(500.0, 240.0))
            .build_ui_state(
                move |ui, state: &mut (Data, Selection, Vec<Action>)| {
                    state.2.extend(
                        show(
                            ui,
                            &state.0,
                            &mut state.1,
                            pane,
                            Features {
                                boolean_toggles: true,
                                ..Features::default()
                            },
                        )
                        .actions,
                    );
                },
                (data, selection, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run();
        let checkbox = harness.get_by_role(Role::CheckBox);
        assert!(checkbox.accesskit_node().is_read_only());
        assert!(
            !checkbox
                .accesskit_node()
                .data()
                .supports_action(AccessibleAction::Click)
        );
        checkbox.click();
        harness.run_steps(1);
        harness.key_press(egui::Key::Space);
        harness.run_steps(1);
        assert!(harness.state().2.is_empty());
        assert_eq!(harness.state().0.rows[0][0].checkbox_value(), Some(true));
    }
}
