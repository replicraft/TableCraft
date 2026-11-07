//! Original omission resolves complete inspected absence, without changing TCNA2.
use super::{Storage, budget, fixtures, image, requests, scope, wire_rows};
use std::cell::Cell;
use tablecraft_ace::{
    ConstraintViolation, CreateColumn, EditError, InsertFieldValue, InspectError,
    NativeOmittedInsertIntent, OmittedRowInsert, RecoverableEditError, create_ace16_database,
    qualify_native_omitted_insert_recovery, read_table,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_schema::{FieldConstraintViolation, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_creation_properties::{Policy, with_policies};

fn omitted() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Omitted,
    ])]
}

#[test]
fn omitted_nullable_companions_match_full_supplied_rows_and_independent_native_images() {
    for recipe in [
        fixtures::InsertCase::LongIntegerNull,
        fixtures::InsertCase::ShortTextNull,
    ] {
        for required in [None, Some(false)] {
            let mut case = fixtures::first_insert(recipe);
            if let Some(required) = required {
                let column = if matches!(recipe, fixtures::InsertCase::LongIntegerNull) {
                    "Score"
                } else {
                    "Name"
                };
                let policies = [Policy {
                    column,
                    required: Some(required),
                    allow_zero_length: None,
                }];
                case.before = with_policies(case.before, &policies);
                case.after = with_policies(case.after, &policies);
            }
            assert!(
                !image(&case.before)
                    .select_native_ordinary_clock_defaults(
                        "Items",
                        &omitted(),
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
            );
            let plan = image(&case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    omitted(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            let supplied = image(&case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), supplied.after_bytes());
            let literal = wire_rows(&case.before, vec![vec![None, None]]);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                literal
            );
            assert_ne!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                supplied.intent().encode(&mut budget(), &|| false).unwrap(),
            );
            assert_eq!(
                read_table(
                    &mut image(plan.after_bytes()).reader(),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .rows(),
                &[case.row],
            );
        }
    }
}

#[test]
fn nullable_absence_reconstructs_original_intent_for_commit_and_interrupted_recovery() {
    let case = fixtures::first_insert(fixtures::InsertCase::ShortTextNull);
    for committed in [false, true] {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                omitted(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let literal = wire_rows(&case.before, vec![vec![None, None]]);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        if !committed {
            target.fail_prefix = Some(8 * 4096 + 4090);
        }
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
        } else {
            assert!(prepared.commit(&mut budget(), &|| false).is_err());
            assert_ne!(target.bytes.get_ref(), &case.before);
            assert_ne!(target.bytes.get_ref(), &case.after);
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeOmittedInsertIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        assert!(
            intent.rows()[0]
                .values()
                .iter()
                .all(|value| matches!(value, InsertFieldValue::Omitted))
        );
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
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            },
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &case.after } else { &case.before }
        );
    }
}

#[test]
fn absent_default_does_not_bypass_required_or_boolean_storage_rules() {
    let case = fixtures::first_insert(fixtures::InsertCase::LongIntegerNull);
    let before = with_policies(
        case.before,
        &[Policy {
            column: "Score",
            required: Some(true),
            allow_zero_length: None,
        }],
    );
    assert!(matches!(
        image(&before).plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            omitted(),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Inspect(
            InspectError::Constraint(ConstraintViolation::Field(
                FieldConstraintViolation::Required
            ))
        )))
    ));

    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Flag", ScalarType::Boolean).with_required(false),
    ];
    let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
        .unwrap()
        .as_bytes()
        .to_vec();
    assert!(
        image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                omitted(),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    // Empty operations still inspect the original declaration and create no row.
    let empty = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts("Items", Vec::new(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(empty.before_bytes(), empty.after_bytes());
    assert_eq!(empty.inserted_rows(), 0);

    let no = Value::Boolean(false);
    let columns = [columns[0], columns[1].with_default_literal(&no).unwrap()];
    let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
        .unwrap()
        .as_bytes()
        .to_vec();
    let plan = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts("Items", omitted(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        read_table(
            &mut image(plan.after_bytes()).reader(),
            scope(),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(1), Value::Boolean(false)]],
    );
}

#[test]
fn nullable_absence_pays_complete_plan_pools_and_polls_final_cancellation() {
    let case = fixtures::first_insert(fixtures::InsertCase::ShortTextNull);
    let invoke = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
        image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            omitted(),
            work,
            cancelled,
        )
    };
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut measured = budget();
    let initial = pools.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0_usize);
    invoke(&mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(pools[index]));
    assert!(used.iter().all(|cost| *cost > 0));
    invoke(&mut ResourceBudget::new(used[0], used[1], used[2]), &|| {
        false
    })
    .unwrap();
    for index in 0..pools.len() {
        let mut limited = used;
        limited[index] -= 1;
        assert!(
            invoke(
                &mut ResourceBudget::new(limited[0], limited[1], limited[2]),
                &|| false
            )
            .is_err()
        );
    }
    for stop in [1, polls.get().div_ceil(2), polls.get()] {
        let seen = Cell::new(0_usize);
        assert!(
            invoke(&mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .is_err()
        );
        assert_eq!(seen.get(), stop);
    }
}

#[test]
fn all_qualified_nullable_companion_carriers_resolve_absent_defaults_to_null() {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Number", ScalarType::Int32).with_required(false),
        CreateColumn::new("Large", ScalarType::Int64).with_required(false),
        CreateColumn::new("Money", ScalarType::Currency).with_required(false),
        CreateColumn::new("When", ScalarType::DateTime).with_required(false),
        CreateColumn::short_text("Name", 20)
            .unwrap()
            .with_required(false),
        CreateColumn::short_binary("Payload", 16)
            .unwrap()
            .with_required(false),
    ];
    let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
        .unwrap()
        .as_bytes()
        .to_vec();
    let original = OmittedRowInsert::new(
        (0..columns.len())
            .map(|_| InsertFieldValue::Omitted)
            .collect(),
    );
    let plan = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            vec![original],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let literal = wire_rows(&before, vec![vec![None; 7]]);
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        literal
    );
    let mut expected = vec![Value::Int32(1)];
    expected.extend((1..columns.len()).map(|_| Value::Null));
    assert_eq!(
        read_table(
            &mut image(plan.after_bytes()).reader(),
            scope(),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .rows(),
        &[expected],
    );
}

#[test]
fn present_null_text_and_empty_defaults_keep_precedence_over_nullable_absence() {
    for default in [
        Value::Null,
        Value::Text("stored".into()),
        Value::Text(String::new()),
    ] {
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32)
                .with_primary_key("PrimaryId")
                .unwrap()
                .with_incrementing_autonumber()
                .unwrap(),
            CreateColumn::short_text("Name", 20)
                .unwrap()
                .with_required(false)
                .with_allow_zero_length(true)
                .unwrap()
                .with_default_literal(&default)
                .unwrap(),
        ];
        let before = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false)
            .unwrap()
            .as_bytes()
            .to_vec();
        for supplied in [false, true] {
            let original = OmittedRowInsert::new(vec![
                InsertFieldValue::Omitted,
                if supplied {
                    InsertFieldValue::Supplied(Value::Null)
                } else {
                    InsertFieldValue::Omitted
                },
            ]);
            let plan = image(&before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    vec![original],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let expected = if supplied {
                Value::Null
            } else {
                default.clone()
            };
            assert_eq!(
                read_table(
                    &mut image(plan.after_bytes()).reader(),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .rows(),
                &[vec![Value::Int32(1), expected]],
            );
            let literal = wire_rows(&before, vec![vec![None, supplied.then_some(Value::Null)]]);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                literal
            );
        }
    }
}
