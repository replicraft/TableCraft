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
