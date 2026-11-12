//! Totals edits change a revision-bound draft without running or losing choices.
use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::Action;
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::{DesignColumn, DesignExpression};

fn harness(mock: bool) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = mock;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

#[test]
fn actual_totals_command_adds_and_removes_the_row_without_build_or_run() {
    for mock in [true, false] {
        let mut harness = harness(mock);
        // NO_CLONE: The expected draft must survive mutable Ribbon interactions so
        // the test detects lost choices after toggling; a live borrow cannot span them.
        let before = harness.state().0.query_design_draft().unwrap().clone();
        let revision = harness
            .state()
            .0
            .query_design_request()
            .unwrap()
            .draft_revision;
        harness
            .get_by_role_and_label(Role::Button, "Totals")
            .click_accesskit();
        harness.run_steps(2);
        assert!(harness.state().0.query_design_draft().unwrap().totals);
        harness.get_by_role_and_label(Role::ComboBox, "Total: 1");
        assert_eq!(
            harness
                .state()
                .0
                .query_design_request()
                .unwrap()
                .draft_revision,
            revision + 1,
        );
        assert!(harness.state().1.is_empty());
        assert!(harness.state().0.query_design_request_id().is_none());

        harness
            .get_by_role_and_label(Role::Button, "Totals")
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(harness.state().0.query_design_draft(), Some(&before));
        assert!(
            harness
                .query_by_role_and_label(Role::ComboBox, "Total: 1")
                .is_none()
        );
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn unavailable_toggle_off_is_disabled_and_preserves_draft_revision_and_results() {
    let mut harness = harness(true);
    assert!(harness.state_mut().0.set_query_data(
        None,
        crate::Data {
            query_contents: None,
            source_revision: None,
            row_ordinals: None,
            table: "Previous query".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: vec![vec!["previous".into()]],
            has_more: false,
        }
    ));
    let session = harness.state_mut().0.query_design.session.as_mut().unwrap();
    session.draft.totals = true;
    session.draft.columns[0].total = DesignTotal::Expression;
    session.draft.columns[0].expression = DesignExpression::Expression("Count(*)".into());
    // NO_CLONE: Retain the exact pre-command text and roles independently while
    // the pointer interaction targets the disabled, incomplete editor shape.
    let before = session.draft.clone();
    let revision = session.revision;
    let previous = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness.run_steps(2);
    let toggle = harness.get_by_role_and_label(Role::Button, "Totals");
    let node = toggle.accesskit_node();
    assert!(node.is_disabled());
    assert!(!node.data().supports_action(egui::accesskit::Action::Click));
    let description = node.data().description().unwrap();
    assert!(description.contains(&harness.state().0.localizer.text("ribbon-unavailable")));
    assert!(
        description.contains(
            &harness
                .state()
                .0
                .localizer
                .text("qbe-totals-off-unavailable")
        )
    );
    toggle.click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.query_design_draft(), Some(&before));
    let session = harness.state().0.query_design.session.as_ref().unwrap();
    assert_eq!(session.revision, revision);
    assert!(session.error.is_none());
    assert_eq!(
        harness.state().0.query_data.as_ref().unwrap().rows.as_ptr(),
        previous
    );
    assert!(harness.state().1.is_empty());
    assert!(!harness.state_mut().0.toggle_query_design_totals());
    assert_eq!(harness.state().0.query_design_draft(), Some(&before));
    assert_eq!(
        harness
            .state()
            .0
            .query_design_request()
            .unwrap()
            .draft_revision,
        revision
    );
}

#[test]
fn where_choice_requires_explicit_hidden_unsorted_input_and_preserves_it() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui_state(
            |ui, state: &mut DesignColumn| {
                column(ui, state, 0);
            },
            DesignColumn::field(1, "Score".into()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::ComboBox, "Total: 1")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.get_by_label("Where").accesskit_node().is_disabled());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().show);
    assert_eq!(harness.state().total, DesignTotal::GroupBy);

    harness.state_mut().show = false;
    harness
        .get_by_role_and_label(Role::ComboBox, "Total: 1")
        .click_accesskit();
    harness.run_steps(2);
    harness.get_by_label("Where").click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().total, DesignTotal::Where);
    assert!(!harness.state().show);
    assert!(harness.state().sort.is_none());
    assert_eq!(
        harness.state().expression,
        DesignExpression::Field(tablecraft_builders::query::DesignField {
            source_id: 1,
            column: "Score".into()
        })
    );
}
