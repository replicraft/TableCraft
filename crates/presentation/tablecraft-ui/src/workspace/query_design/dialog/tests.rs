use crate::preview::{Preview, Scene};
use crate::workspace::{Action, State};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn unsupported_source_tabs_stay_disabled_and_close_keeps_the_existing_design() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    let original = state.query_design_draft().unwrap().clone();
    state.show_query_design_tables();

    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();

    for label in ["Queries", "Both"] {
        // The Navigation Pane also has a Queries button. The source-kind tabs
        // are selectable controls; target that contract inside the modal.
        let source_tab = harness
            .get_all_by_role_and_label(Role::Button, label)
            .find(|node| node.accesskit_node().toggled().is_some())
            .expect("selectable source-kind tab");
        assert!(source_tab.accesskit_node().is_disabled());
    }
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.query_design_draft(), Some(&original));

    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run_steps(1);
    assert!(!harness.state().0.query_design_dialog_open());
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.query_design_draft(), Some(&original));
}

#[test]
fn borrowed_table_list_keeps_selection_and_rechecks_removed_or_unavailable_sources() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    state.database.as_mut().unwrap().tables = vec![
        crate::Table {
            name: "System".into(),
            local: true,
            system: true,
            storage: "Local".into(),
        },
        crate::Table {
            name: "Linked".into(),
            local: false,
            system: false,
            storage: "Linked".into(),
        },
        crate::Table {
            name: "Alpha".into(),
            local: true,
            system: false,
            storage: "Local".into(),
        },
        crate::Table {
            name: "Beta".into(),
            local: true,
            system: false,
            storage: "Local".into(),
        },
    ];
    state.show_query_design_tables();
    state.query_design.session.as_mut().unwrap().selected_table = Some("Beta".into());

    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness
            .state()
            .0
            .query_design
            .session
            .as_ref()
            .unwrap()
            .selected_table
            .as_deref(),
        Some("Beta")
    );
    harness.get_by_role_and_label(Role::Button, "Add").click();
    harness.run_steps(1);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::DescribeQueryDesign { table, .. }] if table == "Beta"
    ));

    harness.state_mut().1.clear();
    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .tables
        .retain(|table| table.name != "Beta");
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    harness.get_by_role_and_label(Role::Button, "Add").click();
    harness.run_steps(1);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::DescribeQueryDesign { table, .. }] if table == "Alpha"
    ));

    harness.state_mut().1.clear();
    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .tables
        .retain(|table| !table.local || table.system);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Add")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .state()
            .0
            .query_design
            .session
            .as_ref()
            .unwrap()
            .selected_table
            .is_none()
    );
}
