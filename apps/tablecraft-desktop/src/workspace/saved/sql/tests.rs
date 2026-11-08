use crate::workspace::Desktop;
use std::time::{Duration, Instant};
use tablecraft_runtime::Command;
use tablecraft_testkit::fixtures::access::saved_queries::fitting_select::storage_pair;
use tablecraft_ui::{
    ObjectKind, ObjectTarget, SavedQuerySqlCloseDecision, SavedQuerySqlEligibility,
};

fn finish(desktop: &mut Desktop, context: &egui::Context) {
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(10);
    while desktop.state.busy {
        assert!(Instant::now() < deadline, "saved SQL inspection timed out");
        desktop.logic(context, &mut frame);
        std::thread::yield_now();
    }
    assert!(desktop.state.error.is_none());
}

#[test]
fn opening_saved_query_inspects_sql_without_executing_editing_or_changing_bytes() {
    let [original, _] = storage_pair();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.accdb");
    std::fs::write(&path, &original).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
    finish(&mut desktop, &context);
    let database = desktop.state.database.as_ref().unwrap();
    let query = database.queries.iter().find(|query| !query.system).unwrap();
    desktop.open_object(ObjectTarget {
        document_revision: database.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: query.object_id,
            name: query.name.clone(),
        },
    });
    finish(&mut desktop, &context);

    let (original_sql, proposed_sql, _, eligibility) = desktop.state.saved_query_sql().unwrap();
    assert_eq!(original_sql, "SELECT 42 AS [Answer];");
    assert_eq!(proposed_sql, original_sql);
    assert_eq!(
        eligibility,
        SavedQuerySqlEligibility::QualifiedConstantSelect
    );
    assert!(!desktop.state.can_edit_saved_query_sql());
    assert!(!desktop.state.can_review_saved_query_sql());
    assert!(!desktop.state.has_modified_saved_query_sql());
    assert!(desktop.state.query_data.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

    // Exercise host transition protection explicitly while production editing
    // remains disabled until its consuming save worker is connected.
    desktop.state.saved_query_sql_edit_available = true;
    assert!(
        desktop
            .state
            .set_saved_query_sql_text("SELECT 43 AS [Answer];".into())
    );
    let generation = desktop.generation;
    desktop.open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    assert_eq!(desktop.generation, generation);
    assert!(desktop.state.saved_query_sql_close_open());
    let cancelled = desktop
        .state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Cancel)
        .unwrap();
    desktop.finish_saved_query_sql_close(cancelled, &context);
    assert!(desktop.deferred_draft_action.is_none());
    assert!(desktop.state.saved_query_sql_dirty());

    desktop.open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    let discarded = desktop
        .state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
        .unwrap();
    desktop.finish_saved_query_sql_close(discarded, &context);
    finish(&mut desktop, &context);
    assert!(desktop.deferred_draft_action.is_none());
    assert!(!desktop.state.has_modified_saved_query_sql());
    assert_ne!(desktop.generation, generation);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
