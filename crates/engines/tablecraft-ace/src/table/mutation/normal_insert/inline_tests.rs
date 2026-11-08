//! Literal complete-image proof with no planner, placement or producer encoder.
use super::proof;
use crate::{NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_inline_lval_insert as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL views"]
fn all_literal_inline_images_prove_without_a_writer_and_reject_causal_forgeries() {
    let cases = sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    });
    for case in cases {
        let requests = vec![RowInsert::new(sources::values(case.recipe))];
        let plan = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        let scope = CatalogScope::new(194);
        proof::prove(
            &plan,
            scope,
            "InlineValues",
            &requests,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let data = case.data as usize * 4096;
        let definition = case.definition as usize * 4096;
        let start = usize::from(u16::from_le_bytes(
            case.after[data + 16..data + 18].try_into().unwrap(),
        ));
        for at in [
            data + start + 6,
            data + 4048,
            data + 4095,
            data + 18,
            definition + 16,
            definition + 184,
            24,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let bad = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: 1,
            };
            assert!(
                proof::prove(
                    &bad,
                    scope,
                    "InlineValues",
                    &requests,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        let count = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert!(
            proof::prove(
                &count,
                scope,
                "InlineValues",
                &requests,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        let mut wrong = sources::values(case.recipe);
        wrong[0] = tablecraft_core::values::Value::Int32(3);
        assert!(
            proof::prove(
                &plan,
                scope,
                "InlineValues",
                &[RowInsert::new(wrong)],
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}
