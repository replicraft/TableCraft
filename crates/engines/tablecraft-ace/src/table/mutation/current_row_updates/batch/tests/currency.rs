//! Actual Currency originals use the closed atomic placement and independent proof.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, NativeFittedIntentGroup,
    NativeFittedUpdates, NativeImage, NativeUpdateProfile, WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    currency::batch as raw, fields, populated,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 2_000_000, 1_000_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(4327),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn value(value: Option<i64>) -> Value {
    value.map_or(Value::Null, |value| {
        Value::Currency(Currency::from_scaled(value))
    })
}

fn requests(case: &raw::Source) -> Vec<FixedScalarUpdate> {
    case.cells
        .iter()
        .map(|cell| FixedScalarUpdate::new(cell.row, value(cell.expected), value(cell.value)))
        .collect()
}

#[test]
fn currency_batches_match_full_raw_after_for_null_extrema_forwarded_growth_and_scale() {
    for case in [
        raw::values(&[Some(12345), None], &[Some(24845), Some(12500)]),
        raw::values(&[Some(12345), None], &[None, None]),
        raw::values(&[Some(12345), None], &[Some(12345), None]),
        raw::values(
            &[Some(i64::MIN), Some(i64::MAX)],
            &[Some(i64::MAX), Some(i64::MIN)],
        ),
        raw::empty(),
        raw::forwarded(),
        raw::materializing(),
        raw::all_selected(),
    ] {
        let groups = vec![
            NativeFittedIntentGroup::new_retained_currency_column_batch(
                case.column,
                NativeFittedUpdates::Fixed(requests(&case)),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        let plan = image(&case.before)
            .plan_recoverable_retained_currency_column_batch_updates(
                case.table,
                groups,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("Currency batch {} rows: {error:?}", case.cells.len()));
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16RetainedCurrencyColumnBatch
        );
        assert_eq!(
            plan.changed_rows(),
            case.cells
                .iter()
                .filter(|cell| cell.expected != cell.value)
                .count()
        );
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.expected);
        let fresh = image(&case.before);
        let rebuilt = plan
            .requalify_with_budgets(fresh, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(rebuilt.after_bytes(), case.expected);
    }
}

#[test]
fn exact_scalar_policy_and_cardinality_remain_separate_from_old_languages() {
    let case = raw::values(&[Some(1), None], &[Some(2), Some(3)]);
    let updates = requests(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &updates,
    }];
    assert!(matches!(
        image(&case.before).plan_retained_currency_row_updates(
            case.table,
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    assert!(
        image(&case.before)
            .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| false)
            .is_err()
    );

    let nulls = [
        FixedScalarUpdate::new(0, Value::Null, Value::Null),
        FixedScalarUpdate::new(1, Value::Null, Value::Null),
    ];
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &nulls,
    }];
    assert!(
        image(&case.before)
            .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| false)
            .is_err()
    );

    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numeric::source(&seed, "CurrencyValues", 4, &[None, None]).bytes;
    assert!(
        image(&before)
            .plan_retained_currency_column_batch_updates(
                case.table,
                &groups,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let empty = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &[],
    }];
    assert!(
        image(&before)
            .plan_retained_currency_column_batch_updates(case.table, &empty, &mut budget(), &|| {
                false
            })
            .is_err()
    );

    let singleton = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &updates[..1],
    }];
    assert!(
        image(&case.before)
            .plan_retained_currency_column_batch_updates(
                case.table,
                &singleton,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&case.before)
            .plan_retained_currency_row_updates(case.table, &singleton, &mut budget(), &|| false)
            .is_ok()
    );
}

#[test]
fn late_conflict_duplicate_and_wrong_carrier_refuse_the_whole_batch() {
    let case = raw::values(&[Some(1), None], &[Some(2), Some(3)]);
    for updates in [
        vec![
            FixedScalarUpdate::new(0, value(Some(1)), value(Some(2))),
            FixedScalarUpdate::new(1, value(Some(9)), value(Some(3))),
        ],
        vec![
            FixedScalarUpdate::new(0, value(Some(1)), value(Some(2))),
            FixedScalarUpdate::new(0, value(Some(1)), value(Some(3))),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Int64(1), value(Some(2))),
            FixedScalarUpdate::new(1, Value::Null, value(Some(3))),
        ],
    ] {
        let groups = [FittedColumnUpdates::Fixed {
            column: case.column,
            updates: &updates,
        }];
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
    }
}

#[test]
fn batch_work_and_terminal_cancellation_remain_caller_owned() {
    let case = raw::values(&[Some(1), None], &[Some(2), Some(3)]);
    let updates = requests(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &updates,
    }];
    let polls = Cell::new(0);
    image(&case.before)
        .plan_retained_currency_column_batch_updates(case.table, &groups, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let terminal = polls.get();
    let polls = Cell::new(0);
    assert!(
        image(&case.before)
            .plan_retained_currency_column_batch_updates(
                case.table,
                &groups,
                &mut budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == terminal
                }
            )
            .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, 2_000_000, 1_000_000_000),
        ResourceBudget::new(1_000_000_000, 0, 1_000_000_000),
        ResourceBudget::new(1_000_000_000, 2_000_000, 0),
    ] {
        assert!(
            image(&case.before)
                .plan_retained_currency_column_batch_updates(
                    case.table,
                    &groups,
                    &mut allowance,
                    &|| false
                )
                .is_err()
        );
    }
}
