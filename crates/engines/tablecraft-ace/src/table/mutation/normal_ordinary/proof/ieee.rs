//! Literal plans enter the independent IEEE proof without producer preparation.
use super::prove_ieee;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ieee_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires six independently qualified existing IEEE images"]
fn ieee_literal_pairs_and_protected_bit_forgeries_have_independent_proof() {
    let cases = source::load_all(|raw, expected| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut counts = [0; 3];
    for case in cases {
        let groups = case.groups();
        let updates: Vec<_> = groups
            .iter()
            .map(|group| {
                vec![FixedScalarUpdate::new(
                    2,
                    group.expected.clone(),
                    group.replacement.clone(),
                )]
            })
            .collect();
        let original: Vec<_> = groups
            .iter()
            .zip(&updates)
            .map(|(group, updates)| FixedColumnUpdates::new("Items", group.column, updates))
            .collect();
        let scope = CatalogScope::new(291);
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        prove_ieee(&plan, scope, "Items", &original, &mut budget(), &|| false).unwrap();
        counts[0] += 1;
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert_eq!(
                prove_ieee(&forged, scope, "Items", &original, &mut budget(), &|| false),
                Err(EditError::Preservation)
            );
            counts[1] += 1;
        }
        let count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 2,
        };
        assert_eq!(
            prove_ieee(&count, scope, "Items", &original, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        counts[2] += 1;
    }
    assert_eq!(counts, [4, 40, 4]);
}
