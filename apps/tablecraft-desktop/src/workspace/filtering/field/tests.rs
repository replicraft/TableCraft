//! Real common-filter requests over the same native source and save owner.
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_native17_creation, canonical_variable_creation,
    field_presentation, mutation_fixture,
};
use tablecraft_ui::{FieldFilterOperator, FieldFilterTarget, ObjectKind, ObjectTarget, SortSpec};

fn target(harness: &Harness<'_, Desktop>, column: &str) -> FieldFilterTarget {
    let state = &harness.state().state;
    FieldFilterTarget {
        document_revision: state.database.as_ref().unwrap().document_revision,
        table: state.data.as_ref().unwrap().table.clone(),
        column: column.into(),
        filter_id: state.active_filter_target().map(|filter| filter.filter_id),
        enabled: state.table_filter_enabled(),
    }
}

fn describe(harness: &mut Harness<'_, Desktop>, column: &str) -> FieldFilterTarget {
    let target = target(harness, column);
    harness.state_mut().describe_field_filter(target.clone());
    finish_reads(harness);
    target
}

fn set_input(harness: &mut Harness<'_, Desktop>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::MultilineTextInput, label)
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, label)
        .type_text(text);
    harness.run_steps(2);
}

fn choose(harness: &mut Harness<'_, Desktop>, label: &str) {
    harness
        .get_by_role_and_label(Role::ComboBox, "Comparison")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, label)
        .click_accesskit();
    harness.run_steps(2);
}

