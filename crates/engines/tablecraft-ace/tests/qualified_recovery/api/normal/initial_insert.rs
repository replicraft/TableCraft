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
