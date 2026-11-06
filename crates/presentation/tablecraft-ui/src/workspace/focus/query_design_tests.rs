use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::{Action, ObjectKind};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::{DesignFieldMetadata, DesignTableMetadata, build};

fn harness() -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::QueryDesign);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

#[test]
fn query_sources_and_grid_participate_in_f6_without_running_or_changing_the_draft() {
    let mut harness = harness();
    let original = harness.state().0.query_design_draft().unwrap().clone();
    harness
        .get_by_role_and_label(Role::Tab, "Query Design")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::F6);
    harness.run_steps(2);
    let source = harness
        .get_all_by_label("*")
        .find(|node| node.is_focused())
        .expect("F6 enters a real current source-list response")
        .rect();

    harness.key_press(Key::F6);
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_role_and_label(Role::TextInput, "Field expression")
            .any(|node| node.is_focused()),
        "the next F6 enters the lower query grid"
    );

    harness.key_press_modifiers(Modifiers::SHIFT, Key::F6);
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_label("*")
            .any(|node| node.is_focused() && node.rect() == source),
        "reverse traversal returns to the same source card"
    );
    assert_eq!(harness.state().0.query_design_draft(), Some(&original));
    assert!(harness.state().1.is_empty());

    harness.state_mut().0.busy = true;
    harness
        .get_by_role_and_label(Role::Tab, "Query Design")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::F6);
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_role_and_label(Role::TextInput, "Field expression")
            .all(|node| !node.is_focused() && node.accesskit_node().is_disabled()),
        "busy source/grid controls cannot become pane entry targets"
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn pending_query_pane_entry_cannot_cross_same_object_view_or_object_boundaries() {
    let mut harness = harness();
    let context = harness.ctx.clone();
    let table_preview = Preview::new(Scene::Datasheet);
    let table_data = table_preview.state().data.as_ref().unwrap();
    let metadata = vec![DesignTableMetadata {
        table: table_data.table.clone(),
        fields: table_data
            .columns
            .iter()
            .map(|column| DesignFieldMetadata {
                name: column.name.clone(),
                scalar_type: None,
            })
            .collect(),
    }];
    let state = &mut harness.state_mut().0;
    let design_scope = state.pane_scope();
    let object = state.active_object().unwrap();
    let publication = state.focus_publication;
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = state.show_query_design_sql().unwrap()
    else {
        panic!("SQL View requests the existing checked builder");
    };
    let built = build(&draft, &metadata).unwrap();
    // Use the real checked build transition; no focus widget ID is
    // retained as a target while the build is pending.
    assert!(state.begin_query_design_build(request, draft, completion, 900));
    state.pane_focus_entry = Some(Entry {
        scope: design_scope,
        pane: Pane::QueryGrid,
        focus_witness: context.memory(|memory| memory.focused()),
    });

    assert_eq!(
        state.set_query_design_built(900, built, metadata),
        Some(completion)
    );
    assert_eq!(state.active_object(), Some(object));
    assert_eq!(state.focus_publication, publication);
    assert!(state.view == DocumentView::Query);
    assert!(!state.query_design_visible());
    assert_ne!(state.pane_scope(), design_scope);
    assert!(
        state.take_pane_entry(&context).is_none(),
        "SQL View must reject the deferred Design entry on the same object"
    );

    // Opening another existing object invalidates the deferred entry even when
    // the database revision is unchanged.
    let sql_scope = state.pane_scope();
    state.pane_focus_entry = Some(Entry {
        scope: sql_scope,
        pane: Pane::QueryGrid,
        focus_witness: context.memory(|memory| memory.focused()),
    });
    let table = state
        .object_tabs()
        .into_iter()
        .find(|tab| matches!(tab.target.kind, ObjectKind::Table { .. }))
        .unwrap();
    assert!(state.activate_object(&table));
    assert!(state.take_pane_entry(&context).is_none());
    assert_ne!(state.pane_scope(), sql_scope);
    assert!(harness.state().1.is_empty());
}
