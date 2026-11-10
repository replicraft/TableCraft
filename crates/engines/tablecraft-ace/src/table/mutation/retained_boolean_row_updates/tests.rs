//! Existing Boolean bits retain complete original records and closed actions.
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
    canonical_retained_add_table::{boolean as raw, fields},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4320);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn source() -> (Vec<u8>, u32, usize) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = raw::append_table(&seed, "BooleanValues");
    let first = raw::append(&empty, definition, raw::Input::Omitted);
    let second = raw::append(&first, definition, raw::Input::Value(true));
    (second, definition, catalog_root)
}

fn plan(
    before: &[u8],
    row: usize,
    expected: bool,
    replacement: bool,
) -> Result<NativeEditPlan, EditError> {
    let requests = [FixedScalarUpdate::new(
        row,
        Value::Boolean(expected),
        Value::Boolean(replacement),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    image(before).plan_retained_boolean_row_updates(
        "BooleanValues",
        &groups,
        &mut budget(),
        &|| false,
    )
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

fn rows(bytes: &[u8], table: &str) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_rows()
}

#[test]
fn retained_boolean_rows_change_only_the_existing_value_bit_through_long_chains() {
    for forwarded in [false, true] {
        let (mut before, definition, catalog_root) = source();
        if forwarded {
            before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
            before = raw_forwarding_chain::extend(&before, 4, catalog_root, 20);
        }
        let native = image(&before);
        native
            .check_native_retained_boolean_rows_table("BooleanValues", &mut budget(), &|| false)
            .unwrap();
        native
            .check_native_retained_cell_updates_table("BooleanValues", &mut budget(), &|| false)
            .unwrap();
        let sibling = rows(&before, "Table1");
        let mut values = [false, true];
        for (row, replacement) in [(0, true), (0, true), (1, false), (0, false), (1, true)] {
            let expected = raw::update(&before, definition, row, replacement);
            let candidate = plan(&before, row, values[row], replacement).unwrap();
            assert_eq!(
                candidate.changed_rows(),
                usize::from(values[row] != replacement)
            );
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &expected);
            let differences = before
                .iter()
                .zip(candidate.after_bytes())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(differences, usize::from(values[row] != replacement));
            values[row] = replacement;
            assert_eq!(
                rows(candidate.after_bytes(), "BooleanValues"),
                vec![
                    vec![Value::Int32(1), Value::Boolean(values[0])],
                    vec![Value::Int32(2), Value::Boolean(values[1])]
                ]
            );
            assert_eq!(rows(candidate.after_bytes(), "Table1"), sibling);
            check_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "BooleanValues",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            before = expected;
        }
    }
}

#[test]
fn boolean_offsets_own_no_payload_in_ordinary_retained_and_historical_rows() {
    let ordinary = raw::append(&raw::source(), 7, raw::Input::Value(true));
    let (retained, definition, _) = source();
    for (original, definition, table) in [
        (ordinary, 7, "Table1"),
        (retained, definition, "BooleanValues"),
    ] {
        let expected = rows(&original, table);
        let mut ignored = original;
        let offset = definition as usize * 4096 + 121;
        ignored[offset..offset + 2].copy_from_slice(&65535_u16.to_le_bytes());
        assert_eq!(rows(&ignored, table), expected);
        let mut bad_id = ignored.to_vec();
        let id_offset = definition as usize * 4096 + 96;
        bad_id[id_offset..id_offset + 2].copy_from_slice(&65535_u16.to_le_bytes());
        assert!(
            read_table(
                &mut Cursor::new(&bad_id),
                SCOPE,
                table,
                &mut budget(),
                &|| false
            )
            .is_err()
        );

        if table == "BooleanValues" {
            let ordinary_after = raw::update(&ignored, definition, 0, true);
            let mut historical = ignored;
            let mut expected_after = ordinary_after;
            let maximum = definition as usize * 4096 + 41;
            historical[maximum..maximum + 2].copy_from_slice(&3_u16.to_le_bytes());
            expected_after[maximum..maximum + 2].copy_from_slice(&3_u16.to_le_bytes());
            assert_eq!(rows(&historical, table), expected);
            let candidate = plan(&historical, 0, false, true).unwrap();
            equal_image(candidate.after_bytes(), &expected_after);
            assert_eq!(
                &candidate.after_bytes()[offset..offset + 2],
                &65535_u16.to_le_bytes()
            );
        }
    }
}

#[test]
fn retained_boolean_rows_refuse_null_absent_bits_wrong_original_and_old_actions() {
    let (before, definition, _) = source();
    assert!(matches!(
        plan(&before, 0, true, false),
        Err(EditError::Conflict)
    ));
    for (expected, replacement) in [
        (Value::Boolean(false), Value::Null),
        (Value::Null, Value::Boolean(true)),
        (Value::Int32(0), Value::Boolean(true)),
    ] {
        let requests = [FixedScalarUpdate::new(0, expected, replacement)];
        let groups = [FittedColumnUpdates::Fixed {
            column: "Enabled",
            updates: &requests,
        }];
        assert!(matches!(
            image(&before).plan_retained_boolean_row_updates(
                "BooleanValues",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    let requests = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    assert!(
        image(&before)
            .plan_retained_row_updates("BooleanValues", &groups, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .plan_retained_currency_row_updates("BooleanValues", &groups, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .plan_retained_opaque_row_updates("BooleanValues", &groups, &mut budget(), &|| false)
            .is_err()
    );
    assert!(matches!(
        image(&before).plan_retained_boolean_row_updates(
            "BooleanValues",
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
            image(&before).plan_retained_boolean_row_updates(
                "BooleanValues",
                &groups,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let mut absent = before;
    let declaration = definition as usize * 4096;
    absent[declaration + 41..declaration + 43].copy_from_slice(&3_u16.to_le_bytes());
    absent[declaration + 105..declaration + 107].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        rows(&absent, "BooleanValues"),
        vec![
            vec![Value::Int32(1), Value::Boolean(false)],
            vec![Value::Int32(2), Value::Boolean(false)]
        ]
    );
    assert!(
        image(&absent)
            .plan_retained_boolean_row_updates("BooleanValues", &groups, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn retained_boolean_capability_preserves_currency_long_and_text_actions_in_one_table() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let empty = raw::mixed::append_table(&seed, "Mixed");
    let mut before = raw::mixed::append(&empty, definition);
    let native = image(&before);
    native
        .check_native_retained_cell_updates_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    native
        .check_native_retained_boolean_rows_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    native
        .check_native_retained_currency_rows_table("Mixed", &mut budget(), &|| false)
        .unwrap();
    native
        .check_native_retained_rows_table("Mixed", &mut budget(), &|| false)
        .unwrap();

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    let expected = raw::mixed::update_bool(&before, definition, 0, true);
    let boolean = image(&before)
        .plan_retained_boolean_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(boolean.after_bytes(), &expected);
    assert!(matches!(
        image(&before).plan_retained_row_updates("Mixed", &groups, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    assert!(matches!(
        image(&before)
            .plan_retained_currency_row_updates("Mixed", &groups, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    before = expected;

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Currency(Currency::from_scaled(0)),
        Value::Currency(Currency::from_scaled(12345)),
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
        image(&before)
            .plan_retained_boolean_row_updates("Mixed", &groups, &mut budget(), &|| false),
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

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Boolean(true),
        Value::Boolean(false),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    let expected = raw::mixed::update_bool(&before, definition, 0, false);
    let boolean = image(&before)
        .plan_retained_boolean_row_updates("Mixed", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(boolean.after_bytes(), &expected);
    assert_eq!(
        rows(boolean.after_bytes(), "Mixed"),
        vec![vec![
            Value::Int32(1),
            Value::Int32(123),
            Value::Currency(Currency::from_scaled(12345)),
            Value::Text("日本語 🦀".into()),
            Value::Boolean(false),
        ]]
    );
    check_table(
        &mut Cursor::new(boolean.after_bytes()),
        SCOPE,
        "Mixed",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}
