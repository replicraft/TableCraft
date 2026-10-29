//! Full variable records and TD growth retain original generation authority.
mod literal;

use super::{AutoNumberRowInsert, IncrementingAutoNumber, request::copy_value};
use crate::{
    EditError, IndexCheckStatus, InspectError, LongValueCheckStatus, PropertyGroupKind,
    RuleCheckStatus, WriteBlocker, check_table, check_table_constraints, check_table_long_values,
    inspect_table_properties, read_table,
    table::mutation::insert::tests::{
        long_value_inserts::{Family, initialization::model as long_rows},
        model as original,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{LimitError, Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn checked(bytes: &[u8], wanted: &[Vec<Value>]) {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), wanted.len());
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), wanted);
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    for field in constraints.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        );
    }
}

fn ordinary_requests(case: &literal::Ordinary) -> Vec<AutoNumberRowInsert> {
    case.added
        .iter()
        .map(|row| {
            let mut values = row.values(true);
            assert_eq!(values.remove(2), Value::Int32(row.id));
            AutoNumberRowInsert::new(values)
        })
        .collect()
}

#[test]
fn variable_companion_and_missing_lval_sources_and_full_images_qualify_before_preview() {
    for code in [5, 6] {
        let ordinary = literal::ordinary(code);
        let before = ordinary
            .case
            .rows
            .iter()
            .map(|row| row.values(true))
            .collect::<Vec<_>>();
        let after = ordinary
            .case
            .rows
            .iter()
            .chain(&ordinary.added)
            .map(|row| row.values(true))
            .collect::<Vec<_>>();
        checked(&ordinary.case.before, &before);
        checked(&ordinary.expected, &after);
        let _ = new_values_source(code);
        for family in [Family::Ole, Family::Memo, Family::CompressedMemo] {
            for chained in [false, true] {
                let case = literal::external(code, family, chained);
                checked(&case.before, &long_rows::rows(family, None));
                checked(&case.expected, &long_rows::rows(family, Some(&case.value)));
                assert_eq!(case.new_length, case.old_length + 10);
                long_rows::checked_maps(&case.expected);
                let report = check_table_long_values(
                    &mut Cursor::new(&case.before),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Absent);
                assert!(report.columns()[0].maps().is_none());
            }
        }
    }
}

#[test]
fn generated_nonfirst_id_preserves_text_binary_empty_and_null_companions() {
    for code in [5, 6] {
        let case = literal::ordinary(code);
        let requests = ordinary_requests(&case);
        let before = case
            .case
            .rows
            .iter()
            .map(|row| row.values(true))
            .collect::<Vec<_>>();
        let after = case
            .case
            .rows
            .iter()
            .chain(&case.added)
            .map(|row| row.values(true))
            .collect::<Vec<_>>();
        checked(&case.case.before, &before);
        checked(&case.expected, &after);
        let candidate = original::capture(&case.case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests,
                IncrementingAutoNumber::new(9),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.before_bytes(), case.case.before);
        assert_eq!(candidate.candidate_bytes(), case.expected);
        assert_eq!(candidate.generated_column_number(), 2);
        assert_eq!(candidate.generated_values(), &[10, 11]);
        assert_eq!(candidate.inserted_rows(), 2);
    }
}

#[test]
fn generated_counter_and_missing_declaration_growth_share_external_memo_ole_image() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo, Family::CompressedMemo] {
            for chained in [false, true] {
                let case = literal::external(code, family, chained);
                checked(&case.before, &long_rows::rows(family, None));
                checked(&case.expected, &long_rows::rows(family, Some(&case.value)));
                let requests = [AutoNumberRowInsert::new(vec![
                    Value::Int32(100),
                    case.value.clone(),
                ])];
                let candidate = original::capture(&case.before)
                    .preview_ace16_incrementing_insert_rows(
                        "Items",
                        &requests,
                        IncrementingAutoNumber::new(9),
                        &mut original::budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(candidate.candidate_bytes(), case.expected);
                assert_eq!(candidate.generated_values(), &[10]);
                assert_eq!(candidate.generated_column_number(), 0);
                assert_eq!(candidate.inserted_rows(), 1);
                long_rows::checked_maps(candidate.candidate_bytes());
            }
        }
    }
}

#[test]
fn companion_payload_copies_charge_exact_dynamic_memory_before_allocating() {
    for value in [Value::Text("Ā".repeat(128)), Value::Binary(vec![0x92; 510])] {
        let length = match &value {
            Value::Text(text) => text.len(),
            Value::Binary(bytes) => bytes.len(),
            _ => unreachable!(),
        } as u64;
        let mut healthy = ResourceBudget::new(length, 1, 10_000);
        assert_eq!(copy_value(&value, &mut healthy, &|| false).unwrap(), value);
        assert_eq!(healthy.remaining(Resource::Bytes), 0);
        let mut short = ResourceBudget::new(length - 1, 1, 10_000);
        assert_eq!(
            copy_value(&value, &mut short, &|| false).err(),
            Some(EditError::Inspect(InspectError::Limit(LimitError {
                resource: Resource::Bytes,
                remaining: length - 1
            })))
        );
        assert_eq!(short.remaining(Resource::Bytes), length - 1);
    }
}

fn new_values_source(code: u8) -> (Vec<u8>, Vec<AutoNumberRowInsert>) {
    let mut case = literal::ordinary(code);
    literal::new_values_property(&mut case.case.before);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.case.before),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.kind() == PropertyGroupKind::Column && group.name() == "ID")
        .unwrap();
    let property = group.get("NewValues").unwrap();
    assert_eq!(property.native_type(), 10);
    assert_eq!(property.text().unwrap(), "Random");
    let before = case
        .case
        .rows
        .iter()
        .map(|row| row.values(true))
        .collect::<Vec<_>>();
    checked(&case.case.before, &before);
    let requests = ordinary_requests(&case);
    (case.case.before, requests)
}

#[test]
fn stored_new_values_property_is_not_overridden_by_caller_increment_policy() {
    for code in [5, 6] {
        let (before, requests) = new_values_source(code);
        let result = original::capture(&before).preview_ace16_incrementing_insert_rows(
            "Items",
            &requests,
            IncrementingAutoNumber::new(9),
            &mut original::budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(EditError::Unsupported(WriteBlocker::Properties))
        );
    }
}
