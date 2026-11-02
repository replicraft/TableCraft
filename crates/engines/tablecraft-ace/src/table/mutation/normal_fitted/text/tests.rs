//! Full-image literal proof and original-carrier/refusal controls.
use super::proof;
use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, SlotScalarUpdate, WriteBlocker,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_text_indexed_update;

const SCOPE: CatalogScope = CatalogScope::new(228);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<genuine_text_indexed_update::Case> {
    genuine_text_indexed_update::load_all(|bytes, expected| {
        let hash = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    })
}

#[test]
#[ignore = "requires all independent Text source images and separately qualified Source"]
fn literal_text_proof_derives_raw_general_keys_and_rejects_protected_forgeries() {
    let mut proven = 0;
    let mut rejected = 0;
    for case in cases() {
        let updates = case
            .updates()
            .into_iter()
            .map(|(row, before, after)| SlotScalarUpdate::new(row, before, after))
            .collect::<Vec<_>>();
        let groups = [FittedColumnUpdates::Variable {
            column: "KeyValue",
            updates: &updates,
        }];
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed_rows(),
        };
        proof::prove(&plan, SCOPE, case.table, &groups, &mut budget(), &|| false).unwrap();
        proven += 1;
        if case.recipe != "existing-family" {
            continue;
        }
        for offset in [
            0,
            case.definition as usize * 4096 + 79,
            case.roots[0] as usize * 4096 + 480,
            case.roots[1] as usize * 4096 + 27,
            case.roots[1] as usize * 4096 + 480,
            case.roots[1] as usize * 4096 + 4095,
            case.roots[2] as usize * 4096 + 480,
            case.data as usize * 4096 + 2,
            case.data as usize * 4096 + 14,
            case.data as usize * 4096 + 4095,
        ] {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed_rows(),
            };
            assert!(matches!(
                proof::prove(&forged, SCOPE, case.table, &groups, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Preservation)
            ));
            rejected += 1;
        }
        let forged = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert!(matches!(
            proof::prove(&forged, SCOPE, case.table, &groups, &mut budget(), &|| {
                false
            }),
            Err(EditError::Preservation)
        ));
        rejected += 1;
    }
    assert_eq!((proven, rejected), (44, 44));
}

#[test]
#[ignore = "requires actual Source and Text binding; eager empty metadata stays closed"]
fn text_binding_rejects_stale_exact_carriers_duplicate_rows_and_unknown_empty_siblings() {
    let mut denied = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "present-noop")
    {
        let image = || {
            NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap()
        };
        let stale = [SlotScalarUpdate::new(
            4,
            Value::Text("A".into()),
            Value::Text("a".into()),
        )];
        assert!(matches!(
            image().plan_ordinary_fitted_updates(
                case.table,
                &[FittedColumnUpdates::Variable {
                    column: "KeyValue",
                    updates: &stale
                }],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Conflict)
        ));
        denied += 1;
        let duplicate = [
            SlotScalarUpdate::new(4, Value::Text("a".into()), Value::Text("A".into())),
            SlotScalarUpdate::new(4, Value::Text("a".into()), Value::Text("A".into())),
        ];
        assert!(matches!(
            image().plan_ordinary_fitted_updates(
                case.table,
                &[FittedColumnUpdates::Variable {
                    column: "KeyValue",
                    updates: &duplicate
                }],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Conflict)
        ));
        denied += 1;
        let groups = [
            FittedColumnUpdates::Variable {
                column: "KeyValue",
                updates: &[],
            },
            FittedColumnUpdates::Variable {
                column: "Missing",
                updates: &[],
            },
        ];
        assert!(matches!(
            image().plan_ordinary_fitted_updates(case.table, &groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        denied += 1;
    }
    assert_eq!(denied, 12);
}
