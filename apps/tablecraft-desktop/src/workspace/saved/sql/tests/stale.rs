//! Deterministic host publication checks, without native requests or filesystem writes.
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{QuerySqlDraft, QuerySqlEligibility};
use tablecraft_ui::{Database, SavedQuery, SavedQuerySqlEligibility, SavedQueryTarget};

fn target(object_id: u128, name: &str) -> SavedQueryTarget {
    SavedQueryTarget {
        document_revision: 7,
        object_id,
        name: name.to_owned(),
    }
}

fn native(target: &SavedQueryTarget, sql: &str) -> QuerySqlDraft {
    QuerySqlDraft {
        target: tablecraft_runtime::SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: target.name.clone(),
        },
        sql: Some(sql.to_owned()),
        eligibility: QuerySqlEligibility::QualifiedExpressionSelect,
    }
}

fn desktop() -> (Desktop, SavedQueryTarget) {
    let target = target(90, "Saved 空");
    let mut desktop = Desktop::new(egui::Context::default(), None);
    desktop.state.saved_query_sql_edit_available = true;
    desktop.state.dismiss_backstage();
    desktop.state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![
            SavedQuery {
                object_id: target.object_id,
                name: target.name.clone(),
                system: false,
            },
            SavedQuery {
                object_id: 91,
                name: "Other".into(),
                system: false,
            },
        ],
    });
    desktop
        .state
        .begin_saved_query_inspection(target.clone(), 1);
    assert!(
        desktop
            .state
            .set_saved_query_details(1, target.clone(), vec![])
    );
    (desktop, target)
}

fn pending(desktop: &mut Desktop, target: &SavedQueryTarget, request_id: u64) {
    let request = desktop
        .state
        .begin_saved_query_sql_inspection(target.clone(), request_id)
        .unwrap();
    desktop.generation = request_id;
    desktop.pending = PendingOperation::SavedSqlInspection {
        request_id,
        request,
    };
}

fn text(desktop: &Desktop) -> (String, String, u64, SavedQuerySqlEligibility) {
    let (original, proposed, revision, eligibility) = desktop.state.saved_query_sql().unwrap();
    (
        original.to_owned(),
        proposed.to_owned(),
        revision,
        eligibility,
    )
}

#[test]
fn wrong_runtime_identity_or_request_id_never_publishes_inspection_sql() {
    let (mut desktop, target) = desktop();
    pending(&mut desktop, &target, 10);
    desktop.publish_saved_query_sql(9, native(&target, "stale request"));
    assert!(desktop.state.saved_query_sql().is_none());
    assert!(desktop.state.saved_query_sql_pending());

    for field in 0..3 {
        let mut wrong = target.clone();
        match field {
            0 => wrong.document_revision += 1,
            1 => wrong.object_id += 1,
            _ => wrong.name.push('x'),
        }
        desktop.publish_saved_query_sql(10, native(&wrong, "foreign owner"));
        assert!(desktop.state.saved_query_sql().is_none());
        assert!(desktop.state.saved_query_sql_pending());
    }
    desktop.publish_saved_query_sql(10, native(&target, "SELECT 42 AS [Answer];"));
    assert_eq!(text(&desktop).0, "SELECT 42 AS [Answer];");
    assert!(!desktop.state.saved_query_sql_pending());
    assert!(!desktop.state.has_modified_saved_query_sql());
}

#[test]
fn late_inspection_cannot_replace_newer_exact_user_text_or_aba_revision() {
    let (mut desktop, target) = desktop();
    pending(&mut desktop, &target, 10);
    desktop.publish_saved_query_sql(10, native(&target, "SELECT 42 AS [Answer];"));
    pending(&mut desktop, &target, 11);
    assert!(
        desktop
            .state
            .set_saved_query_sql_text(" select  43 as [Answer];\n".into())
    );
    let first_revision = text(&desktop).2;
    assert!(
        desktop
            .state
            .set_saved_query_sql_text("SELECT 44 AS [Answer];".into())
    );
    assert!(
        desktop
            .state
            .set_saved_query_sql_text(" select  43 as [Answer];\n".into())
    );
    let expected = text(&desktop);
    assert!(expected.2 > first_revision);
    desktop.publish_saved_query_sql(11, native(&target, "SELECT 42 AS [Answer];"));
    assert_eq!(text(&desktop), expected);
    assert!(desktop.state.has_modified_saved_query_sql());
    assert!(!desktop.state.saved_query_sql_pending());
}

#[test]
fn late_reply_for_parked_dirty_owner_cannot_touch_current_saved_query() {
    let (mut desktop, first) = desktop();
    pending(&mut desktop, &first, 10);
    desktop.publish_saved_query_sql(10, native(&first, "SELECT 42 AS [Answer];"));
    assert!(
        desktop
            .state
            .set_saved_query_sql_text("SELECT 43 AS [Answer];".into())
    );
    let first_tab = desktop.state.active_object().unwrap();
    let first_revision = desktop
        .state
        .saved_query_sql_revision_for(&first_tab)
        .unwrap();
    pending(&mut desktop, &first, 11);

    let second = target(91, "Other");
    desktop
        .state
        .begin_saved_query_inspection(second.clone(), 2);
    assert!(
        desktop
            .state
            .set_saved_query_details(2, second.clone(), vec![])
    );
    let request = desktop
        .state
        .begin_saved_query_sql_inspection(second.clone(), 12)
        .unwrap();
    assert!(desktop.state.set_saved_query_sql_inspection(
        12,
        &request,
        Some("SELECT 7 AS [Other];".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect,
    ));
    let current_tab = desktop.state.active_object().unwrap();
    let expected = text(&desktop);
    // The host still retains the exact first-owner request. The shared owner
    // check must reject its otherwise matching reply while the second is active.
    desktop.publish_saved_query_sql(11, native(&first, "SELECT 42 AS [Answer];"));
    assert_eq!(desktop.state.active_object().unwrap(), current_tab);
    assert_eq!(text(&desktop), expected);
    assert_eq!(
        desktop.state.saved_query_sql_revision_for(&first_tab),
        Some(first_revision)
    );
    assert!(desktop.state.has_modified_saved_query_sql());
    assert!(desktop.state.activate_object(&first_tab));
    assert_eq!(text(&desktop).1, "SELECT 43 AS [Answer];");
    assert!(desktop.state.saved_query_sql_dirty());
}
