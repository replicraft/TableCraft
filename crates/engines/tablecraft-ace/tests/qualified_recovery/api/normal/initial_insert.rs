//! First insertion uses the existing normal route after genuine empty creation.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    CreateColumn, InsertFieldValue, NativeImage, NativeOmittedInsertIntent, OmittedRowInsert,
    create_ace16_database, qualify_native_omitted_insert_recovery, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_initial_table_creation::{self as declarations, inserts as fixtures},
    tcna2,
};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn created(case: &fixtures::FirstInsert) -> Vec<u8> {
    let mut columns = vec![
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
    ];
    match case.declaration {
        declarations::Case::IdOnly => {}
        declarations::Case::LongInteger => {
            columns.push(CreateColumn::new("Score", ScalarType::Int32));
        }
        declarations::Case::ShortText => {
            columns.push(CreateColumn::short_text("Name", 255).unwrap());
        }
        declarations::Case::Mixed => unreachable!("not a first-insert fixture"),
    }
    let database = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false).unwrap();
    assert_eq!(database.as_bytes(), case.before);
    database.as_bytes().to_vec()
}

fn presence(case: &fixtures::FirstInsert) -> Vec<Option<Value>> {
    let mut values = vec![None];
    values.extend(case.companion.clone().map(Some));
    values
}

fn requests(case: &fixtures::FirstInsert) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(
        presence(case)
            .into_iter()
            .map(|value| value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied))
            .collect(),
    )]
}

fn wire(case: &fixtures::FirstInsert) -> Vec<u8> {
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((case.before.len() as u64).to_le_bytes());
    binding.update(&case.before);
    tcna2::original_presence_wire(
        "Items",
        case.before.len(),
        binding.finalize().into(),
        vec![presence(case)],
    )
}

