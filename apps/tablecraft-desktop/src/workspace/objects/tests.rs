//! Real desktop object ownership over independent native fixtures.
use crate::workspace::Desktop;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::workspace::test_support::finish_save;
use crate::workspace::test_support::{finish_reads, focus_cell, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::saved_queries::{
    EMPTY_ID, Fragment, QUERY_ID, database_with_query_flags,
};
use tablecraft_testkit::fixtures::access::{
    canonical_multiple_creation, canonical_native17_creation,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_ui::EditTarget;
use tablecraft_ui::{FilterComparison, ObjectKind, ObjectTab, ObjectTarget, SortSpec};

fn table(desktop: &Desktop, name: &str) -> ObjectTarget {
    ObjectTarget {
        document_revision: desktop.state.database.as_ref().unwrap().document_revision,
        kind: ObjectKind::Table { name: name.into() },
    }
}

fn tab(desktop: &Desktop, target: &ObjectTarget) -> ObjectTab {
    desktop
        .state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.target == *target)
        .expect("the specific object has an open tab")
}

fn multiple_tables() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_multiple_creation::source(canonical_multiple_creation::Case::Populated).bytes,
    )
}

#[test]
fn table_tabs_keep_independent_pages_selections_and_native_view_definitions() {
    let (_directory, path, original, mut harness) = opened(multiple_tables());
    let numbers = table(harness.state(), "Numbers");
    harness.state_mut().open_object(numbers.clone());
    finish_reads(&mut harness);
    let numbers_tab = tab(harness.state(), &numbers);
    let sort_target = harness.state().state.current_sort_target().unwrap();
    harness.state_mut().sort_view(
        sort_target,
        Some(SortSpec {
            column: "Id".into(),
            descending: true,
        }),
        false,
    );
    finish_reads(&mut harness);
    focus_cell(&mut harness, "Score", "1", "2147483647");
    let selection = harness.state().state.current_filter_selection().unwrap();
    harness
        .state_mut()
        .filter_selection(selection, FilterComparison::Equal);
    finish_reads(&mut harness);
    let numbers_filter = harness.state().state.active_filter_target().unwrap();
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0),
        Some(1)
    );
    assert_eq!(
        harness.state().state.selected_cell_text(),
        Some("2147483647")
    );

    let mixed = table(harness.state(), "Mixed Values");
    harness
        .get_by_role_and_label(Role::Button, "Mixed Values")
        .click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.active_object(),
        Some(numbers_tab.clone()),
        "one Navigation Pane click selects without opening"
    );
    harness.key_press(egui::Key::Enter);
    harness.run_steps(1);
    finish_reads(&mut harness);
    let mixed_tab = tab(harness.state(), &mixed);
    let sort_target = harness.state().state.current_sort_target().unwrap();
    harness.state_mut().sort_view(
        sort_target,
        Some(SortSpec {
            column: "Id".into(),
            descending: false,
        }),
        false,
    );
    finish_reads(&mut harness);
    let mixed_filter = harness.state().state.active_filter_target().unwrap();
    harness
        .state_mut()
        .filter_page(mixed_filter.clone(), 1, false);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 1);
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
    assert!(!harness.state().state.table_filter_enabled());

    harness.state_mut().activate_object(numbers_tab.clone());
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.active_object(),
        Some(numbers_tab.clone())
    );
    assert_eq!(
        harness.state().state.active_filter_target(),
        Some(numbers_filter)
    );
    assert!(harness.state().state.table_filter_enabled());
    assert_eq!(
        harness.state().state.table_sort(),
        Some(SortSpec {
            column: "Id".into(),
            descending: true,
        })
    );
    assert_eq!(
        harness.state().state.selected_cell_text(),
        Some("2147483647")
    );

    let count = harness.state().state.object_tabs().len();
    harness.state_mut().view_object_schema("Numbers".into());
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.active_object(),
        Some(numbers_tab.clone())
    );
    assert_eq!(harness.state().state.object_tabs().len(), count);
    assert_eq!(
        harness.state().state.schema.as_ref().unwrap().table,
        "Numbers"
    );
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.active_object(),
        Some(numbers_tab.clone())
    );
    assert_eq!(
        harness.state().state.selected_cell_text(),
        Some("2147483647")
    );

    harness.state_mut().activate_object(mixed_tab.clone());
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 1);
    assert_eq!(
        harness.state().state.active_filter_target(),
        Some(mixed_filter.clone())
    );
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
    harness.state_mut().close_object(numbers_tab);
    harness.run_steps(2);
    assert_eq!(harness.state().state.active_object(), Some(mixed_tab));
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.target == numbers)
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 1);
    assert!(harness.state().filter_handle(&mixed_filter).is_some());
    harness.state_mut().filter_page(mixed_filter, 0, false);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().table,
        "Mixed Values"
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 0);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn old_object_actions_cannot_replace_a_new_source_with_the_same_table_names() {
    let (_directory, path, original, mut harness) = opened(multiple_tables());
    let old_target = table(harness.state(), "Mixed Values");
    harness.state_mut().open_object(old_target.clone());
    finish_reads(&mut harness);
    let old_tab = tab(harness.state(), &old_target);

    harness.state_mut().open();
    let opening = harness.state().generation;
    harness.state_mut().activate_object(old_tab.clone());
    harness.state_mut().close_object(old_tab.clone());
    harness.state_mut().open_object(old_target.clone());
    assert_eq!(harness.state().generation, opening);
    finish_reads(&mut harness);

    let fresh_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_ne!(fresh_revision, old_target.document_revision);
    let active = harness.state().state.active_object();
    let generation = harness.state().generation;
    harness.state_mut().activate_object(old_tab.clone());
    harness.state_mut().close_object(old_tab);
    harness.state_mut().open_object(old_target);
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.active_object(), active);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .all(|tab| { tab.target.document_revision == fresh_revision })
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

