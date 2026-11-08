//! Literal numeric insert proof: no planner, stage keys or reservations.
use super::indexed::proof;
use crate::{NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_integral_indexed as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires 34 native/SDK-qualified integral source views"]
fn literal_integral_insert_images_prove_and_raw_carrier_history_and_count_forgeries_refuse() {
    let cases = sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    });
    for case in cases.into_iter().filter(|case| case.operation == "insert") {
        let requests = sources::insert_values(case.table, case.recipe)
            .into_iter()
            .map(RowInsert::new)
            .collect::<Vec<_>>();
        let count = requests.len();
        let plan = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: count,
        };
        let scope = CatalogScope::new(197);
        proof::prove(&plan, scope, case.table, &requests, &mut budget(), &|| {
            false
        })
        .unwrap();
        for at in [
            case.data as usize * 4096 + 4064,
            case.roots[1] as usize * 4096 + 480,
            case.definition as usize * 4096 + 79,
            24,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: count,
            };
            assert!(
                proof::prove(
                    &forged,
                    scope,
                    case.table,
                    &requests,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        let wrong_count = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: count + 1,
        };
        assert!(
            proof::prove(
                &wrong_count,
                scope,
                case.table,
                &requests,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}
