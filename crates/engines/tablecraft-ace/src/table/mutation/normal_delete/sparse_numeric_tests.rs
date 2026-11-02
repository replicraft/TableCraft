//! Literal numeric delete proof: no planner, stage keys or reservations.
use super::proof;
use crate::{NativeEditPlan, RowDelete};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::qualified_sparse_numeric as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires source-qualified sparse, stored-ID and crossing-byte literal views"]
fn literal_sparse_delete_images_prove_and_raw_carrier_history_and_count_forgeries_refuse() {
    let cases = sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    });
    for case in cases.into_iter().filter(|case| case.operation == "delete") {
        let requests = vec![0, 1, 2]
            .into_iter()
            .map(|slot| RowDelete::new(slot, case.record(slot)))
            .collect::<Vec<_>>();
        let count = requests.len();
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: count,
        };
        let scope = CatalogScope::new(197);
        proof::prove(&plan, scope, case.table, &requests, &mut budget(), &|| {
            false
        })
        .unwrap_or_else(|error| {
            panic!(
                "literal proof failed: {} {} {} h{}: {error:?}",
                case.table, case.family, case.recipe, case.header
            )
        });
        for at in [
            case.data as usize * 4096 + 4096 - 2 * case.record_bytes() + 6,
            case.roots[1] as usize * 4096 + 480,
            case.definition as usize * 4096 + 99 + 25 + 9,
            24,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
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
            before: case.before.clone(),
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
