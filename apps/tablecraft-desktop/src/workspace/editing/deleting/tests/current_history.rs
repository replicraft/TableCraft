//! Actual DELETE and reinsert cycles preserve current metadata and generated history.
use crate::workspace::Desktop;
use crate::workspace::editing::test_support::{review_row_delete, save_reviewed_row};
use crate::workspace::test_support::inserting::{
    begin_generated_insert, choose_field_mode, review_and_save_insert,
};
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_core::values::Value;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    current_delete as raw,
    current_insert::{self, Input},
    deleted_field::populated::{Case, readdition::Target},
};
use tablecraft_ui::CellKind;

fn assert_page(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    rows: &[Vec<(String, CellKind)>],
    offset: usize,
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.offset as usize, offset);
    assert!(
        !page.has_more,
        "Each independent source fits the displayed suffix"
    );
    assert_eq!(page.columns.len(), columns.len());
    for (field, (name, number)) in page.columns.iter().zip(columns) {
        assert_eq!(&field.name, name);
        assert_eq!(&field.native_number, number);
    }
    assert_eq!(page.rows.len(), rows.len());
    for (row, original) in page.rows.iter().zip(rows) {
        assert_eq!(row.len(), original.len());
        for (cell, (text, kind)) in row.iter().zip(original) {
            assert_eq!(&cell.text, text);
            assert!(
                cell.kind == *kind,
                "Row deletion changed a surviving cell kind"
            );
        }
    }
}

fn journey(case: Case, target: Target) {
    let raw::Source {
        bytes,
        mut roots,
        next_id,
        ..
    } = raw::source(case, target, true, true, true);
    let (_directory, path, mut previous, mut harness) = opened(bytes);
    let page = harness.state().state.data.as_ref().unwrap();
    let columns = page
        .columns
        .iter()
        .map(|field| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let prototypes = page.rows[0]
        .iter()
        .map(|cell| cell.kind)
        .collect::<Vec<_>>();
    let mut rows = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), roots.len());
    // Remove an old forwarded root, the new TCNA7 root, and then the remaining
    // direct roots. Physical slots stay distinct from fresh UI row ordinals.
    let mut order = vec![roots[0], *roots.last().unwrap()];
    order.extend_from_slice(&roots[1..roots.len() - 1]);
    for root in order {
        let selected = roots
            .iter()
            .position(|candidate| *candidate == root)
            .unwrap();
        let expected = raw::after(&previous, &[root]);
        let old_tab = harness.state().state.active_object().unwrap();
        review_row_delete(&mut harness, selected);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            previous,
            "DELETE review is inert"
        );
        assert!(!harness.state().editing.has_save());
        assert!(!journal_path(&path).exists());
        save_reviewed_row(&mut harness, "Delete reviewed row");
        let actual = std::fs::read(&path).unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(
            actual.iter().zip(&expected).position(|(a, b)| a != b),
            None,
            "First DELETE difference for {case:?} {target:?} root={root}"
        );
        assert_eq!(actual, expected);
        assert_eq!(
            &expected[10 * 4096..11 * 4096],
            &previous[10 * 4096..11 * 4096],
            "DELETE must retain every MR2 history snapshot"
        );
        roots.remove(selected);
        rows.remove(selected);
        let refresh = selected.min(rows.len().saturating_sub(1));
        assert_page(&harness, &columns, &rows[refresh..], refresh);
        if refresh != 0 {
            // DELETE retains the adjacent current record, so its acknowledged
            // page can be a suffix. Navigate normally before checking all rows.
            let first = harness.get_by_role_and_label(Role::Button, "First");
            assert!(!first.accesskit_node().is_disabled());
            first.click_accesskit();
            harness.run_steps(1);
            finish_reads(&mut harness);
        }
        assert_page(&harness, &columns, &rows, 0);
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.target.kind, old_tab.target.kind);
        assert!(fresh.target.document_revision > old_tab.target.document_revision);
        assert!(!harness.state_mut().state.activate_object(&old_tab));
        assert!(!journal_path(&path).exists());
        previous = expected;
    }
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert!(harness.state().state.current_delete_target().is_none());

    let expected = current_insert::append(&previous, case, target, Input::Omitted);
    begin_generated_insert(&mut harness);
    for (name, _) in &columns[1..] {
        choose_field_mode(&mut harness, name, "Omitted");
    }
    review_and_save_insert(&mut harness, &path, &previous);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let row = current_insert::row(case, target, Input::Omitted, next_id)
        .into_iter()
        .zip(prototypes)
        .map(|(value, mut kind)| {
            let (text, null) = match value {
                Value::Null => ("NULL".into(), true),
                Value::Int32(value) => (value.to_string(), false),
                _ => panic!("Omitted declarations here produce only LONG or NULL"),
            };
            let CellKind::Scalar { is_null, .. } = &mut kind else {
                panic!("Current declaration fields are scalar");
            };
            *is_null = null;
            (text, kind)
        })
        .collect::<Vec<_>>();
    rows.push(row);
    assert_page(&harness, &columns, &rows, 0);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][0].text,
        next_id.to_string(),
        "Deleting all rows must not reset LAST"
    );
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_page(&reopened, &columns, &rows, 0);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

fn cancelled(target: Target) {
    let source = raw::source(Case::MixedBinary, target, true, true, true);
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let original_tab = harness.state().state.active_object().unwrap();
    let count = harness.state().state.data.as_ref().unwrap().rows.len();
    review_row_delete(&mut harness, 0);
    let review_id = harness.state().editing.delete_review.as_ref().unwrap().0;
    assert_eq!(std::fs::read(&path).unwrap(), before);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_delete(review_id);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.delete_review.is_none());
    assert_eq!(harness.state().state.active_object().unwrap(), original_tab);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows.len(),
        count
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
}

#[test]
fn number_present_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::NumberPresent, Target::Number);
}

#[test]
fn number_present_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::NumberPresent, Target::ShortText);
}

#[test]
fn number_absent_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::NumberAbsent, Target::Number);
}

#[test]
fn number_absent_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::NumberAbsent, Target::ShortText);
}

#[test]
fn text_present_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::TextPresent, Target::Number);
}

#[test]
fn text_present_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::TextPresent, Target::ShortText);
}

#[test]
fn text_absent_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::TextAbsent, Target::Number);
}

#[test]
fn text_absent_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::TextAbsent, Target::ShortText);
}

#[test]
fn mixed_text_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::MixedText, Target::Number);
}

#[test]
fn mixed_text_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::MixedText, Target::ShortText);
}

#[test]
fn mixed_binary_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::MixedBinary, Target::Number);
}

#[test]
fn mixed_binary_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::MixedBinary, Target::ShortText);
}

#[test]
fn id_only_text_current_number_deletes_to_empty_then_resumes_generation() {
    journey(Case::IdOnlyText, Target::Number);
}

#[test]
fn id_only_text_current_text_deletes_to_empty_then_resumes_generation() {
    journey(Case::IdOnlyText, Target::ShortText);
}

#[test]
fn cancelled_current_number_delete_cannot_replay_the_stale_review() {
    cancelled(Target::Number);
}

#[test]
fn cancelled_current_text_delete_cannot_replay_the_stale_review() {
    cancelled(Target::ShortText);
}
