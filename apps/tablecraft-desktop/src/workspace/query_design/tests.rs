//! Visual query editing uses real metadata/build receipts and explicit execution.
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_display::ScalarPresentation;
use tablecraft_runtime::Response;
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_ui::{ObjectKind, ObjectTarget, QueryDesignCompletion};

fn source() -> Vec<u8> {
    let rows = (1..=205).map(|id| (id, id % 2)).collect::<Vec<_>>();
    mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0])
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
}

fn sql(harness: &mut Harness<'_, Desktop>, sql: &str) {
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text(sql);
    harness.run_steps(2);
}

fn run(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn design(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Design View")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn actual_sql_run_supplies_clock_random_and_session_context_without_edit_identity() {
    let (_directory, path, original, mut harness) = opened(source());
    scratch(&mut harness);
    sql(
        &mut harness,
        "SELECT Date() AS [Today], Time() AS [ClockTime], Now() AS [RightNow], \
         Rnd() AS [Sequence], Rnd(0) AS [Repeated], CurrentUser() AS [UserName]",
    );
    let mut initial_random = None;
    for _ in 0..2 {
        run(&mut harness);
        let data = harness.state().state.query_data.as_ref().unwrap();
        assert_eq!(data.rows.len(), 1);
        assert_eq!(data.columns.len(), 6);
        assert!(data.source_revision.is_none());
        assert!(data.row_ordinals.is_none());
        let row = &data.rows[0];
        let Some(ScalarPresentation::DateTime { calendar, .. }) = row[0].presentation else {
            panic!("Date must retain typed date presentation");
        };
        assert_eq!(
            (calendar.hour(), calendar.minute(), calendar.second()),
            (0, 0, 0)
        );
        let Some(ScalarPresentation::DateTime { calendar, .. }) = row[1].presentation else {
            panic!("Time must retain typed date presentation");
        };
        assert_eq!(
            (calendar.year(), calendar.month(), calendar.day()),
            (1899, 12, 30)
        );
        assert!(matches!(
            row[2].presentation,
            Some(ScalarPresentation::DateTime { .. })
        ));
        assert_eq!(row[3].text, row[4].text);
        if let Some(initial) = &initial_random {
            assert_eq!(&row[3].text, initial);
        } else {
            initial_random = Some(row[3].text.clone());
        }
        assert_eq!(row[5].text, "Admin");
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
}

fn criterion(harness: &mut Harness<'_, Desktop>, index: usize, value: &str) {
    harness
        .get_all_by_role_and_label(Role::TextInput, "Criterion")
        .nth(index)
        .unwrap()
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_all_by_role_and_label(Role::TextInput, "Criterion")
        .nth(index)
        .unwrap()
        .type_text(value);
    harness.run_steps(2);
}

#[test]
fn sql_import_loads_source_fields_and_real_qbe_edits_run_through_the_existing_query_service() {
    let (_directory, path, original, mut harness) = opened(source());
    scratch(&mut harness);
    sql(&mut harness, "SELECT [Id], [Score] FROM [Items]");
    run(&mut harness);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .len(),
        205
    );
    let previous_rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    design(&mut harness);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous_rows
    );
    assert_eq!(
        harness.state().state.query_design_draft().unwrap().sources[0].table,
        "Items"
    );
    harness.get_by_role_and_label(Role::Button, "Id");
    harness.get_by_role_and_label(Role::Button, "Score");

    criterion(&mut harness, 0, ">= 100");
    criterion(&mut harness, 1, "= 0");
    harness
        .get_by_role_and_label(Role::CheckBox, "Show: 2")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_all_by_role(Role::ComboBox)
        .find(|node| !node.accesskit_node().is_disabled())
        .unwrap()
        .click_accesskit();
    harness.run_steps(2);
    harness.get_by_label("Descending").click_accesskit();
    harness.run_steps(2);
    let draft = harness.state().state.query_design_draft().unwrap();
    assert!(!draft.columns[1].show);
    assert_eq!(draft.columns[1].criteria[0], "= 0");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous_rows
    );
    run(&mut harness);
    let data = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(data.columns.len(), 1);
    assert_eq!(data.columns[0].name, "Id");
    assert_eq!(data.rows.len(), 53);
    assert_eq!(data.rows.first().unwrap()[0].text, "204");
    assert_eq!(data.rows.last().unwrap()[0].text, "100");
    assert!(data.source_revision.is_none());
    assert!(data.row_ordinals.is_none());
    assert!(!data.has_more);
    assert!(harness.state().state.sql.contains("ORDER BY"));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn unsupported_and_malformed_sql_import_preserve_the_literal_editor_and_previous_results() {
    let (_directory, path, original, mut harness) = opened(source());
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS [Kept]");
    run(&mut harness);
    let previous_rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    for input in [
        "SELECT [Id] FROM [Items] UNION SELECT [Id] FROM [Items]",
        "SELECT [Id] FROM",
    ] {
        sql(&mut harness, input);
        design(&mut harness);
        assert_eq!(harness.state().state.sql, input);
        assert_eq!(
            harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .as_ptr(),
            previous_rows
        );
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
        harness.get_by_role_and_label(Role::MultilineTextInput, "SQL query");
        if input.contains("UNION") {
            let labels = harness
                .get_all_by_role(Role::Label)
                .filter_map(|node| node.value())
                .collect::<Vec<_>>();
            assert!(
                labels.iter().any(|value| {
                    value.starts_with("unsupported clause or extra statement at ")
                }),
                "SQL-only UNION must expose the parser diagnosis in a visible label: {labels:?}"
            );
        }
        assert!(!harness.state().state.busy);
        assert!(harness.state().state.query_design_request_id().is_none());
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn cancelled_real_build_receipt_cannot_run_after_the_sql_object_is_closed_and_reopened() {
    let (_directory, path, original, mut harness) = opened(source());
    scratch(&mut harness);
    sql(&mut harness, "SELECT [Id] FROM [Items]");
    run(&mut harness);
    design(&mut harness);
    let request = harness.state().state.query_design_request().unwrap();
    let draft = harness.state().state.query_design_draft().unwrap().clone();
    let old_tab = harness.state().state.active_object().unwrap();
    let previous_rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    harness.state_mut().build_query_design(
        request.clone(),
        draft.clone(),
        QueryDesignCompletion::Run,
    );
    let request_id = harness.state().generation;
    assert!(harness.state().state.busy);

    // Capture a real completed worker receipt before the UI consumes it. This
    // models cancellation winning the publication race, without fake task hooks.
    let deadline = Instant::now() + Duration::from_secs(10);
    let (generation, result) = loop {
        assert!(
            Instant::now() < deadline,
            "native design build did not finish"
        );
        if let Some(reply) = harness.state().worker.poll() {
            break reply;
        }
        std::thread::yield_now();
    };
    assert_eq!(generation, request_id);
    let Response::QueryDesignBuilt {
        target,
        built,
        metadata,
    } = result.unwrap()
    else {
        panic!("the actual build must return its canonical receipt");
    };
    harness.state_mut().cancel_query_design(request_id);
    assert!(!harness.state().state.busy);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous_rows
    );
    harness.state_mut().close_object(old_tab.clone());
    scratch(&mut harness);
    assert_ne!(harness.state().state.active_object().unwrap(), old_tab);
    sql(&mut harness, "SELECT 77 AS [NewScratch]");
    let generation = harness.state().generation;
    harness
        .state_mut()
        .publish_query_design_built(request_id, target, built, metadata);
    harness
        .state_mut()
        .build_query_design(request, draft, QueryDesignCompletion::Run);
    harness.run_steps(2);
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.sql, "SELECT 77 AS [NewScratch]");
    assert!(harness.state().state.query_data.is_none());
    assert!(!harness.state().state.busy);
    assert_eq!(std::fs::read(path).unwrap(), original);
}
