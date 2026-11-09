//! Source-free UI ownership checks, not native qualification or fake save authority.
use super::{FieldRenameRequest, FieldRenameTarget};
use crate::workspace::test_support;
use crate::{Action, Column, State};
use egui_kittest::Harness;
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;

pub(in crate::workspace) fn naming() -> (State, FieldRenameTarget) {
    let mut state = test_support::saved_table(
        "Field rename",
        vec![
            Column {
                name: "ID".into(),
                caption: None,
                native_number: Some(0),
            },
            Column {
                name: "Score".into(),
                caption: Some("Friendly".into()),
                native_number: Some(17),
            },
        ],
    );
    state.field_rename_available = true;
    let target = FieldRenameTarget {
        tab: state.active_object().unwrap(),
        column: "Score".into(),
        native_number: 17,
    };
    assert!(state.begin_field_rename(target.copy().unwrap()));
    (state, target)
}

pub(in crate::workspace) fn observed() -> (State, FieldRenameTarget) {
    let (mut state, target) = naming();
    let outgoing = state.field_rename_inspection_target().unwrap();
    let prepared = state.prepare_field_rename_inspection(&outgoing).unwrap();
    prepared.bind(80);
    assert!(state.set_field_rename_inspection_by_id(
        80,
        7,
        "Table1",
        "Score",
        17,
        true,
        Some(FieldRenameNameProfile::SameWidthAscii { units: 5 })
    ));
    (state, target)
}

pub(in crate::workspace) fn accepted(changed: bool) -> (State, FieldRenameRequest) {
    let (mut state, target) = observed();
    if changed {
        assert!(state.set_field_rename_name(&target, "Total".into()));
    }
    let request = state.field_rename_review_request().unwrap();
    assert!(state.begin_field_rename_review(&request, 81));
    assert!(state.accept_field_rename_review(&request, 81, 90));
    (state, request)
}

pub(in crate::workspace::field_rename) fn harness(
    state: State,
) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 720.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}
