//! The desktop consumes real inspected definitions without owning query semantics.
use super::*;
use eframe::App;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tablecraft_testkit::fixtures::access::saved_queries::{
    Fragment, QUERY_ID, database_with_query_flags,
};
use tablecraft_ui::EditTarget;

fn row(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn opened() -> (tempfile::TempDir, PathBuf, Vec<u8>, egui::Context, Desktop) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.accdb");
    let bytes = database_with_query_flags(
        &[
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            Fragment {
                expression: Some("Id"),
                ..row(6)
            },
            Fragment {
                expression: Some("Id >= 2"),
                ..row(8)
            },
        ],
        0,
    );
    std::fs::write(&path, &bytes).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
    finish(&mut desktop, &context);
    assert_eq!(desktop.state.database.as_ref().unwrap().queries.len(), 2);
    assert!(desktop.state.query_data.is_none());
    assert!(desktop.state.selected_saved_target().is_none());
    (directory, path, bytes, context, desktop)
}

fn finish(desktop: &mut Desktop, context: &egui::Context) {
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(10);
    while desktop.state.busy {
        assert!(Instant::now() < deadline, "saved-query task did not finish");
        desktop.logic(context, &mut frame);
        std::thread::yield_now();
    }
    assert!(desktop.state.error.is_none());
}

fn target(desktop: &Desktop) -> SavedQueryTarget {
    let database = desktop.state.database.as_ref().unwrap();
    let query = database
        .queries
        .iter()
        .find(|query| query.name == "Saved")
        .unwrap();
    SavedQueryTarget {
        document_revision: database.document_revision,
        object_id: query.object_id,
        name: query.name.clone(),
    }
}

#[test]
fn inspect_then_run_preserves_the_sql_slot_and_publishes_noneditable_saved_results() {
    let (_directory, path, original, context, mut desktop) = opened();
    desktop.state.sql = "SELECT 99 AS OriginalSqlResult".into();
    desktop.submit(Command::Query {
        sql: desktop.state.sql.clone(),
    });
    finish(&mut desktop, &context);
    assert_eq!(
        desktop.state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );

    let selected = target(&desktop);
    desktop.inspect_saved_query(selected.clone());
    finish(&mut desktop, &context);
    assert_eq!(desktop.state.selected_saved_target(), Some(&selected));
    assert!(
        desktop.state.selected_cell_text().is_none(),
        "inspection runs no query"
    );
    assert!(desktop.state.can_run_saved_query(&selected, &[]));
    assert_eq!(desktop.state.sql, "SELECT 99 AS OriginalSqlResult");

    desktop.run_saved_query(selected, Vec::new());
    finish(&mut desktop, &context);
    assert_eq!(desktop.state.selected_cell_text(), Some("2"));
    assert_eq!(
        desktop.state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    assert!(!desktop.state.can_edit_cell(&EditTarget {
        document_revision: desktop.state.database.as_ref().unwrap().document_revision,
        table: "Saved".into(),
        column: "Id".into(),
        parent_ordinal: 0,
    }));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn stale_saved_actions_cannot_replace_a_new_open_or_dispatch_changed_inputs() {
    let (_directory, _path, _original, context, mut desktop) = opened();
    let stale = target(&desktop);
    desktop.inspect_saved_query(stale.clone());
    desktop.open();
    let generation = desktop.generation;
    desktop.inspect_saved_query(stale.clone());
    assert_eq!(desktop.generation, generation);
    finish(&mut desktop, &context);
    desktop.inspect_saved_query(stale);
    assert_eq!(desktop.generation, generation);
    assert!(desktop.state.selected_saved_target().is_none());

    let current = target(&desktop);
    desktop.inspect_saved_query(current.clone());
    finish(&mut desktop, &context);
    let generation = desktop.generation;
    desktop.run_saved_query(
        current,
        vec![ParameterInput {
            name: "not a declared parameter".into(),
            kind: "text".into(),
            text: "literal input".into(),
            is_null: false,
        }],
    );
    assert_eq!(desktop.generation, generation);
    assert!(!desktop.state.busy);
}
