//! Genuine ordinary CREATE and indexed UPDATE outputs feed the manual INSERT source.
use crate::{
    FittedColumnUpdates, FixedScalarUpdate, InsertFieldValue, NativeImage, OmittedRowInsert,
    SecondaryIndexCreate,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::inserting as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 1_000_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(41110),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing full image byte"
    );
}
fn rows(values: &[Option<i32>]) -> Vec<OmittedRowInsert> {
    values
        .iter()
        .enumerate()
        .map(|(position, value)| {
            OmittedRowInsert::new(vec![if position == 0 && value.is_none() {
                InsertFieldValue::Omitted
            } else {
                InsertFieldValue::Supplied(value.map_or(Value::Null, Value::Int32))
            }])
        })
        .collect()
}

#[test]
fn real_create_update_then_manual_insert_keeps_direction_row_identity_and_historical_counts() {
    for descending in [false, true] {
        let case = raw::source(descending);
        let creation = &case.updated.creation;
        let created = image(&creation.before)
            .prepare_retained_long_index_creation(
                creation.table,
                &SecondaryIndexCreate::new(creation.index_name, creation.column, descending),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(created.candidate_bytes(), &creation.expected);
        let changes = case
            .updated
            .cells
            .iter()
            .map(|cell| {
                FixedScalarUpdate::new(
                    cell.row,
                    cell.expected.map_or(Value::Null, Value::Int32),
                    cell.value.map_or(Value::Null, Value::Int32),
                )
            })
            .collect::<Vec<_>>();
        let groups = [FittedColumnUpdates::Fixed {
            column: creation.column,
            updates: &changes,
        }];
        let updated = image(created.candidate_bytes())
            .plan_retained_indexed_long_updates(creation.table, &groups, &mut budget(), &|| false)
            .unwrap();
        same(updated.after_bytes(), &case.updated.expected);
        let source = image(updated.after_bytes());
        assert!(
            source
                .select_native_retained_manual_indexed_insert_table(
                    creation.table,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
        );
        source
            .check_native_retained_manual_indexed_insert_table(
                creation.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let plan = source
            .plan_retained_manual_indexed_omitted_row_inserts(
                creation.table,
                &rows(&case.values),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(plan.before_bytes(), &case.updated.expected);
        same(plan.after_bytes(), &case.expected);
        assert_eq!(plan.changed_rows(), 4);
        let counter = creation.target_definition as usize * 4096 + 67;
        assert_eq!(
            &case.updated.expected[counter..counter + 4],
            &4_u32.to_le_bytes()
        );
        assert_eq!(&case.expected[counter..counter + 4], &6_u32.to_le_bytes());
    }
}

#[test]
fn preexisting_unique_and_required_insert_constraints_are_checked_before_effects() {
    use crate::{EditError, IndexConstraintViolation, InspectError};
    for descending in [false, true] {
        for required in [false, true] {
            let case = raw::unique(descending, required);
            let table = case.updated.creation.table;
            let plan = image(&case.updated.expected)
                .plan_retained_manual_indexed_omitted_row_inserts(
                    table,
                    &rows(&case.values),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(plan.after_bytes(), &case.expected);
            let duplicate = [
                OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Int32(7))]),
                OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Int32(2))]),
            ];
            let refused = image(&case.updated.expected)
                .plan_retained_manual_indexed_omitted_row_inserts(
                    table,
                    &duplicate,
                    &mut budget(),
                    &|| false,
                );
            assert!(matches!(
                refused,
                Err(EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                )))
            ));
            if required {
                for value in [
                    InsertFieldValue::Omitted,
                    InsertFieldValue::Supplied(Value::Null),
                ] {
                    let null = [OmittedRowInsert::new(vec![value])];
                    assert!(
                        image(&case.updated.expected)
                            .plan_retained_manual_indexed_omitted_row_inserts(
                                table,
                                &null,
                                &mut budget(),
                                &|| false
                            )
                            .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn empty_requests_still_check_complete_source_and_resources_without_legacy_fallback() {
    let case = raw::source(false);
    let table = case.updated.creation.table;
    let plan = image(&case.updated.expected)
        .plan_retained_manual_indexed_omitted_row_inserts(table, &[], &mut budget(), &|| false)
        .unwrap();
    same(plan.after_bytes(), &case.updated.expected);
    assert_eq!(plan.changed_rows(), 0);
    for requests in [Vec::new(), rows(&case.values)] {
        assert!(
            image(&case.updated.expected)
                .plan_retained_manual_indexed_omitted_row_inserts(
                    table,
                    &requests,
                    &mut budget(),
                    &|| true
                )
                .is_err()
        );
        assert!(
            image(&case.updated.expected)
                .plan_retained_manual_indexed_omitted_row_inserts(
                    table,
                    &requests,
                    &mut ResourceBudget::new(0, 0, 0),
                    &|| false
                )
                .is_err()
        );
        let mut malformed = case.updated.expected.to_vec();
        malformed[case.updated.creation.root as usize * 4096 + 4] ^= 1;
        assert!(
            image(&malformed)
                .plan_retained_manual_indexed_omitted_row_inserts(
                    table,
                    &requests,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    assert!(
        image(&case.updated.creation.before)
            .plan_retained_manual_indexed_omitted_row_inserts(
                table,
                &rows(&case.values),
                &mut budget(),
                &|| false
            )
            .is_err(),
        "no index is a different source family"
    );
    assert!(
        image(&case.updated.expected)
            .plan_retained_keyless_omitted_row_inserts(
                table,
                &rows(&case.values),
                &mut budget(),
                &|| false
            )
            .is_err(),
        "old keyless10 remains closed"
    );
}

#[test]
fn compressed_preexisting_guard_survives_create_then_update_and_null_insert_with_typed_companions()
{
    for descending in [false, true] {
        let case = raw::compressed::source(descending);
        let creation = &case.creation;
        let created = image(&creation.before)
            .prepare_retained_long_index_creation(
                creation.table,
                &SecondaryIndexCreate::new(creation.index_name, creation.column, descending),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(created.candidate_bytes(), &creation.expected);
        let changes = case
            .changes
            .iter()
            .map(|cell| FixedScalarUpdate::new(cell.row, Value::Null, Value::Int32(5)))
            .collect::<Vec<_>>();
        let groups = [FittedColumnUpdates::Fixed {
            column: "Guard",
            updates: &changes,
        }];
        let updated = image(created.candidate_bytes())
            .plan_retained_indexed_long_updates(creation.table, &groups, &mut budget(), &|| false)
            .unwrap();
        same(updated.after_bytes(), &case.updated);
        let prefix = case.guard_root as usize * 4096 + 24;
        assert_eq!(
            &updated.after_bytes()[prefix..prefix + 2],
            &3_u16.to_le_bytes()
        );
        let request = [OmittedRowInsert::new(
            raw::compressed::values()
                .into_iter()
                .map(InsertFieldValue::Supplied)
                .collect(),
        )];
        let inserted = image(updated.after_bytes())
            .plan_retained_manual_indexed_omitted_row_inserts(
                creation.table,
                &request,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(inserted.after_bytes(), &case.expected);
        assert_eq!(
            &inserted.after_bytes()[prefix..prefix + 2],
            &0_u16.to_le_bytes()
        );
        assert_eq!(inserted.changed_rows(), 1);
    }
}
