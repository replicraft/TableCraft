use super::{Storage, budget, fixtures, image, requests, scope, wire};
use tablecraft_ace::{
    ConstraintViolation, CreateColumn, EditError, InsertFieldValue, InspectError,
    NativeOmittedInsertIntent, OmittedRowInsert, RecoverableEditError, create_ace16_database,
    qualify_native_omitted_insert_recovery, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_schema::{FieldConstraintViolation, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_creation_properties::{Policy, with_policies};

#[test]
fn generated_insert_enforces_required_and_empty_text_independently_from_literal_sources() {
    for required in [false, true] {
        for allow_zero_length in [false, true] {
            let policies = [Policy {
                column: "Name",
                required: Some(required),
                allow_zero_length: Some(allow_zero_length),
            }];
            for recipe in [
                fixtures::InsertCase::ShortText,
                fixtures::InsertCase::ShortTextNull,
                fixtures::InsertCase::ShortTextEmpty,
            ] {
                let mut case = fixtures::first_insert(recipe);
                case.before = with_policies(case.before, &policies);
                case.after = with_policies(case.after, &policies);
                let result = image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(&case),
                    &mut budget(),
                    &|| false,
                );
                let violation = match recipe {
                    fixtures::InsertCase::ShortTextNull if required => {
                        Some(FieldConstraintViolation::Required)
                    }
                    fixtures::InsertCase::ShortTextEmpty if !allow_zero_length => {
                        Some(FieldConstraintViolation::EmptyText)
                    }
                    _ => None,
                };
                if let Some(wanted) = violation {
                    assert!(matches!(
                        result,
                        Err(RecoverableEditError::Edit(EditError::Inspect(InspectError::Constraint(ConstraintViolation::Field(actual))))) if actual == wanted
                    ));
                    continue;
                }
                let plan = result.unwrap();
                assert_eq!(plan.before_bytes(), case.before);
                assert_eq!(plan.after_bytes(), case.after);
                let literal = wire(&case);
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    literal
                );
                let mut target = Storage::new(case.before.clone());
                let mut journal = Storage::new(Vec::new());
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap();
                assert_eq!(target.bytes.get_ref(), &case.after);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeOmittedInsertIntent::decode(&literal, &mut budget(), &|| false).unwrap();
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
    }
}

#[test]
fn policy_metadata_does_not_invent_a_missing_default_or_allow_generated_null() {
    let mut case = fixtures::first_insert(fixtures::InsertCase::ShortText);
    case.before = with_policies(
        case.before,
        &[Policy {
            column: "Name",
            required: Some(true),
            allow_zero_length: Some(true),
        }],
    );
    for values in [
        vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted],
        vec![
            InsertFieldValue::Supplied(Value::Null),
            InsertFieldValue::Supplied(Value::Text("kept".into())),
        ],
    ] {
        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    vec![OmittedRowInsert::new(values)],
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
}

#[test]
fn omitted_literal_defaults_are_validated_against_actual_required_and_empty_policies() {
    for (default, required, allow, violation) in [
        (Value::Text(String::new()), true, true, None),
        (
            Value::Text(String::new()),
            true,
            false,
            Some(FieldConstraintViolation::EmptyText),
        ),
        (
            Value::Null,
            true,
            true,
            Some(FieldConstraintViolation::Required),
        ),
    ] {
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32)
                .with_primary_key("PrimaryId")
                .unwrap()
                .with_incrementing_autonumber()
                .unwrap(),
            CreateColumn::short_text("Name", 255)
                .unwrap()
                .with_required(required)
                .with_allow_zero_length(allow)
                .unwrap()
                .with_default_literal(&default)
                .unwrap(),
        ];
        let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
            .unwrap()
            .as_bytes()
            .to_vec();
        let result = image(&before).plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            vec![OmittedRowInsert::new(vec![
                InsertFieldValue::Omitted,
                InsertFieldValue::Omitted,
            ])],
            &mut budget(),
            &|| false,
        );
        if let Some(wanted) = violation {
            assert!(matches!(
                result,
                Err(RecoverableEditError::Edit(EditError::Inspect(InspectError::Constraint(ConstraintViolation::Field(actual))))) if actual == wanted
            ));
            continue;
        }

        let plan = result.unwrap();
        let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let expected = plan.after_bytes().to_vec();
        let mut target = Storage::new(before);
        let mut journal = Storage::new(Vec::new());
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.bytes.get_ref(), &expected);
        assert_eq!(
            read_table(
                &mut image(&expected).reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .rows(),
            &[vec![Value::Int32(1), Value::Text(String::new())]],
        );
        let intent = NativeOmittedInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        assert!(
            intent.rows()[0]
                .values()
                .iter()
                .all(|value| matches!(value, InsertFieldValue::Omitted))
        );
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
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
            RecoveryOutcome::Committed,
        );
        assert_eq!(target.writes, writes);
    }
}
