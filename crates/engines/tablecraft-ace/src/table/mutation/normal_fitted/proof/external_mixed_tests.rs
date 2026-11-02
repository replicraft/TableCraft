//! Literal plans exercise fresh proof without invoking a planner or serializer.
use crate::{FittedColumnUpdates, NativeEditPlan, NativeFittedIntentGroup, NativeFittedUpdates};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_external_lval_mixed as fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn from_original(
    groups: Vec<tablecraft_testkit::fixtures::access::genuine_external_lval_update::Group>,
) -> Vec<NativeFittedIntentGroup> {
    fixture::map_groups(groups, crate::SlotScalarUpdate::new, |column, updates| {
        NativeFittedIntentGroup::new(
            column,
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    })
}

fn borrowed(owned: &[NativeFittedIntentGroup]) -> Vec<FittedColumnUpdates<'_>> {
    owned
        .iter()
        .map(|group| match group.updates() {
            NativeFittedUpdates::Variable(updates) => FittedColumnUpdates::Variable {
                column: group.column(),
                updates,
            },
            NativeFittedUpdates::Fixed(_) => unreachable!(),
        })
        .collect()
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_literal_proof_rejects_moved_descriptor_slack_payload_maps_and_counts() {
    let cases = fixture::load_all(|raw, expected| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut proved = 0;
    let mut refused = 0;
    for case in cases.iter().filter(|case| case.recipe != "original") {
        for reverse in [false, true] {
            let owned = from_original(fixture::request_groups(case, reverse));
            let groups = borrowed(&owned);
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: fixture::changed_rows(case),
            };
            super::prove(
                &plan,
                CatalogScope::new(193),
                "Items",
                &groups,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            proved += 1;
            for at in fixture::forged_offsets(case) {
                let mut after = case.after.clone();
                after[at] ^= 1;
                let forged = NativeEditPlan {
                    before: case.before.clone(),
                    after,
                    changed_rows: fixture::changed_rows(case),
                };
                assert!(
                    super::prove(
                        &forged,
                        CatalogScope::new(193),
                        "Items",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
                refused += 1;
            }
            let wrong_count = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: fixture::changed_rows(case) + 1,
            };
            assert!(
                super::prove(
                    &wrong_count,
                    CatalogScope::new(193),
                    "Items",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            refused += 1;
            for stale in 0..2 {
                let mut requests = fixture::request_groups(case, reverse);
                requests[stale].updates[0].expected = tablecraft_core::values::Value::Null;
                let wrong = from_original(requests);
                assert!(
                    super::prove(
                        &plan,
                        CatalogScope::new(193),
                        "Items",
                        &borrowed(&wrong),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
                refused += 1;
            }
        }
    }
    assert_eq!((proved, refused), (32, 672));
}
