//! Independent complete-image literals and causal pre-effect refusal controls.
use super::proof;
use crate::{EditError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert::{self as sources, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(158)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(sources::values())]
}

#[test]
#[ignore = "requires four source-qualified genuine normal INSERT images"]
fn complete_original_requests_match_literal_after_and_empty_keeps_image() {
    let all = cases();
    for case in all {
        let plan = image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &requests(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);

        let empty = image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &[], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.changed_rows(), 0);
    }
}

#[test]
#[ignore = "requires four source-qualified genuine normal INSERT images"]
fn original_payload_metadata_null_and_count_forgeries_fail_fresh_proof() {
    let all = cases();
    for case in all {
        let data = case.data as usize * 4096;
        let definition = case.definition as usize * 4096;
        for offset in [
            data + 4095,
            data + 4030,
            data + 4010,
            data + 4029,
            data + 2,
            definition + 16,
            4096 + 4095,
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after: forged,
                changed_rows: 1,
            };
            assert!(
                proof::prove(
                    &forged,
                    scope(),
                    "Sentinel",
                    &requests(),
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        let wrong_count = NativeEditPlan {
            before: case.before,
            after: case.after,
            changed_rows: 0,
        };
        assert!(
            proof::prove(
                &wrong_count,
                scope(),
                "Sentinel",
                &requests(),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
#[ignore = "requires four source-qualified genuine normal INSERT images"]
fn unsupported_new_lval_wide_switch_text_empty_text_and_wrong_carriers_refuse_without_plan() {
    let all = cases();
    for case in all {
        for values in [
            vec![
                Value::Int32(24),
                Value::Text("new".into()),
                Value::Binary(Vec::new()),
            ],
            vec![
                Value::Int32(24),
                Value::Text("\u{feff}\u{0100}".into()),
                Value::Null,
            ],
            vec![Value::Int32(24), Value::Text(String::new()), Value::Null],
            vec![Value::Int64(24), Value::Text("new".into()), Value::Null],
            vec![Value::Int32(24), Value::Text("new".into())],
        ] {
            assert!(
                image(&case.before)
                    .plan_ordinary_row_inserts(
                        "Sentinel",
                        &[RowInsert::new(values)],
                        &mut budget(),
                        &|| false,
                    )
                    .is_err()
            );
        }
        // More complete records than this owned page can contain cannot trigger
        // allocation or a map change, even though one requested row fits.
        let batch = (0..255)
            .map(|_| RowInsert::new(sources::values()))
            .collect::<Vec<_>>();
        assert!(matches!(
            image(&case.before).plan_ordinary_row_inserts(
                "Sentinel",
                &batch,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}

#[test]
#[ignore = "requires four source-qualified genuine normal INSERT images"]
fn original_insert_exact_pools_and_cancellation_never_return_partial_plan() {
    let all = cases();
    for case in all {
        let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
        let mut measured = budget();
        let initial = pools.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0);
        image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &requests(), &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
        let needed: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
        image(&case.before)
            .plan_ordinary_row_inserts(
                "Sentinel",
                &requests(),
                &mut ResourceBudget::new(needed[0], needed[1], needed[2]),
                &|| false,
            )
            .unwrap();
        for index in 0..3 {
            let mut limits = needed;
            limits[index] -= 1;
            assert!(
                image(&case.before)
                    .plan_ordinary_row_inserts(
                        "Sentinel",
                        &requests(),
                        &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                        &|| false,
                    )
                    .is_err()
            );
        }
        for stop in [1, polls.get() / 2, polls.get()] {
            let count = Cell::new(0);
            assert!(
                image(&case.before)
                    .plan_ordinary_row_inserts("Sentinel", &requests(), &mut budget(), &|| {
                        count.set(count.get() + 1);
                        count.get() == stop
                    },)
                    .is_err()
            );
        }
    }
}
