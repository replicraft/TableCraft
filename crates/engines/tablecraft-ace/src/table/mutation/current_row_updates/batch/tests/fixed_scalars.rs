//! Six exact scalar descriptors share atomic placement, never inferred carriers.
use super::super::{Policy, proof};
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, NativeFixedScalarKind, NativeImage,
    WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, LegacyDateTime, Single, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fixed_scalars::{
    COLUMNS_TO_EDIT, Column, Scalar, batch as raw,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 4_000_000, 1_000_000_000)
}

fn kind(column: Column) -> NativeFixedScalarKind {
    match column {
        Column::ByteValue => NativeFixedScalarKind::Byte,
        Column::ShortValue => NativeFixedScalarKind::Int16,
        Column::LargeValue => NativeFixedScalarKind::Int64,
        Column::SingleValue => NativeFixedScalarKind::Single,
        Column::DoubleValue => NativeFixedScalarKind::Double,
        Column::DateValue => NativeFixedScalarKind::DateTime,
    }
}

fn value(value: Option<Scalar>) -> Value {
    match value {
        None => Value::Null,
        Some(Scalar::Byte(value)) => Value::Byte(value),
        Some(Scalar::Int16(value)) => Value::Int16(value),
        Some(Scalar::Int64(value)) => Value::Int64(value),
        Some(Scalar::SingleBits(value)) => Value::Single(Single::from_bits(value)),
        Some(Scalar::DoubleBits(value)) => Value::Double(Double::from_bits(value)),
        Some(Scalar::DateTimeBits(value)) => {
            Value::DateTime(LegacyDateTime::from_bits(value).unwrap())
        }
    }
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(4328),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(case: &raw::Source) -> Vec<FixedScalarUpdate> {
    case.cells
        .iter()
        .map(|cell| FixedScalarUpdate::new(cell.row, value(cell.expected), value(cell.value)))
        .collect()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

#[test]
fn six_kinds_match_independent_null_forwarded_empty_and_materialized_images() {
    for column in COLUMNS_TO_EDIT {
        for case in [
            raw::values(
                column,
                &[Some(raw::initial(column)), None],
                &[None, Some(raw::replacement(column))],
            ),
            raw::values(column, &[None, None], &[None, None]),
            raw::values(column, &[], &[]),
            raw::empty(column),
            raw::forwarded(column),
            raw::materializing(column),
        ] {
            let requests = requests(&case);
            let groups = [FittedColumnUpdates::Fixed {
                column: column.name(),
                updates: &requests,
            }];
            let original = image(&case.before);
            assert_eq!(
                original
                    .retained_fixed_scalar_column_kind(
                        case.table,
                        column.name(),
                        &mut budget(),
                        &|| false
                    )
                    .unwrap(),
                kind(column)
            );
            let plan = original
                .plan_retained_fixed_scalar_column_batch_updates(
                    case.table,
                    &groups,
                    kind(column),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(plan.before_bytes(), &case.before);
            same(plan.after_bytes(), &case.expected);
        }
    }
}

#[test]
fn bit_sensitive_extremes_keep_exact_originals_and_protected_companions() {
    for (column, first, second) in [
        (Column::ByteValue, Scalar::Byte(0), Scalar::Byte(255)),
        (
            Column::ShortValue,
            Scalar::Int16(i16::MIN),
            Scalar::Int16(i16::MAX),
        ),
        (
            Column::LargeValue,
            Scalar::Int64(i64::MIN),
            Scalar::Int64(i64::MAX),
        ),
        (
            Column::SingleValue,
            Scalar::SingleBits(0x8000_0000),
            Scalar::SingleBits(1),
        ),
        (
            Column::DoubleValue,
            Scalar::DoubleBits(0x8000_0000_0000_0000),
            Scalar::DoubleBits(1),
        ),
        (
            Column::DateValue,
            Scalar::DateTimeBits((-657_434_f64).to_bits()),
            Scalar::DateTimeBits(2_958_465.5_f64.to_bits()),
        ),
    ] {
        let case = raw::values(
            column,
            &[Some(first), Some(second)],
            &[Some(second), Some(first)],
        );
        let requests = requests(&case);
        let groups = [FittedColumnUpdates::Fixed {
            column: column.name(),
            updates: &requests,
        }];
        let plan = image(&case.before)
            .plan_retained_fixed_scalar_column_batch_updates(
                case.table,
                &groups,
                kind(column),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(plan.after_bytes(), &case.expected);
    }
}

#[test]
fn wrong_descriptor_duplicates_late_conflicts_and_singletons_grant_no_effect() {
    for column in COLUMNS_TO_EDIT {
        for case in [
            raw::values(column, &[None, None], &[None, None]),
            raw::empty(column),
        ] {
            let requests = requests(&case);
            let groups = [FittedColumnUpdates::Fixed {
                column: column.name(),
                updates: &requests,
            }];
            for other in COLUMNS_TO_EDIT {
                if kind(other) != kind(column) {
                    assert!(matches!(
                        image(&case.before).plan_retained_fixed_scalar_column_batch_updates(
                            case.table,
                            &groups,
                            kind(other),
                            &mut budget(),
                            &|| false
                        ),
                        Err(EditError::Unsupported(WriteBlocker::Column))
                    ));
                }
            }
        }
        let case = raw::values(
            column,
            &[Some(raw::initial(column)), None],
            &[None, Some(raw::replacement(column))],
        );
        let requests = requests(&case);
        for shape in 0..3 {
            let invalid = match shape {
                0 => vec![FixedScalarUpdate::new(
                    0,
                    value(Some(raw::initial(column))),
                    Value::Null,
                )],
                1 => vec![
                    FixedScalarUpdate::new(0, value(Some(raw::initial(column))), Value::Null),
                    FixedScalarUpdate::new(0, value(Some(raw::initial(column))), Value::Null),
                ],
                _ => vec![
                    FixedScalarUpdate::new(0, value(Some(raw::initial(column))), Value::Null),
                    FixedScalarUpdate::new(1, value(Some(raw::initial(column))), Value::Null),
                ],
            };
            let groups = [FittedColumnUpdates::Fixed {
                column: column.name(),
                updates: &invalid,
            }];
            assert!(
                image(&case.before)
                    .plan_retained_fixed_scalar_column_batch_updates(
                        case.table,
                        &groups,
                        kind(column),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
        let groups = [FittedColumnUpdates::Fixed {
            column: column.name(),
            updates: &requests,
        }];
        assert!(
            image(&case.before)
                .plan_retained_fixed_scalar_row_updates(case.table, &groups, &mut budget(), &|| {
                    false
                })
                .is_err()
        );
        assert!(
            image(&case.before)
                .plan_retained_long_column_batch_updates(
                    case.table,
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert!(
            image(&case.before)
                .plan_retained_currency_column_batch_updates(
                    case.table,
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert!(
            image(&case.before)
                .plan_retained_fixed_scalar_column_batch_updates(
                    case.table,
                    &groups,
                    kind(column),
                    &mut ResourceBudget::new(0, 0, 0),
                    &|| false
                )
                .is_err()
        );
        assert!(
            image(&case.before)
                .plan_retained_fixed_scalar_column_batch_updates(
                    case.table,
                    &groups,
                    kind(column),
                    &mut budget(),
                    &|| true
                )
                .is_err()
        );
    }
}

#[test]
fn fresh_independent_proof_rejects_protected_and_selected_after_tampering() {
    let case = raw::materializing(Column::LargeValue);
    let requests = requests(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column.name(),
        updates: &requests,
    }];
    let original = image(&case.before);
    proof::prove(
        &original,
        case.table,
        &groups,
        &case.expected,
        Policy::FixedScalar(kind(case.column)),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for offset in [
        3 * 4096 + 3000,
        case.definition as usize * 4096 + 3000,
        (case.definition as usize + 1) * 4096 + 4000,
    ] {
        let mut forged = case.expected.to_vec();
        forged[offset] ^= 1;
        assert!(
            proof::prove(
                &original,
                case.table,
                &groups,
                &forged,
                Policy::FixedScalar(kind(case.column)),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    same(original.as_bytes(), &case.before);
}
