//! Fresh proof rejects descriptors, variable footer, slack and unrelated metadata.
use crate::{
    FittedColumnUpdates, NativeEditPlan, NativeFittedIntentGroup, NativeFittedUpdates,
    SlotScalarUpdate,
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_external_lval_update;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires external UPDATE32 source admission before matching proofs"]
fn external_long_fresh_proof_reconstructs_whole_images_without_staged_descriptors() {
    let cases = genuine_external_lval_update::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut proved = 0;
    let mut refused = 0;
    for case in cases {
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
            before: case.before.clone().into(),
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
        let delta = usize::from(case.header == 6) * 17;
        for offset in [
            (95 + delta) * 4096 + 4031,
            (100 + delta) * 4096 + 4087,
            (103 + delta) * 4096 + 4095,
            case.definition as usize * 4096 + 20,
        ] {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
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
            before: case.before.clone().into(),
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
    assert_eq!(proved, 28);
    assert_eq!(refused, 140);
}