fn apply(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Apply filter")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn common_numeric_bounds_replace_the_same_field_and_retain_native_sort_and_failed_draft() {
    let rows = (1..=205).map(|id| (id, id % 2)).collect::<Vec<_>>();
    let original = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(original);
    let sort = SortSpec {
        column: "Id".into(),
        descending: true,
    };
    let sort_target = harness.state().state.current_sort_target().unwrap();
    harness
        .state_mut()
        .sort_view(sort_target, Some(sort.clone()), false);
    finish_reads(&mut harness);

    describe(&mut harness, "Id");
    choose(&mut harness, "Between");
    set_input(&mut harness, "Value", "105");
    set_input(&mut harness, "Second value", "100");
    apply(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 6);
    assert_eq!(data.parent_ordinal(0), Some(104));
    assert_eq!(data.parent_ordinal(5), Some(99));
    assert_eq!(harness.state().state.table_sort(), Some(sort.clone()));
    assert_eq!(harness.state().state.table_filter_count(), 1);

    let filter = harness.state().state.active_filter_target().unwrap();
    harness.state_mut().filter_page(filter, 0, false);
    finish_reads(&mut harness);
    assert!(!harness.state().state.table_filter_enabled());
    describe(&mut harness, "Id");
    choose(&mut harness, "Equals");
    set_input(&mut harness, "Value", "2");
    apply(&mut harness);
    assert_eq!(harness.state().state.table_filter_count(), 1);
    assert!(harness.state().state.table_filter_enabled());
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
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

    let retained = harness.state().filter.as_ref().unwrap().target().clone();
    let retained_rows = harness.state().state.data.as_ref().unwrap().rows.as_ptr();
    describe(&mut harness, "Id");
    set_input(&mut harness, "Value", "not a number");
    harness
        .get_by_role_and_label(Role::Button, "Apply filter")
        .click_accesskit();
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(Instant::now() < deadline, "field filter did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    assert_eq!(harness.state().filter.as_ref().unwrap().target(), &retained);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows.as_ptr(),
        retained_rows
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Value")
            .value()
            .as_deref(),
        Some("not a number")
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn common_field_metadata_works_without_rows_and_stale_source_targets_cannot_dispatch() {
    let original = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let (_directory, path, original, mut harness) = opened(original);
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    let stale = describe(&mut harness, "Score");
    let request_id = harness.state().generation;
    harness.state_mut().close_field_filter(request_id);
    harness.run_steps(2);
    let scratch = ObjectTarget {
        document_revision: stale.document_revision,
        kind: ObjectKind::Sql,
    };
    harness.state_mut().open_object(scratch);
    let generation = harness.state().generation;
    harness.state_mut().describe_field_filter(stale.clone());
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().state.field_filter_request_id().is_none());
    harness.state_mut().open();
    finish_reads(&mut harness);
    let generation = harness.state().generation;
    harness.state_mut().describe_field_filter(stale.clone());
    harness
        .state_mut()
        .apply_field_filter(stale, FieldFilterOperator::Equal, vec!["10".into()]);
    assert_eq!(harness.state().generation, generation);
    assert!(!harness.state().state.busy);
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn common_text_patterns_preserve_null_vs_empty_and_native_wildcard_escaping() {
    let original = canonical_native17_creation::source(
        canonical_variable_creation::source(canonical_variable_creation::Case::NullAndEmpty).bytes,
    );
    let (_directory, path, original, mut harness) = opened(original);
    describe(&mut harness, "TextValue");
    choose(&mut harness, "Equals");
    set_input(&mut harness, "Value", "*");
    apply(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.parent_ordinal(0), Some(1));
    assert_eq!(data.parent_ordinal(1), Some(3));

    describe(&mut harness, "TextValue");
    choose(&mut harness, "Does not equal");
    set_input(&mut harness, "Value", "*");
    apply(&mut harness);
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    describe(&mut harness, "TextValue");
    choose(&mut harness, "Equals");
    set_input(&mut harness, "Value", "[*]");
    apply(&mut harness);
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    describe(&mut harness, "TextValue");
    choose(&mut harness, "Is Null");
    apply(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.parent_ordinal(0), Some(0));
    assert_eq!(data.parent_ordinal(1), Some(2));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn common_date_and_currency_bounds_use_native_values_instead_of_display_strings() {
    for (native_type, values, first, second) in [
        (
            8,
            vec![2.0_f64.to_bits(), 3.0_f64.to_bits(), 4.0_f64.to_bits()],
            "1900-01-03T00:00:00",
            "1900-01-01T00:00:00",
        ),
        (5, vec![12_344_u64, 12_345, 12_346], "1.2345", "1.2345"),
    ] {
        let original = field_presentation::score64(&[], native_type, &values);
        let (_directory, path, original, mut harness) = opened(original);
        describe(&mut harness, "Score");
        choose(&mut harness, "Between");
        set_input(&mut harness, "Value", first);
        set_input(&mut harness, "Second value", second);
        apply(&mut harness);
        let data = harness.state().state.data.as_ref().unwrap();
        if native_type == 8 {
            assert_eq!(data.rows.len(), 3);
            assert_eq!(data.parent_ordinal(0), Some(0));
            assert_eq!(data.parent_ordinal(2), Some(2));
        } else {
            assert_eq!(data.rows.len(), 1);
            assert_eq!(data.parent_ordinal(0), Some(1));
        }
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn reviewed_edit_reopens_and_reapplies_the_common_field_criterion() {
    use crate::workspace::test_support::finish_save;
    use tablecraft_runtime::SaveOutcome;
    use tablecraft_ui::EditTarget;

    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    describe(&mut harness, "Score");
    choose(&mut harness, "Greater than or equal to");
    set_input(&mut harness, "Value", "20");
    apply(&mut harness);
    let old = harness.state().state.active_filter_target().unwrap();
    assert_eq!(harness.state().filter.as_ref().unwrap().matched_rows(), 2);
    harness.state_mut().begin_edit(EditTarget {
        document_revision: old.document_revision,
        table: old.table.clone(),
        column: "Score".into(),
        parent_ordinal: 1,
    });
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("19");
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
    assert!(matches!(
        finish_save(&mut harness).outcome,
        SaveOutcome::Saved { .. }
    ));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let current = harness.state().state.active_filter_target().unwrap();
    assert_ne!(current.document_revision, old.document_revision);
    assert_ne!(current.filter_id, old.filter_id);
    assert_eq!(harness.state().state.table_filter_count(), 1);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.parent_ordinal(0), Some(2));
    assert_eq!(data.rows[0][1].text, "30");
    assert_ne!(std::fs::read(path).unwrap(), original);
}
