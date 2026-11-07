use super::*;
use crate::workspace::query_design::sources::tests::session;
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::DesignSource;

fn label(prefix: &str, right: &str) -> String {
    format!(
        "{prefix}: \u{2068}[Customer].[CustomerID]\u{2069} ↔ \u{2068}[{right}].[CustomerID]\u{2069}"
    )
}

#[test]
fn qualified_controls_change_and_remove_only_the_chosen_join_and_obey_disabled_state() {
    let mut state = session();
    state.draft.sources[0].alias = Some("Customer".into());
    state.draft.sources[1].alias = Some("Shipping".into());
    state.draft.sources.push(DesignSource {
        id: 3,
        table: "Orders".into(),
        alias: Some("Billing".into()),
    });
    state.draft.joins[0].right.column = "CustomerID".into();
    let mut second = state.draft.joins[0].clone();
    second.right.source_id = 3;
    state.draft.joins.push(second);
    let original = state.draft.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 300.0))
        .build_ui_state(
            |ui, state: &mut (Session, bool, bool)| {
                ui.add_enabled_ui(state.1, |ui| {
                    state.2 |= show(ui, &mut state.0);
                });
            },
            (state, true, false),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    assert_eq!(harness.state().0.draft, original);
    assert!(!harness.state().2);

    let billing_kind = label("Join type", "Billing");
    let shipping_kind = label("Join type", "Shipping");
    harness
        .get_by_role_and_label(Role::ComboBox, &billing_kind)
        .click();
    harness.run_steps(2);
    harness.get_by_label("All right rows").click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.draft.joins[0], original.joins[0]);
    assert_eq!(harness.state().0.draft.joins[1].kind, DesignJoinKind::Right);
    assert_eq!(
        harness.state().0.draft.joins[1].left,
        original.joins[1].left
    );
    assert_eq!(
        harness.state().0.draft.joins[1].right,
        original.joins[1].right
    );

    harness
        .get_by_role_and_label(Role::Button, &label("Remove join", "Billing"))
        .click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.draft.joins, [original.joins[0].clone()]);
    assert!(harness.state().2);
    assert!(
        harness
            .query_by_role_and_label(Role::ComboBox, &billing_kind)
            .is_none()
    );

    harness.state_mut().1 = false;
    harness.state_mut().2 = false;
    harness.run_steps(2);
    let disabled = harness.get_by_role_and_label(Role::ComboBox, &shipping_kind);
    assert!(disabled.accesskit_node().is_disabled());
    disabled.click_accesskit();
    harness
        .get_by_role_and_label(Role::Button, &label("Remove join", "Shipping"))
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().0.draft.joins, [original.joins[0].clone()]);
    assert!(!harness.state().2);
    assert!(harness.query_by_label("All right rows").is_none());
}
