//! Literal complete source AFTERs prove without any planner, serializer or stage.
use super::proof;
use crate::{InsertFieldValue, NativeEditPlan, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::normal_clock_defaults::{self as fixtures, Field};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests(case: &fixtures::Case<'_>) -> Vec<OmittedRowInsert> {
    case.requests()
        .into_iter()
        .map(|row| {
            OmittedRowInsert::new(
                row.into_iter()
                    .map(|field| match field {
                        Field::Supplied(value) => InsertFieldValue::Supplied(value),
                        Field::Omitted => InsertFieldValue::Omitted,
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
#[ignore = "requires independent clock source18 native/SDK qualification before manual proof"]
fn literal_clock_and_no_read_outcomes_prove_and_causal_forgeries_refuse_without_stage() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let scope = CatalogScope::new(253);
    let mut accepted = 0;
    let mut refused = 0;
    for case in sources.cases() {
        let rows = requests(&case);
        let reading = case
            .needs_clock()
            .then(|| ClockReading::new(case.clock_input()).unwrap());
        let plan = NativeEditPlan {
            before: case.before.to_vec(),
            after: case.after.to_vec(),
            changed_rows: rows.len(),
        };
        proof::prove(
            &plan,
            scope,
            "Items",
            &rows,
            reading,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        accepted += 1;
        for at in [152, 3 * 4096 + 4095, 8 * 4096 + 1000, 8 * 4096 + 4083] {
            let mut forged = case.after.to_vec();
            forged[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.to_vec(),
                after: forged,
                changed_rows: rows.len(),
            };
            assert!(
                proof::prove(
                    &forged,
                    scope,
                    "Items",
                    &rows,
                    reading,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            refused += 1;
        }
        let wrong_count = NativeEditPlan {
            before: case.before.to_vec(),
            after: case.after.to_vec(),
            changed_rows: rows.len() + 1,
        };
        assert!(
            proof::prove(
                &wrong_count,
                scope,
                "Items",
                &rows,
                reading,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        refused += 1;
        if case.recipe != fixtures::Recipe::Empty {
            let wrong_rows = case
                .requests()
                .into_iter()
                .enumerate()
                .map(|(ordinal, row)| {
                    OmittedRowInsert::new(
                        row.into_iter()
                            .enumerate()
                            .map(|(position, field)| {
                                if ordinal == 0 && position == 0 {
                                    InsertFieldValue::Supplied(Value::Int32(99))
                                } else {
                                    match field {
                                        Field::Supplied(value) => InsertFieldValue::Supplied(value),
                                        Field::Omitted => InsertFieldValue::Omitted,
                                    }
                                }
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            assert!(
                proof::prove(
                    &plan,
                    scope,
                    "Items",
                    &wrong_rows,
                    reading,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            refused += 1;
            for at in [8 * 4096 + 4072, 8 * 4096 + 4080] {
                let mut forged = case.after.to_vec();
                forged[at] ^= 1;
                let forged = NativeEditPlan {
                    before: case.before.to_vec(),
                    after: forged,
                    changed_rows: rows.len(),
                };
                assert!(
                    proof::prove(
                        &forged,
                        scope,
                        "Items",
                        &rows,
                        reading,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
                refused += 1;
            }
        }
        let extra_or_missing = if reading.is_some() {
            None
        } else {
            Some(ClockReading::new(case.clock_input()).unwrap())
        };
        assert!(
            proof::prove(
                &plan,
                scope,
                "Items",
                &rows,
                extra_or_missing,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        refused += 1;
    }
    assert_eq!(accepted, 18);
    assert_eq!(refused, 150);
}
