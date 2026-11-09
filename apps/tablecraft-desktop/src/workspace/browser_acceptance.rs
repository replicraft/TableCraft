//! Cross-view read-only browser acceptance over an independent multirow image.
use super::test_support::{choose_total, finish_reads, focus_cell, opened, show_totals};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_ui::SortSpec;

#[test]
fn later_filtered_page_and_complete_totals_survive_sql_and_schema_visits() {
    let records = (1..=205).map(|id| (id, id % 2)).collect::<Vec<_>>();
    let original = mutation_fixture::with_rows_and_sort_order(&records, [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(original);
    let table = harness.state().state.active_object().unwrap();
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 100);

    focus_cell(&mut harness, "Score", "2", "0");
    harness
        .get_by_role_and_label(Role::Button, "Selection")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Equals selected value")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().filter.as_ref().unwrap().matched_rows(), 102);

    focus_cell(&mut harness, "Id", "1", "2");
    harness
        .get_by_role_and_label(Role::Button, "Descending")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let filtered = harness.state().state.active_filter_target().unwrap();
    assert_eq!(
        harness.state().state.table_sort(),
        Some(SortSpec {
            column: "Id".into(),
            descending: true,
        })
    );

    harness
        .get_by_role_and_label(Role::Button, "Page tools")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Next page ›")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.offset, 100);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0][0].text, "4");
    assert_eq!(page.rows[1][0].text, "2");
    assert_eq!(page.parent_ordinal(0), Some(3));
    assert_eq!(page.parent_ordinal(1), Some(1));
    assert!(!page.has_more);

    show_totals(&mut harness);
    choose_total(&mut harness, "Id", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "10506",
        "the total covers all 102 filtered records, not the two visible records"
    );

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.sql.is_empty());
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text("SELECT Count(*) AS Total FROM Items");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let query = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(query.rows.len(), 1);
    assert_eq!(query.rows[0][0].text, "205");
    assert!(query.row_ordinals.is_none());

    harness
        .get_by_role_and_label(Role::Tab, "Items")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().state.active_object(), Some(table.clone()));
    assert_eq!(
        harness.state().state.active_filter_target(),
        Some(filtered.clone())
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 100);
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "10506"
    );

    harness
        .get_by_role_and_label(Role::Button, "Design view")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.schema.as_ref().unwrap().table,
        "Items"
    );
    assert_eq!(harness.state().state.active_object(), Some(table.clone()));

    harness
        .get_by_role_and_label(Role::Button, "Datasheet view")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let restored = harness.state().state.data.as_ref().unwrap();
    assert_eq!(restored.offset, 100);
    assert_eq!(restored.rows.len(), 2);
    assert_eq!(restored.parent_ordinal(0), Some(3));
    assert_eq!(restored.parent_ordinal(1), Some(1));
    assert_eq!(harness.state().state.active_object(), Some(table));
    assert_eq!(harness.state().state.active_filter_target(), Some(filtered));
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "10506"
    );
    assert!(harness.state().state.query_data.is_none());

    harness
        .get_by_role_and_label(Role::Tab, "Query")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "205",
        "the native table's filtered page must not replace the independent query result"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn checked_last_record_remains_reachable_after_returning_to_the_first_native_page() {
    let records = (1..=205).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let original = mutation_fixture::with_rows(&records);
    let (_directory, path, original, mut harness) = opened(original);
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness
        .state_mut()
        .navigate_record(tablecraft_ui::RecordTarget {
            document_revision: revision,
            table: "Items".into(),
            parent_ordinal: 200,
            column: "Score".into(),
        });
    finish_reads(&mut harness);
    assert!(!harness.state().state.data.as_ref().unwrap().has_more);

    harness
        .get_by_role_and_label(Role::Button, "First")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 0);

    harness
        .get_by_role_and_label(Role::Button, "Last")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.offset, 204);
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0][0].text, "205");
    assert_eq!(std::fs::read(path).unwrap(), original);
}