fn fragment(object: i32, attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(object),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn query_source() -> Vec<u8> {
    database_with_query_flags(
        &[
            Fragment {
                name1: Some("First minimum"),
                flag: Some(4),
                ..fragment(QUERY_ID, 2)
            },
            Fragment {
                name1: Some("Items"),
                ..fragment(QUERY_ID, 5)
            },
            Fragment {
                expression: Some("Id"),
                ..fragment(QUERY_ID, 6)
            },
            Fragment {
                expression: Some("Id >= [First minimum]"),
                ..fragment(QUERY_ID, 8)
            },
            Fragment {
                name1: Some("Second maximum"),
                flag: Some(4),
                ..fragment(EMPTY_ID, 2)
            },
            Fragment {
                name1: Some("Items"),
                ..fragment(EMPTY_ID, 5)
            },
            Fragment {
                expression: Some("Id"),
                ..fragment(EMPTY_ID, 6)
            },
            Fragment {
                expression: Some("Id <= [Second maximum]"),
                ..fragment(EMPTY_ID, 8)
            },
        ],
        0,
    )
}

fn saved(desktop: &Desktop, name: &str) -> ObjectTarget {
    let database = desktop.state.database.as_ref().unwrap();
    let query = database
        .queries
        .iter()
        .find(|query| query.name == name)
        .unwrap();
    ObjectTarget {
        document_revision: database.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: query.object_id,
            name: query.name.clone(),
        },
    }
}

fn run_saved(harness: &mut Harness<'_, Desktop>, prompt: &str, input: &str) {
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, prompt)
        .type_text(input);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn saved_tabs_inspect_inertly_and_keep_separate_committed_parameters_and_results() {
    let (_directory, path, original, mut harness) = opened(query_source());
    let scratch = ObjectTarget {
        document_revision: harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        kind: ObjectKind::Sql,
    };
    harness.state_mut().open_object(scratch.clone());
    let scratch_tab = tab(harness.state(), &scratch);
    harness.state_mut().state.sql = "SELECT 99 AS Scratch".into();
    let first = saved(harness.state(), "Saved");
    harness.state_mut().open_object(first.clone());
    finish_reads(&mut harness);
    assert!(harness.state().state.selected_cell_text().is_none());
    run_saved(&mut harness, "First minimum", "2");
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
    let first_tab = tab(harness.state(), &first);

    let second = saved(harness.state(), "Empty");
    harness.state_mut().open_object(second.clone());
    finish_reads(&mut harness);
    assert!(harness.state().state.selected_cell_text().is_none());
    run_saved(&mut harness, "Second maximum", "1");
    assert_eq!(harness.state().state.selected_cell_text(), Some("1"));
    let second_tab = tab(harness.state(), &second);

    harness.state_mut().activate_object(first_tab);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "First minimum")
            .value()
            .as_deref(),
        Some("2")
    );
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));

    harness.state_mut().activate_object(second_tab);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some("1"));
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Scratch");
    assert!(harness.state().state.query_data.is_none());
    harness.state_mut().activate_object(scratch_tab);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Scratch");
    assert!(harness.state().state.query_data.is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn save_owner_vetoes_other_tabs_and_reopen_revokes_old_object_and_row_rights() {
    let (_directory, path, original, mut harness) = opened(multiple_tables());
    let numbers = table(harness.state(), "Numbers");
    harness.state_mut().open_object(numbers.clone());
    finish_reads(&mut harness);
    let numbers_tab = tab(harness.state(), &numbers);
    let mixed = table(harness.state(), "Mixed Values");
    harness.state_mut().open_object(mixed.clone());
    finish_reads(&mut harness);
    let mixed_tab = tab(harness.state(), &mixed);
    harness.state_mut().activate_object(numbers_tab.clone());
    finish_reads(&mut harness);
    let old_row = EditTarget {
        document_revision: numbers.document_revision,
        table: "Numbers".into(),
        column: "Score".into(),
        parent_ordinal: 1,
    };
    harness.state_mut().begin_edit(old_row.clone());
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("42");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    assert!(harness.state().editing.has_save());
    let generation = harness.state().generation;
    harness.state_mut().activate_object(mixed_tab.clone());
    harness.state_mut().close_object(mixed_tab.clone());
    assert_eq!(harness.state().generation, generation);
    assert_eq!(
        harness.state().state.active_object(),
        Some(numbers_tab.clone())
    );
    assert!(harness.state().state.object_tabs().contains(&mixed_tab));
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1))
    );
    assert!(harness.state().editing.has_save());

    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    let fresh = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_ne!(fresh, numbers.document_revision);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .all(|tab| tab.target.document_revision == fresh)
    );
    assert!(!harness.state().state.can_edit_cell(&old_row));
    let active = harness.state().state.active_object();
    let generation = harness.state().generation;
    harness.state_mut().activate_object(mixed_tab.clone());
    harness.state_mut().close_object(numbers_tab);
    harness.state_mut().open_object(mixed);
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.active_object(), active);
    let data = harness.state().state.data.as_ref().unwrap();
    let row = data.row_index(1).unwrap();
    assert_eq!(data.rows[row][1].text, "42");
    assert_ne!(std::fs::read(path).unwrap(), original);
}