#[test]
fn fresh_initial_schemas_commit_first_generated_row_and_preserve_original_presence() {
    for recipe in fixtures::CASES {
        let case = fixtures::first_insert(recipe);
        let before = created(&case);
        let plan = image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before, "{recipe:?}");
        assert_eq!(plan.after_bytes(), case.after, "{recipe:?}");
        assert_eq!(plan.inserted_rows(), 1);
        let literal = wire(&case);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );

        let mut target = Storage::new(before);
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);

        let saved = image(target.bytes.get_ref());
        let expected = vec![case.row.clone()];
        assert_eq!(
            read_table(
                &mut saved.reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            expected
        );
        assert_eq!(
            read_table_index(
                &mut saved.reader(),
                scope(),
                "Items",
                "PrimaryId",
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .rows(),
            expected
        );

        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeOmittedInsertIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_omitted_insert_recovery(
            checked,
            intent,
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let writes = target.writes;
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn interrupted_first_inserts_reconstruct_original_presence_and_restore_exact_source() {
    for recipe in fixtures::CASES {
        let case = fixtures::first_insert(recipe);
        let plan = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        target.fail_prefix = Some(8 * 4096 + 4090);
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        // The target writes its original-sized image. Include the definition
        // counter and a portion of DATA, then fail before the leaf is complete.
        assert!(prepared.commit(&mut budget(), &|| false).is_err());
        assert_ne!(target.bytes.get_ref(), &case.before);
        assert_ne!(target.bytes.get_ref(), &case.after);
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent =
            NativeOmittedInsertIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_omitted_insert_recovery(
            checked,
            intent,
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
}

#[test]
fn fresh_initial_schema_never_invents_generation_defaults_or_allow_zero_length() {
    for recipe in fixtures::CASES {
        let case = fixtures::first_insert(recipe);
        let mut denied = Vec::new();
        for generated in [Value::Int32(1), Value::Null] {
            let mut values = presence(&case);
            values[0] = Some(generated);
            denied.push(values);
        }
        if case.companion.is_some() {
            let mut values = presence(&case);
            values[1] = None;
            denied.push(values);
        }
        if matches!(case.declaration, declarations::Case::ShortText) {
            denied.push(vec![None, Some(Value::Text(String::new()))]);
        }
        for values in denied {
            let request = OmittedRowInsert::new(
                values
                    .into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            );
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_omitted_row_inserts(
                        "Items",
                        vec![request],
                        &mut budget(),
                        &|| false,
                    )
                    .is_err(),
                "{recipe:?}"
            );
        }
    }
}

#[test]
fn initial_constant_companion_defaults_preserve_original_presence_through_insert_and_recovery() {
    use tablecraft_core::values::Currency;

    let defaults = [
        Value::Int32(0),
        Value::Int64(0),
        Value::Boolean(false),
        Value::Currency(Currency::from_scaled(0)),
    ];
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_default_literal(&defaults[0])
            .unwrap(),
        CreateColumn::new("Large", ScalarType::Int64)
            .with_default_literal(&defaults[1])
            .unwrap(),
        CreateColumn::new("Flag", ScalarType::Boolean)
            .with_default_literal(&defaults[2])
            .unwrap(),
        CreateColumn::new("Money", ScalarType::Currency)
            .with_default_literal(&defaults[3])
            .unwrap(),
        CreateColumn::new("When", ScalarType::DateTime),
    ];
    let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
        .unwrap()
        .as_bytes()
        .to_vec();
    let supplied = [
        Value::Int32(17),
        Value::Int64(29),
        Value::Boolean(true),
        Value::Currency(Currency::from_scaled(12345)),
        Value::Null,
    ];
    let request = |defaulted: bool| {
        let mut values = vec![InsertFieldValue::Omitted];
        values.extend(
            supplied
                .iter()
                .cloned()
                .enumerate()
                .map(|(position, value)| {
                    if defaulted && position < defaults.len() {
                        InsertFieldValue::Omitted
                    } else {
                        InsertFieldValue::Supplied(value)
                    }
                }),
        );
        OmittedRowInsert::new(values)
    };
    for defaulted in [false, true] {
        for committed in [false, true] {
            let plan = image(&before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    vec![request(defaulted)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let expected_after = plan.after_bytes().to_vec();
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut binding = Sha256::new();
            binding.update(b"TCNI0001:BEFORE\0");
            binding.update((before.len() as u64).to_le_bytes());
            binding.update(&before);
            let mut original = vec![None];
            original.extend(
                supplied
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(position, value)| {
                        if defaulted && position < defaults.len() {
                            None
                        } else {
                            Some(value)
                        }
                    }),
            );
            assert_eq!(
                wire,
                tcna2::original_presence_wire(
                    "Items",
                    before.len(),
                    binding.finalize().into(),
                    vec![original]
                )
            );
            let mut target = Storage::new(before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
            } else {
                drop(prepared);
            }
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent =
                NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_omitted_insert_recovery(
                checked,
                intent,
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let outcome = qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &expected_after } else { &before }
            );
            if committed {
                let mut expected = vec![Value::Int32(1)];
                if defaulted {
                    expected.extend(defaults.iter().cloned());
                    expected.push(Value::Null);
                } else {
                    expected.extend(supplied.iter().cloned());
                }
                assert_eq!(
                    read_table(
                        &mut image(target.bytes.get_ref()).reader(),
                        scope(),
                        "Items",
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
                    .rows(),
                    &[expected]
                );
            }
        }
    }
    let mut explicit_null = vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
    ];
    explicit_null.extend(
        supplied[1..]
            .iter()
            .cloned()
            .map(InsertFieldValue::Supplied),
    );
    let null_plan = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            vec![OmittedRowInsert::new(explicit_null)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let null_rows = read_table(
        &mut image(null_plan.after_bytes()).reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(null_rows.rows()[0][1], Value::Null);
    let omitted = OmittedRowInsert::new(
        (0..columns.len())
            .map(|_| InsertFieldValue::Omitted)
            .collect(),
    );
    assert!(
        image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                vec![omitted],
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let mut unknown = before.clone();
    let old = "DefaultValue"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let replacement = "OtherSetting"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(old.len(), replacement.len());
    let at = unknown
        .windows(old.len())
        .position(|bytes| bytes == old)
        .unwrap();
    unknown[at..at + old.len()].copy_from_slice(&replacement);
    assert!(
        image(&unknown)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                vec![request(true)],
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}
