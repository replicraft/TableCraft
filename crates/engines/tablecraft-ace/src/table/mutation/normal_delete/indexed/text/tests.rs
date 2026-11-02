//! Direct literal proof acceptance, never a staged or producer-derived plan.
use crate::{EditError, NativeEditPlan, RowDelete};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_text_cardinality as sources;

const SCOPE: CatalogScope = CatalogScope::new(229);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn text_delete_proof_accepts_only_literal_complete_images_and_protects_causal_bytes() {
    let mut literals = 0;
    let mut denials = 0;
    for case in cases().into_iter().filter(|case| case.action == "delete") {
        let requests = case
            .witnesses()
            .into_iter()
            .map(|(ordinal, raw)| RowDelete::new(ordinal, raw))
            .collect::<Vec<_>>();
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed_rows(),
        };
        super::super::super::proof::prove(
            &plan,
            SCOPE,
            case.table,
            &requests,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        literals += 1;
        if case.recipe != "delete-case-pair" {
            continue;
        }
        for offset in [
            0,
            case.definition as usize * 4096 + 16,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 4095,
            case.data as usize * 4096 + 4095,
        ] {
            let mut forged = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed_rows(),
            };
            forged.after[offset] ^= 1;
            assert!(matches!(
                super::super::super::proof::prove(
                    &forged,
                    SCOPE,
                    case.table,
                    &requests,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            denials += 1;
        }
        let forged = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed_rows() + 1,
        };
        assert!(matches!(
            super::super::super::proof::prove(
                &forged,
                SCOPE,
                case.table,
                &requests,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
        denials += 1;
    }
    assert_eq!(literals, 36);
    assert_eq!(denials, 28);
}
