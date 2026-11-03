//! Literal plans exercise the actual wide independent proof without staging.
use super::prove;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_wide_ordinary_siblings::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires eighteen independent wider-sibling images"]
fn literal_wide_proof_protects_boolean_text_binary_footer_maps_and_complete_index_storage() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let scope = CatalogScope::new(253);
    let mut positives = 0;
    let mut forged = 0;
    let mut counts = 0;
    let mut stales = 0;
    for case in cases {
        for &reverse in case.orders() {
            let values: Vec<_> = case
                .updates(reverse)
                .into_iter()
                .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                .collect();
            let groups = [FixedColumnUpdates::new(case.table, "KeyValue", &values)];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed(),
            };
            prove(&plan, scope, case.table, &groups, &mut budget(), &|| false).unwrap();
            positives += 1;
        }
        let values: Vec<_> = case
            .updates(false)
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect();
        let groups = [FixedColumnUpdates::new(case.table, "KeyValue", &values)];
        for (offset, mask) in case.forgeries() {
            let mut after = case.after.clone();
            after[offset] ^= mask;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed(),
            };
            assert_eq!(
                prove(&plan, scope, case.table, &groups, &mut budget(), &|| false),
                Err(EditError::Preservation)
            );
            forged += 1;
        }
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed() + 1,
        };
        assert_eq!(
            prove(&plan, scope, case.table, &groups, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        counts += 1;
        // Replacing empty and no-op groups makes their stale plan inputs identical.
        if case.recipe != Recipe::Empty {
            let values = [FixedScalarUpdate::new(
                0,
                case.number(i64::MIN + 1),
                case.number(0),
            )];
            let groups = [FixedColumnUpdates::new(case.table, "KeyValue", &values)];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed(),
            };
            assert_eq!(
                prove(&plan, scope, case.table, &groups, &mut budget(), &|| false),
                Err(EditError::Conflict)
            );
            stales += 1;
        }
    }
    assert_eq!((positives, forged, counts, stales), (28, 720, 24, 20));
}
