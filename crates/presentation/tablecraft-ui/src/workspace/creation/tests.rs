use super::*;
use crate::Database;
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn exact_creation_request_owns_pending_and_terminal_publication_facts() {
    let mut state = State {
        creation_available: true,
        ..State::default()
    };
    state.open_creation();
    assert!(state.request_database_creation().is_none());
    assert!(state.set_creation_destination("/chosen/Database1.accdb".into()));
    let request = state.request_database_creation().unwrap();
    assert!(state.set_creation_destination("/chosen/Changed.accdb".into()));
    assert!(!state.begin_database_creation(&request));

    let request = state.request_database_creation().unwrap();
    assert!(state.begin_database_creation(&request));
    assert!(state.creation_is_pending());
    assert!(state.busy);
    assert!(state.request_database_creation().is_none());
    assert!(!state.begin_database_creation(&request));
    assert!(!state.set_creation_destination("/other/file.accdb".into()));
    assert!(!state.finish_database_creation(request.id + 1, CreationOutcome::Created));
    assert!(state.mark_creation_cancelling(request.id));
    assert_eq!(state.creation_status_key(), Some("creation-cancelling"));
    assert!(state.finish_database_creation(request.id, CreationOutcome::VisibilityUncertain));
    assert!(!state.busy);
    assert!(!state.creation_is_pending());
    assert_eq!(state.creation_destination(), "/chosen/Changed.accdb");
    assert_eq!(state.creation_status_key(), Some("creation-uncertain"));
    assert!(state.request_database_creation().is_none());
    assert!(!state.finish_database_creation(request.id, CreationOutcome::NotPublished));
}

#[test]
fn unsupported_hosts_bad_extensions_and_oversized_destinations_never_emit_requests() {
    let mut state = State::default();
    state.open_creation();
    assert!(!state.creation_form_open());
    assert!(state.set_creation_destination("/chosen/file.accdb".into()));
    assert!(state.request_database_creation().is_none());
    state.creation_available = true;
    state.open_creation();
    assert!(state.set_creation_destination("/chosen/file.mdb".into()));
    assert!(state.request_database_creation().is_none());
    assert!(!state.set_creation_destination("x".repeat(4097)));
    assert_eq!(state.creation_destination(), "/chosen/file.mdb");
}

#[test]
fn blank_card_opens_inert_form_then_explicit_create_captures_selected_destination() {
    for theme in [crate::Theme::Light, crate::Theme::Dark] {
        let state = State {
            creation_available: true,
            theme,
            ..State::default()
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1024.0, 720.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "Blank database")
            .click();
        harness.run_steps(2);
        assert_eq!(harness.state().0.backstage, Backstage::New);
        assert!(harness.state().1.is_empty());
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Create")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            harness
                .query_by_role_and_label(Role::TextInput, "File name")
                .is_some()
        );
        harness
            .get_by_role_and_label(Role::Button, "Browse for a location")
            .click();
        harness.run_steps(2);
        assert!(matches!(
            harness.state().1.as_slice(),
            [Action::ChooseCreationLocation]
        ));
        harness.state_mut().1.clear();
        assert!(
            harness
                .state_mut()
                .0
                .set_creation_destination("/chosen/Test.accdb".into())
        );
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Create")
            .click();
        harness.run_steps(2);
        let [Action::CreateDatabase { request }] = harness.state().1.as_slice() else {
            panic!("only the explicit Create click may request publication");
        };
        assert_eq!(request.destination, "/chosen/Test.accdb");
    }
}

#[test]
fn oversized_unicode_paste_preserves_whole_selected_destination() {
    let mut state = State {
        creation_available: true,
        ..State::default()
    };
    state.open_creation();
    assert!(state.set_creation_destination("/chosen/Original.accdb".into()));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1024.0, 720.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "File name")
        .focus();
    harness.step();
    harness.event(egui::Event::Paste("界".repeat(1400)));
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.creation_destination(),
        "/chosen/Original.accdb"
    );
    assert!(harness.state().0.creation.selected);
    assert!(harness.state().0.creation.path_too_long);
    assert!(harness.state().1.is_empty());
    assert!(harness.state_mut().0.request_database_creation().is_some());
}

#[test]
fn opened_database_without_user_tables_is_not_the_open_welcome_surface() {
    let mut state = State {
        database: Some(Database {
            title_path: None,
            document_revision: 1,
            title: "Blank".into(),
            format: "Access 2016".into(),
            tables: Vec::new(),
            queries: Vec::new(),
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1024.0, 720.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(
        harness
            .query_by_label("This database has no user tables.")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Creating tables is not available yet.")
            .is_some()
    );
    assert!(harness.state().1.is_empty());
}
