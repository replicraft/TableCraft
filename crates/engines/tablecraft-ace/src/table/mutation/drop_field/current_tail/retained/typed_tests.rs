//! Typed metadata retirement preserves physical records and separate row authority.
use crate::{
    FittedColumnUpdates, FixedScalarUpdate, InsertFieldValue, NativeImage, OmittedRowInsert,
    inspect_table_schema, read_table,
    table::mutation::{current_schema, record_serialization},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{
        boolean, currency, exact_values, field_changes, fields, fixed_scalars,
    },
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4104);

#[derive(Clone, Copy, Debug)]
enum Case {
    Fixed,
    Exact,
    Currency,
    Boolean,
    Text,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn source(case: Case) -> (Vec<u8>, u32, usize, &'static [&'static str]) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let (bytes, names): (Vec<u8>, &'static [&'static str]) = match case {
        Case::Fixed => {
            let empty = fixed_scalars::append_table(&seed, "Values");
            (
                fixed_scalars::append_values(
                    &empty,
                    definition,
                    &[
                        fixed_scalars::Scalar::Byte(17),
                        fixed_scalars::Scalar::Int16(-1234),
                        fixed_scalars::Scalar::Int64(9_007_199_254_740_993),
                        fixed_scalars::Scalar::SingleBits(0x3fa0_0000),
                        fixed_scalars::Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
                        fixed_scalars::Scalar::DateTimeBits(0x4004_0000_0000_0000),
                    ],
                ),
                &[
                    "DateValue",
                    "DoubleValue",
                    "SingleValue",
                    "LargeValue",
                    "ShortValue",
                    "ByteValue",
                    "Score",
                ],
            )
        }
        Case::Exact => {
            let empty = exact_values::append_table(&seed, "Values");
            (
                exact_values::append(
                    &empty,
                    definition,
                    Some([0x31; 16]),
                    Some(90_071_992_547_409_931_234),
                ),
                &["Amount", "Token"],
            )
        }
        Case::Currency => {
            let empty = currency::append_table(&seed, "Values");
            (
                currency::append(&empty, definition, currency::Input::Value(12345)),
                &["Amount"],
            )
        }
        Case::Boolean => {
            let empty = boolean::append_table(&seed, "Values");
            (
                boolean::append(&empty, definition, boolean::Input::Value(true)),
                &["Enabled"],
            )
        }
        Case::Text => {
            let empty = currency::mixed::append_table(&seed, "Values");
            let inserted = currency::mixed::append(&empty, definition);
            (
                currency::mixed::update_text(&inserted, definition, 0, Some("original")),
                &["Label"],
            )
        }
    };
    (bytes, definition, root, names)
}

fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        SCOPE,
        "Values",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_rows()
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
fn retained_typed_tail_drop_preserves_all_scalar_storage_and_high_water_history() {
    for case in [
        Case::Fixed,
        Case::Exact,
        Case::Currency,
        Case::Boolean,
        Case::Text,
    ] {
        for forwarded in [false, true] {
            let (mut before, definition, root, names) = source(case);
            if forwarded {
                before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
                before = raw_forwarding_chain::extend(&before, 4, root, 20);
            }
            for name in names {
                let native = image(&before);
                let schema = inspect_table_schema(
                    &mut native.reader(),
                    SCOPE,
                    "Values",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let mut expected_rows = rows(&before);
                for row in &mut expected_rows {
                    row.pop().unwrap();
                }
                let old_shape = matches!(*name, "Score" | "Label");
                assert_eq!(
                    native
                        .select_native_retained_typed_tail_field_drop_source(
                            "Values",
                            name,
                            &mut budget(),
                            &|| false
                        )
                        .unwrap(),
                    !old_shape
                );
                let old = image(&before).prepare_retained_tail_field_drop(
                    "Values",
                    name,
                    &mut budget(),
                    &|| false,
                );
                if old_shape {
                    assert!(old.unwrap().is_retained());
                } else {
                    assert!(old.is_err());
                }
                let expected = field_changes::drop_tail(&before, definition, root, name);
                let candidate = native
                    .prepare_retained_typed_tail_field_drop("Values", name, &mut budget(), &|| {
                        false
                    })
                    .unwrap_or_else(|error| panic!("{case:?}/{forwarded}/{name}: {error:?}"));
                assert!(candidate.is_retained_typed());
                assert!(!candidate.is_retained());
                equal_image(candidate.after_bytes(), &expected);
                assert_eq!(rows(candidate.after_bytes()), expected_rows);
                let after = inspect_table_schema(
                    &mut Cursor::new(candidate.after_bytes()),
                    SCOPE,
                    "Values",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(after.columns().len() + 1, schema.columns().len());
                assert_eq!(
                    after.physical_layout().max_columns,
                    schema.physical_layout().max_columns
                );
                assert_eq!(
                    after.physical_layout().variable_slots,
                    schema.physical_layout().variable_slots
                );
                for page in [definition + 1, definition + 2] {
                    let start = page as usize * 4096;
                    assert_eq!(
                        &candidate.after_bytes()[start..start + 4096],
                        &before[start..start + 4096]
                    );
                }
                before = expected;
            }
            if matches!(case, Case::Fixed | Case::Currency | Case::Boolean) {
                assert!(
                    image(&before)
                        .prepare_retained_typed_tail_field_drop(
                            "Values",
                            "ID",
                            &mut budget(),
                            &|| false
                        )
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn retained_typed_tail_drop_leaves_complete_survivors_editable_and_fresh_inserts_exact() {
    let (mut before, definition, root, _) = source(Case::Exact);
    for name in ["Amount", "Token"] {
        let expected = field_changes::drop_tail(&before, definition, root, name);
        let candidate = image(&before)
            .prepare_retained_typed_tail_field_drop("Values", name, &mut budget(), &|| false)
            .unwrap();
        equal_image(candidate.after_bytes(), &expected);
        before = expected;
    }
    image(&before)
        .check_native_retained_rows_table("Values", &mut budget(), &|| false)
        .unwrap();
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(0),
        Value::Int32(124),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &updates,
    }];
    let candidate = image(&before)
        .plan_retained_row_updates("Values", &groups, &mut budget(), &|| false)
        .unwrap();
    let expected = field_changes::update_score(&before, definition, 0, Some(124));
    equal_image(candidate.after_bytes(), &expected);
    before = expected;
    let request = OmittedRowInsert::new(vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted]);
    let candidate = image(&before)
        .plan_retained_omitted_row_inserts("Values", &[request], &mut budget(), &|| false)
        .unwrap();
    let expected = field_changes::append_score(&before, definition, Some(0));
    equal_image(candidate.after_bytes(), &expected);
    assert_eq!(
        rows(candidate.after_bytes()),
        vec![
            vec![Value::Int32(1), Value::Int32(124)],
            vec![Value::Int32(2), Value::Int32(0)]
        ]
    );
}

#[test]
fn retained_typed_tail_drop_preserves_genuine_ambiguous_history_without_row_authority() {
    let (mut before, definition, root, _) = source(Case::Exact);
    before = fields::after(&before, definition, root, fields::Target::Number);
    before = fields::after(&before, definition, root, fields::Target::ShortText);
    for name in ["NextText", "NextNumber"] {
        before = field_changes::drop_tail(&before, definition, root, name);
    }
    let expected = field_changes::drop_tail(&before, definition, root, "Amount");
    let candidate = image(&before)
        .prepare_retained_typed_tail_field_drop("Values", "Amount", &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.after_bytes(), &expected);
    before = expected;
    let native = image(&before);
    let current = current_schema::Family::Retained
        .observe(&native, "Values", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(current.schema.physical_layout().max_columns, 6);
    assert_eq!(current.schema.physical_layout().variable_slots, 2);
    let data = (definition as usize + 1) * 4096;
    let start = usize::from(u16::from_le_bytes(
        before[data + 14..data + 16].try_into().unwrap(),
    )) & 0x1fff;
    let record = &before[data + start..data + 4096];
    assert_eq!(&record[..2], &[4, 0]);
    assert!(
        record_serialization::opaque_rows::is_ambiguous(
            &current.schema,
            record,
            &mut budget(),
            &|| false
        )
        .unwrap()
    );
    assert!(
        native
            .check_native_retained_cell_updates_table("Values", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        native
            .check_native_retained_insert_table("Values", &mut budget(), &|| false)
            .is_err()
    );

    // Readonly schema admission can remove the next typed tail while protecting
    // every old record. This does not promote that source to Complete row proof.
    let expected = field_changes::drop_tail(&before, definition, root, "Token");
    let candidate = native
        .prepare_retained_typed_tail_field_drop("Values", "Token", &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.after_bytes(), &expected);
    before = expected;
    assert_eq!(rows(&before), vec![vec![Value::Int32(1), Value::Int32(0)]]);
    let native = image(&before);
    assert!(
        native
            .check_native_retained_rows_table("Values", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        native
            .select_native_retained_opaque_insert_table("Values", &mut budget(), &|| false)
            .unwrap()
    );
    let request = OmittedRowInsert::new(vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted]);
    let candidate = native
        .plan_retained_opaque_omitted_row_inserts("Values", &[request], &mut budget(), &|| false)
        .unwrap();
    let expected = field_changes::append_score(&before, definition, Some(0));
    equal_image(candidate.after_bytes(), &expected);
    assert_eq!(
        rows(candidate.after_bytes()),
        vec![
            vec![Value::Int32(1), Value::Int32(0)],
            vec![Value::Int32(2), Value::Int32(0)]
        ]
    );
    assert!(
        image(candidate.after_bytes())
            .check_native_retained_rows_table("Values", &mut budget(), &|| false)
            .is_err()
    );
}
