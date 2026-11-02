//! Independently qualified literal images enter proof without staging/planning.
use super::proof;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan, WriteBlocker};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_generated_companion_update::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(243)
}

#[test]
#[ignore = "requires twelve independently qualified generated-companion Score images"]
fn literal_score_proof_preserves_generated_counter_defaults_deleted_rows_and_index_complement() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut positives = 0;
    let mut byte_refusals = 0;
    let mut count_refusals = 0;
    let mut stale_refusals = 0;
    let mut generated_refusals = 0;
    for case in &cases {
        for &reverse in case.orders() {
            let updates: Vec<_> = case
                .updates(reverse)
                .into_iter()
                .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                .collect();
            let groups = [FixedColumnUpdates::new(case.table, "Score", &updates)];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed(),
            };
            proof::prove(&plan, scope(), case.table, &groups, &mut budget(), &|| {
                false
            })
            .unwrap();
            positives += 1;
        }
        let updates: Vec<_> = case
            .updates(false)
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect();
        let groups = [FixedColumnUpdates::new(case.table, "Score", &updates)];
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed(),
            };
            assert_eq!(
                proof::prove(&plan, scope(), case.table, &groups, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Preservation)
            );
            byte_refusals += 1;
        }
        let wrong_count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed() + 1,
        };
        assert_eq!(
            proof::prove(
                &wrong_count,
                scope(),
                case.table,
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        count_refusals += 1;

        // Replacing the original groups with a stale witness makes the no-op
        // and named-empty literal plans identical. Keep only one such context.
        if case.recipe != Recipe::Empty {
            let stale = [FixedScalarUpdate::new(
                0,
                Value::Int32(11),
                Value::Int32(17),
            )];
            let groups = [FixedColumnUpdates::new(case.table, "Score", &stale)];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed(),
            };
            assert_eq!(
                proof::prove(&plan, scope(), case.table, &groups, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Conflict)
            );
            stale_refusals += 1;
        }
        if case.recipe == Recipe::Changed {
            let groups = [FixedColumnUpdates::new(case.table, "Id", &[])];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.before.clone(),
                changed_rows: 0,
            };
            assert_eq!(
                proof::prove(&plan, scope(), case.table, &groups, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Unsupported(WriteBlocker::Column))
            );
            generated_refusals += 1;
        }
    }
    assert_eq!(positives, 18);
    assert_eq!(byte_refusals, 272);
    assert_eq!(count_refusals, 16);
    assert_eq!(stale_refusals, 12);
    assert_eq!(generated_refusals, 4);
}
