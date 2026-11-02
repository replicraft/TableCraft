//! Literal source plans enter only the fresh independent omitted-INSERT proof.
use crate::{InsertFieldValue, NativeEditPlan, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::creation_date_defaults as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests(phase: &str) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(if phase == "omitted" {
        vec![
            InsertFieldValue::Supplied(Value::Int32(2)),
            InsertFieldValue::Omitted,
        ]
    } else {
        vec![
            InsertFieldValue::Supplied(Value::Int32(3)),
            InsertFieldValue::Supplied(Value::Null),
        ]
    })]
}

#[test]
#[ignore = "requires all source36 native parser/bit/Source gates; calls no planner or serializer"]
fn literal_date_after_images_pass_independent_proof_and_bit_mask_count_request_forgeries_fail() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    });
    let scope = CatalogScope::new(232);
    let mut positives = 0;
    let mut denials = 0;
    for header in [5, 6] {
        for case in fixtures::CASES {
            for phase in ["omitted", "supplied-null"] {
                let before = sources.bytes(header, case, "before");
                let after = sources.bytes(header, case, phase);
                let plan = NativeEditPlan {
                    before: before.clone(),
                    after: after.clone(),
                    changed_rows: 1,
                };
                super::proof::prove(
                    &plan,
                    scope,
                    "Items",
                    &requests(phase),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                positives += 1;
                // All are independent original/source offsets: new row starts
                // 4066, Date follows count2+Id4, and its final mask is byte14.
                for offset in [8 * 4096 + 4066 + 6, 8 * 4096 + 4066 + 14, 20] {
                    let mut forged = after.clone();
                    forged[offset] ^= if offset == 20 {
                        3
                    } else if offset == 8 * 4096 + 4066 + 14 {
                        2
                    } else {
                        1
                    };
                    let plan = NativeEditPlan {
                        before: before.clone(),
                        after: forged,
                        changed_rows: 1,
                    };
                    assert!(
                        super::proof::prove(
                            &plan,
                            scope,
                            "Items",
                            &requests(phase),
                            &mut budget(),
                            &|| false
                        )
                        .is_err()
                    );
                    denials += 1;
                }
                let wrong_count = NativeEditPlan {
                    before: before.clone(),
                    after: after.clone(),
                    changed_rows: 0,
                };
                assert!(
                    super::proof::prove(
                        &wrong_count,
                        scope,
                        "Items",
                        &requests(phase),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
                denials += 1;
                let other = vec![OmittedRowInsert::new(vec![
                    InsertFieldValue::Supplied(Value::Int32(if phase == "omitted" {
                        2
                    } else {
                        3
                    })),
                    if phase == "omitted" {
                        InsertFieldValue::Supplied(Value::Null)
                    } else {
                        InsertFieldValue::Omitted
                    },
                ])];
                assert!(
                    super::proof::prove(&plan, scope, "Items", &other, &mut budget(), &|| false)
                        .is_err()
                );
                denials += 1;
                if case.id == "epoch-zero" && phase == "omitted" {
                    let negative_zero = vec![OmittedRowInsert::new(vec![
                        InsertFieldValue::Supplied(Value::Int32(2)),
                        InsertFieldValue::Supplied(fixtures::date(0x8000000000000000)),
                    ])];
                    assert!(
                        super::proof::prove(
                            &plan,
                            scope,
                            "Items",
                            &negative_zero,
                            &mut budget(),
                            &|| false
                        )
                        .is_err()
                    );
                    denials += 1;
                }
            }
        }
    }
    assert_eq!((positives, denials), (24, 122));
}
