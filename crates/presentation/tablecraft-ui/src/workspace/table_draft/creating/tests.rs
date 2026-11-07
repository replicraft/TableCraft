use crate::workspace::{
    Action, Database, DocumentView, ObjectKind, ObjectTarget, State, Table, ribbon::Tab,
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> State {
    let mut state = State {
        table_save_available: true,
        database: Some(Database {
            document_revision: 71,
            title: "Blank database".into(),
            title_path: None,
            format: "Access 2016".into(),
            tables: Vec::new(),
            queries: Vec::new(),
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state
}

#[test]
fn qualification_is_explicit_revision_bound_and_preserved_by_inert_navigation() {
    let mut state = state();
    assert!(!state.can_create_first_table());
    state.set_first_table_creation_available(70, true);
    assert!(!state.can_create_first_table());
    state.set_first_table_creation_available(71, true);
    assert!(state.can_create_first_table());
    state.set_first_table_creation_available(70, false);
    assert!(state.can_create_first_table());
    state.show_open();
    assert!(!state.can_create_first_table());
    state.dismiss_backstage();
    state.clear_schema();
    assert!(state.can_create_first_table());
    state.busy = true;
    assert!(!state.can_create_first_table());
    state.busy = false;
    state.view = DocumentView::Design;
    assert!(state.can_create_first_table());
    state.view = DocumentView::Query;
    assert!(state.can_create_first_table());
    state.view = DocumentView::Table;
    state.table_save_available = false;
    assert!(!state.can_create_first_table());
    state.table_save_available = true;
    state.database.as_mut().unwrap().document_revision = 72;
    assert!(!state.can_create_first_table());
    state.set_first_table_creation_available(72, true);
    assert!(state.can_create_first_table());
    state.set_first_table_creation_available(72, false);
    assert!(!state.can_create_first_table());
}

#[test]
fn create_table_click_uses_the_same_datasheet_draft_in_normal_and_synthetic_hosts() {
    for synthetic in [false, true] {
        let mut state = state();
        state.mock_preview = synthetic;
        state.set_first_table_creation_available(71, true);
        let scratch = state
            .prepare_object_open(ObjectTarget {
                document_revision: 71,
                kind: ObjectKind::Sql,
            })
            .unwrap()
            .tab;
        state.sql = "SELECT 41 + 1 AS [Kept scratch text]".into();
        assert!(state.view == DocumentView::Query);
        assert!(state.can_create_first_table());
        state.ribbon_tab = Tab::Create;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 820.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Table Design")
                .accesskit_node()
                .is_disabled()
        );
        harness
            .get_by_role_and_label(Role::Button, "Table")
            .click_accesskit();
        harness.run_steps(2);
        let state = &harness.state().0;
        assert!(state.view == DocumentView::Table);
        assert!(matches!(
            state.active_object().unwrap().target.kind,
            ObjectKind::DraftTable { .. }
        ));
        assert!(state.has_unsaved_table_drafts());
        assert!(!state.can_create_first_table());
        assert!(state.database.as_ref().unwrap().tables.is_empty());
        assert!(state.data.is_none());
        assert!(harness.state().1.is_empty());
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Click to Add")
                .is_focused()
        );
        assert!(harness.state_mut().0.activate_object(&scratch));
        assert_eq!(harness.state().0.active_object(), Some(scratch));
        assert_eq!(
            harness.state().0.sql,
            "SELECT 41 + 1 AS [Kept scratch text]"
        );
        assert!(harness.state().0.view == DocumentView::Query);
        assert!(harness.state().0.has_unsaved_table_drafts());
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn unqualified_or_populated_database_never_dispatches_create_table() {
    for qualified in [false, true] {
        let mut state = state();
        if qualified {
            state.set_first_table_creation_available(71, true);
            state.database.as_mut().unwrap().tables.push(Table {
                name: "Existing".into(),
                local: true,
                system: false,
                storage: "Local".into(),
            });
        }
        state.ribbon_tab = Tab::Create;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 820.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        let table = harness.get_by_role_and_label(Role::Button, "Table");
        assert!(table.accesskit_node().is_disabled());
        table.click_accesskit();
        harness.run_steps(2);
        assert!(!harness.state().0.has_unsaved_table_drafts());
        assert!(harness.state().1.is_empty());
    }
}
