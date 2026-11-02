//! Direct complete-image proof over independent explicit-property sources.
use crate::{EditError, NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_text_azl as sources;

const SCOPE: CatalogScope = CatalogScope::new(234);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn explicit_azl_literal_proofs_preserve_property_payloads_rows_and_noop_origins() {
    let all = sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut contexts = 0;
    let mut refusals = 0;
    for case in all.into_iter().filter(|case| case.after.is_some()) {
        let requests = case
            .insert_values()
            .into_iter()
            .map(RowInsert::new)
            .collect::<Vec<_>>();
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after_bytes().to_vec(),
            changed_rows: case.changed_rows(),
        };
        super::super::prove(&plan, SCOPE, case.table, &requests, &mut budget(), &|| {
            false
        })
        .unwrap();
        contexts += 1;
        if case.recipe == "empty-insert" {
            assert_eq!(case.after_bytes(), case.before);
        }
        for offset in [
            case.property_name_offset,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 480,
            case.data as usize * 4096 + 4095,
        ] {
            let mut forged = NativeEditPlan {
                before: case.before.clone(),
                after: case.after_bytes().to_vec(),
                changed_rows: case.changed_rows(),
            };
            forged.after[offset] ^= 1;
            assert!(matches!(
                super::super::prove(
                    &forged,
                    SCOPE,
                    case.table,
                    &requests,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            refusals += 1;
        }
    }
    assert_eq!((contexts, refusals), (20, 80));
}
