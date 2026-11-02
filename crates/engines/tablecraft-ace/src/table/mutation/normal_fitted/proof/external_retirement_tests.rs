//! Fresh proof rejects descriptors, variable footer, slack and unrelated metadata.
use crate::{
    FittedColumnUpdates, NativeEditPlan, NativeFittedIntentGroup, NativeFittedUpdates,
    SlotScalarUpdate,
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_external_lval_continuation;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires external continuation36 source admission before matching proofs"]
fn external_retirement_fresh_proof_reconstructs_source_relative_maps_parents_and_residuals() {
    let cases = genuine_external_lval_continuation::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut proved = 0;
    let mut refused = 0;
    for case in cases.into_iter().filter(|case| {
        matches!(
            case.recipe,
            "clear-single-blob"
                | "clear-single-memo"
                | "clear-single-both"
                | "empty-single-blob"
                | "clear-memo-chain"
        )
    }) {
        let owned = case
            .groups
            .iter()
            .map(|group| {
                let updates = NativeFittedUpdates::Variable(
                    group
                        .updates
                        .iter()
                        .map(|cell| {
                            SlotScalarUpdate::new(
                                cell.row,
                                cell.expected.clone(),
                                cell.replacement.clone(),
                            )
                        })
                        .collect(),
                );
                NativeFittedIntentGroup::new(group.column, updates, &mut budget(), &|| false)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let groups = owned
            .iter()
            .map(|group| match group.updates() {
                NativeFittedUpdates::Fixed(updates) => FittedColumnUpdates::Fixed {
                    column: group.column(),
                    updates,
                },
                NativeFittedUpdates::Variable(updates) => FittedColumnUpdates::Variable {
                    column: group.column(),
                    updates,
                },
            })
            .collect::<Vec<_>>();
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed,
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
        if !matches!(case.recipe, "clear-single-both" | "clear-memo-chain") {
            continue;
        }
        let delta = usize::from(case.header == 6) * 17;
        let retired = genuine_external_lval_continuation::retired_pages(&case);
        let map_change = case
            .before
            .iter()
            .zip(&case.after)
            .enumerate()
            .find_map(|(at, (old, new))| {
                let page = at / 4096;
                (old != new && page != case.data as usize && !retired.contains(&(page as u32)))
                    .then_some(at)
            })
            .unwrap();
        for offset in [
            retired[0] as usize * 4096,
            retired[0] as usize * 4096 + 4095,
            (103 + delta) * 4096 + 4095,
            case.definition as usize * 4096 + 20,
            map_change,
        ] {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed,
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
        let forged = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed + 1,
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
    assert_eq!(proved, 20);
    assert_eq!(refused, 48);
}
