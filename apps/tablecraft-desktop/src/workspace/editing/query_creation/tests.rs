//! Real Name entry, authored owner and consuming native save/reopen journeys.
mod design;
mod failures;
mod unsupported;

use crate::workspace::{
    Desktop,
    test_support::{app_harness, finish_reads, finish_schema_save},
};
use egui::{Context, Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_runtime::{Worker, journal_path};
use tablecraft_testkit::{
    fixtures::access::saved_queries::creation as raw, support::expressions::clock::CountingClock,
};
use tablecraft_ui::{ObjectKind, ObjectTarget};

fn opened() -> (
    tempfile::TempDir,
    PathBuf,
    raw::Source,
    Arc<CountingClock>,
    Harness<'static, Desktop>,
) {
    let date = LegacyDateTime::from_ole_days(45000.5).unwrap();
    let case = raw::source("Created", "Answer", 99, date);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queries.accdb");
    std::fs::write(&path, &case.before).unwrap();
    let reading =
        ClockReading::new(ExtendedDateTime::new(2023, 3, 15, 12, 0, 0, 0).unwrap()).unwrap();
    let clock = Arc::new(CountingClock::new(reading));
    let mut desktop = Desktop::new(Context::default(), None);
    // NO_CLONE: The real worker retains the injected provider while the observer
    // counts reads through actual review, save, guarded reopen and explicit Run.
    desktop.worker = Worker::with_query_clock(|| {}, clock.clone());
    // NO_CLONE: The actual Open flow owns its selected path independently of
    // the filesystem observer retained by this acceptance test.
    desktop.select_file(path.clone());
    desktop.open();
    let mut harness = app_harness(desktop);
    finish_reads(&mut harness);
    (directory, path, case, clock, harness)
}

fn scratch(harness: &mut Harness<'_, Desktop>) {
    let document_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness.state_mut().open_object(ObjectTarget {
        document_revision,
        kind: ObjectKind::Sql,
    });
    harness.run_steps(2);
    finish_reads(harness);
}

fn saved(harness: &mut Harness<'_, Desktop>, name: &str) {
    let database = harness.state().state.database.as_ref().unwrap();
    let query = database
        .queries
        .iter()
        .find(|query| query.name == name)
        .unwrap();
    // NO_CLONE: The actual open action owns a target name after the borrowed
    // catalog lookup ends; the live database retains its independent catalog.
    let target = ObjectTarget {
        document_revision: database.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: query.object_id,
            name: query.name.clone(),
        },
    };
    harness.state_mut().open_object(target);
    harness.run_steps(1);
    finish_reads(harness);
}

fn sql(harness: &mut Harness<'_, Desktop>, text: &str) {
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(2);
}

fn name(harness: &mut Harness<'_, Desktop>, save_as: bool) {
    let modifiers = if save_as {
        Modifiers::COMMAND | Modifiers::SHIFT
    } else {
        Modifiers::COMMAND
    };
    harness.key_press_modifiers(modifiers, Key::S);
    harness.run_steps(2);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !harness.state().state.query_creation_name_open() {
        assert!(
            Instant::now() < deadline,
            "actual Save Build must reach Name entry: {:?}",
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness
        .get_by_role_and_label(Role::TextInput, "Query Name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste("Created".into()));
    harness.run_steps(1);
}

fn accept_name(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(1);
}

fn run_saved(harness: &mut Harness<'_, Desktop>, expected: &str) {
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some(expected));
}

#[test]
fn scratch_name_cancel_then_actual_save_creates_and_runs_new_query_without_replacing_old() {
    let (_directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 7 AS Previous;");
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(&mut harness, "SELECT 99 AS Answer;");
    let tab = harness.state().state.active_object().unwrap();
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    name(&mut harness, false);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.query_creation_name_open());
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        revision
    );
    assert_eq!(clock.calls(), 0);
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    assert!(!journal_path(&path).exists());

    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_eq!(active.id, tab.id);
    assert!(
        matches!(&active.target.kind, ObjectKind::SavedQuery { name, .. } if name == "Created")
    );
    assert!(active.target.document_revision > revision);
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    run_saved(&mut harness, "99");
    saved(&mut harness, "Saved");
    run_saved(&mut harness, "42");
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
}

#[test]
fn dirty_saved_sql_save_as_uses_authored_text_and_retains_the_original_dirty_owner() {
    let (_directory, path, case, clock, mut harness) = opened();
    saved(&mut harness, "Saved");
    // NO_CLONE: The observer retains the old exact stored-editor text across
    // creation publication, independently of both live editor lifetimes.
    let original = harness
        .state()
        .state
        .saved_query_sql()
        .unwrap()
        .0
        .to_string();
    sql(&mut harness, "SELECT 99 AS Answer;");
    let tab = harness.state().state.active_object().unwrap();
    let source_id = match &tab.target.kind {
        ObjectKind::SavedQuery { object_id, .. } => *object_id,
        _ => panic!("expected real saved editor owner"),
    };
    assert!(harness.state().state.has_modified_saved_query_sql());
    name(&mut harness, true);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert_eq!(
        harness.state().state.saved_query_sql().unwrap().1,
        "SELECT 99 AS Answer;"
    );
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);

    name(&mut harness, true);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_ne!(active.id, tab.id);
    let ObjectKind::SavedQuery { object_id, name } = &active.target.kind else {
        panic!("Save As must publish an actual saved destination");
    };
    assert_ne!(*object_id, source_id);
    assert_eq!(name, "Created");
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    run_saved(&mut harness, "99");
    let tabs = harness.state().state.object_tabs();
    let old = tabs
        .into_iter()
        .find(|tab| {
            matches!(&tab.target.kind, ObjectKind::SavedQuery { name, .. } if name == "Saved")
        })
        .expect("original dirty saved editor is still a separately retained owner");
    assert_eq!(old.id, tab.id);
    harness.state_mut().activate_object(old);
    harness.run_steps(2);
    finish_reads(&mut harness);
    let (stored, proposed, _, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(stored, original);
    assert_eq!(proposed, "SELECT 99 AS Answer;");
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Run saved query")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
}
