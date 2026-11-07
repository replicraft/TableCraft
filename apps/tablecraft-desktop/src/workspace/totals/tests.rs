//! Desktop Totals use complete native views, not the currently displayed page.
use crate::workspace::Desktop;
use crate::workspace::test_support::{choose_total, finish_reads, focus_cell, opened, show_totals};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_native17_creation, canonical_variable_creation,
    field_presentation, mutation_fixture,
};
use tablecraft_ui::{
    CellKind, EditTarget, FilterComparison, ObjectKind, ObjectTarget, SortSpec, TotalAggregate,
    TotalSpec, TotalsTarget,
};

fn target(harness: &Harness<'_, Desktop>) -> TotalsTarget {
    let state = &harness.state().state;
    TotalsTarget {
        document_revision: state.database.as_ref().unwrap().document_revision,
        table: state.data.as_ref().unwrap().table.clone(),
        filter_id: state.active_filter_target().map(|filter| filter.filter_id),
        enabled: state.table_filter_enabled(),
    }
}

#[test]
fn totals_cover_all_native_rows_and_track_current_filter_mode_without_changing_records() {
    let rows = (1..=205).map(|id| (id, id % 2)).collect::<Vec<_>>();
    let source = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(source);
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 100);
    show_totals(&mut harness);
    choose_total(&mut harness, "Id", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "21115"
    );
    assert_eq!(harness.state().state.selected_cell_text(), Some("21115"));
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 100);
    assert!(
        !harness.state().state.can_edit_cell(&EditTarget {
            document_revision: harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .document_revision,
            table: "Items".into(),
            column: "Id".into(),
            parent_ordinal: 0,
        })
    );

    for label in ["Paste", "Delete"] {
        assert!(
            harness
                .get_by_role_and_label(Role::Button, label)
                .accesskit_node()
                .is_disabled(),
            "{label} must not target the previously selected native record"
        );
    }
    let generation = harness.state().generation;
    harness.event(egui::Event::Paste("42".into()));
    harness.run_steps(2);
    assert_eq!(harness.state().generation, generation);
    assert!(!harness.state().state.edit_dialog_open());
    harness
        .get_by_role_and_label(Role::Button, "Copy")
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| {
                matches!(command, egui::OutputCommand::CopyText(value) if value == "21115")
            })
    );

    focus_cell(&mut harness, "Score", "2", "0");
    assert!(
        harness.state().state.can_edit_cell(&EditTarget {
            document_revision: harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .document_revision,
            table: "Items".into(),
            column: "Score".into(),
            parent_ordinal: 1,
        })
    );
    let selection = harness.state().state.current_filter_selection().unwrap();
    harness
        .state_mut()
        .filter_selection(selection, FilterComparison::Equal);
    finish_reads(&mut harness);
    let current = target(&harness);
    harness.state_mut().describe_totals(current);
    finish_reads(&mut harness);
    choose_total(&mut harness, "Id", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "10506"
    );
    let sort_target = harness.state().state.current_sort_target().unwrap();
    harness.state_mut().sort_view(
        sort_target,
        Some(SortSpec {
            column: "Id".into(),
            descending: true,
        }),
        true,
    );
    finish_reads(&mut harness);
    let current = target(&harness);
    harness.state_mut().describe_totals(current);
    finish_reads(&mut harness);
    choose_total(&mut harness, "Id", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "10506"
    );
    let filter = harness.state().state.active_filter_target().unwrap();
    harness.state_mut().filter_page(filter, 0, false);
    finish_reads(&mut harness);
    let current = target(&harness);
    harness.state_mut().describe_totals(current);
    finish_reads(&mut harness);
    choose_total(&mut harness, "Id", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Id").unwrap().text,
        "21115"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn totals_count_present_empty_text_and_empty_native_tables_without_record_targets() {
    let source = canonical_native17_creation::source(
        canonical_variable_creation::source(canonical_variable_creation::Case::NullAndEmpty).bytes,
    );
    let (_directory, path, original, mut harness) = opened(source);
    show_totals(&mut harness);
    choose_total(&mut harness, "TextValue", "Count");
    assert_eq!(
        harness.state().state.totals_cell("TextValue").unwrap().text,
        "2"
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 4);
    assert_eq!(std::fs::read(path).unwrap(), original);

    let source = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let (_directory, path, original, mut harness) = opened(source);
    show_totals(&mut harness);
    choose_total(&mut harness, "Id", "Count");
    assert_eq!(harness.state().state.totals_cell("Id").unwrap().text, "0");
    choose_total(&mut harness, "Score", "Sum");
    assert!(matches!(
        harness.state().state.totals_cell("Score").unwrap().kind,
        CellKind::Scalar {
            is_null: true,
            stored_calculation: false,
            ..
        }
    ));
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert!(
        !harness.state().state.can_edit_cell(&EditTarget {
            document_revision: harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .document_revision,
            table: "Items".into(),
            column: "Id".into(),
            parent_ordinal: 0,
        })
    );
    assert!(harness.state().state.current_delete_target().is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn totals_currency_is_exact_and_old_source_or_inactive_object_targets_cannot_dispatch() {
    let source = field_presentation::score64(&[], 5, &[12_345, 12_346]);
    let (_directory, path, original, mut harness) = opened(source);
    show_totals(&mut harness);
    choose_total(&mut harness, "Score", "Sum");
    assert!(matches!(
        harness
            .state()
            .state
            .totals_cell("Score")
            .unwrap()
            .presentation,
        Some(tablecraft_display::ScalarPresentation::Currency { scaled: 24_691, .. })
    ));
    let stale = target(&harness);
    let scratch = ObjectTarget {
        document_revision: stale.document_revision,
        kind: ObjectKind::Sql,
    };
    harness.state_mut().open_object(scratch);
    let generation = harness.state().generation;
    harness.state_mut().describe_totals(stale.clone());
    assert_eq!(harness.state().generation, generation);
    harness.state_mut().open();
    finish_reads(&mut harness);
    let generation = harness.state().generation;
    harness.state_mut().describe_totals(stale.clone());
    harness.state_mut().compute_totals(
        stale,
        vec![TotalSpec {
            column: "Score".into(),
            aggregate: TotalAggregate::Sum,
        }],
    );
    assert_eq!(harness.state().generation, generation);
    assert!(!harness.state().state.busy);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[test]
fn reviewed_edit_reopens_totals_preferences_and_recomputes_on_the_fresh_source() {
    use crate::workspace::test_support::finish_save;
    use std::time::{Duration, Instant};
    use tablecraft_runtime::SaveOutcome;

    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    show_totals(&mut harness);
    choose_total(&mut harness, "Score", "Sum");
    assert_eq!(
        harness.state().state.totals_cell("Score").unwrap().text,
        "60"
    );
    assert!(harness.state().state.total_selection_active());
    let stale_totals = target(&harness);
    let stale_edit = EditTarget {
        document_revision: stale_totals.document_revision,
        table: stale_totals.table.clone(),
        column: "Score".into(),
        parent_ordinal: 1,
    };
    assert!(!harness.state().state.can_edit_cell(&stale_edit));
    focus_cell(&mut harness, "Score", "2", "20");
    assert!(harness.state().state.can_edit_cell(&stale_edit));
    harness.state_mut().begin_edit(stale_edit.clone());
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("25");
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
    let report = finish_save(&mut harness);
    let SaveOutcome::Saved { receipt, .. } = report.outcome else {
        panic!("the reviewed change must actually commit");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        stale_totals.document_revision
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    // Reopening and metadata/results use separate worker requests. Let the
    // retained aggregate preference finish its fresh service publication.
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.totals_cell("Score").is_none() {
        assert!(
            Instant::now() < deadline,
            "fresh native totals did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(!harness.state().state.busy);
    assert_ne!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        stale_totals.document_revision
    );
    assert_eq!(
        harness.state().state.totals_cell("Score").unwrap().text,
        "65"
    );
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[1][1].text,
        "25"
    );
    assert!(!harness.state().state.can_edit_cell(&stale_edit));
    let generation = harness.state().generation;
    harness.state_mut().describe_totals(stale_totals);
    assert_eq!(harness.state().generation, generation);
    assert_ne!(std::fs::read(path).unwrap(), original);
}
