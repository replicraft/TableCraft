//! Exact Currency carriers, ignored NULL bytes and retained sibling ownership.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    SlotScalarUpdate, WriteBlocker, check_table, read_table,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{currency as raw, fields, opaque_rows},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4319);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn value(value: Option<i64>) -> Value {
    value.map_or(Value::Null, |value| {
        Value::Currency(Currency::from_scaled(value))
    })
}

fn source() -> (Vec<u8>, u32, usize) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = raw::append_table(&seed, "MoneyTable");
    let first = raw::append(&empty, definition, raw::Input::Value(12345));
    (
        raw::append(&first, definition, raw::Input::Null),
        definition,
        catalog_root,
    )
}

fn plan(
    before: &[u8],
    row: usize,
    expected: Option<i64>,
    replacement: Option<i64>,
) -> Result<NativeEditPlan, EditError> {
    let requests = [FixedScalarUpdate::new(
        row,
        value(expected),
        value(replacement),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &requests,
    }];
    image(before)
        .plan_retained_currency_row_updates("MoneyTable", &groups, &mut budget(), &|| false)
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_currency_rows_match_scaled_extremes_null_noop_and_forwarded_images() {
    for forwarded in [false, true] {
        let (mut before, definition, catalog_root) = source();
        if forwarded {
            before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
            before = raw_forwarding_chain::extend(&before, 4, catalog_root, 20);
        }
        let native = image(&before);
        native
            .check_native_retained_currency_rows_table("MoneyTable", &mut budget(), &|| false)
            .unwrap();
        native
            .check_native_retained_cell_updates_table("MoneyTable", &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            native.check_native_retained_rows_table("MoneyTable", &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        let sibling = read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut values = [Some(12345), None];
        for (row, replacement) in [
            (0, Some(i64::MIN)),
            (0, Some(i64::MAX)),
            (0, None),
            (0, None),
            (1, Some(-1)),
            (1, Some(1)),
            (1, Some(12345)),
        ] {
            let expected = raw::update(&before, definition, row, replacement);
            let candidate = plan(&before, row, values[row], replacement).unwrap();
            assert_eq!(
                candidate.changed_rows(),
                usize::from(values[row] != replacement)
            );
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &expected);
            values[row] = replacement;
            let table = read_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "MoneyTable",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                table.rows(),
                &[
                    vec![Value::Int32(1), value(values[0])],
                    vec![Value::Int32(2), value(values[1])]
                ]
            );
            let other = read_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(other.rows(), sibling.rows());
            check_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "MoneyTable",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            before = expected;
        }
        let requests = [FixedScalarUpdate::new(0, Value::Null, value(Some(1)))];
        let groups = [FittedColumnUpdates::Fixed {
            column: "Amount",
            updates: &requests,
        }];
        assert!(matches!(
            image(&before)
                .plan_retained_row_updates("MoneyTable", &groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
}

#[test]
fn retained_currency_rows_reject_wrong_original_carriers_and_preserve_controls() {
    let (before, _, _) = source();
    assert!(matches!(
        plan(&before, 0, Some(12344), Some(1)),
        Err(EditError::Conflict)
    ));
    let wrong = [FixedScalarUpdate::new(
        0,
        Value::Int64(12345),
        value(Some(1)),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &wrong,
    }];
    assert!(matches!(
        image(&before).plan_retained_currency_row_updates(
            "MoneyTable",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    let requests = [FixedScalarUpdate::new(0, value(Some(12345)), Value::Null)];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &requests,
    }];
    assert!(matches!(
        image(&before).plan_retained_currency_row_updates(
            "MoneyTable",
            &groups,
            &mut budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&before).plan_retained_currency_row_updates(
                "MoneyTable",
                &groups,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let opaque = opaque_rows::source(opaque_rows::Case::VariableRetired);
    assert!(matches!(
        image(&opaque.bytes).check_native_retained_currency_rows_table(
            "Table1",
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Unsupported))
    ));
    let ordinary = raw::append(&raw::source(), 7, raw::Input::Omitted);
    let requests = [FixedScalarUpdate::new(0, value(Some(0)), value(Some(1)))];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &requests,
    }];
    assert!(matches!(
        image(&ordinary).plan_retained_currency_row_updates(
            "Table1",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn retained_currency_capability_keeps_long_and_text_actions_in_one_real_table() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let empty = raw::mixed::append_table(&seed, "Mixed");
    let mut before = raw::mixed::append(&empty, definition);
    let native = image(&before);
    native
        .check_native_retained_cell_updates_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    native
        .check_native_retained_currency_rows_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    native
        .check_native_retained_rows_table("Mixed", &mut budget(), &|| false)
        .unwrap();

    let requests = [FixedScalarUpdate::new(
        0,
        value(Some(0)),
        value(Some(12345)),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &requests,
    }];
    let expected = raw::mixed::update_currency(&before, definition, 0, Some(12345));
    let currency = image(&before)
        .plan_retained_currency_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(currency.after_bytes(), &expected);
    assert!(matches!(
        image(&before).plan_retained_row_updates("Mixed", &groups, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    before = expected;

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Int32(0),
        Value::Int32(123),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &requests,
    }];
    let expected = raw::mixed::update_number(&before, definition, 0, Some(123));
    let number = image(&before)
        .plan_retained_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(number.after_bytes(), &expected);
    assert!(matches!(
        image(&before)
            .plan_retained_currency_row_updates("Mixed", &groups, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    before = expected;

    let requests = [SlotScalarUpdate::new(
        0,
        Value::Null,
        Value::Text("日本語 🦀".into()),
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: "Label",
        updates: &requests,
    }];
    let expected = raw::mixed::update_text(&before, definition, 0, Some("日本語 🦀"));
    let text = image(&before)
        .plan_retained_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(text.after_bytes(), &expected);
    before = expected;

    let requests = [FixedScalarUpdate::new(0, value(Some(12345)), Value::Null)];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Amount",
        updates: &requests,
    }];
    let expected = raw::mixed::update_currency(&before, definition, 0, None);
    let currency = image(&before)
        .plan_retained_currency_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(currency.after_bytes(), &expected);
    let observed = read_table(
        &mut Cursor::new(currency.after_bytes()),
        SCOPE,
        "Mixed",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        observed.rows(),
        &[vec![
            Value::Int32(1),
            Value::Int32(123),
            Value::Null,
            Value::Text("日本語 🦀".into())
        ]]
    );
    image(currency.after_bytes())
        .check_native_retained_cell_updates_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    check_table(
        &mut Cursor::new(currency.after_bytes()),
        SCOPE,
        "Mixed",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}
