//! Literal whole-image proof inputs, independently qualified before planners.
use super::super::proof;
use crate::{EditError, NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_compound_root_split;

const SCOPE: CatalogScope = CatalogScope::new(224);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(226),
        Value::Binary(
            (0..84)
                .map(|position| (position * 31 + 226) as u8)
                .collect(),
        ),
    ])]
}

#[test]
#[ignore = "requires four full84 views through independent read/Source gates"]
fn literal_compound_direct_proof_rejects_data_index_map_history_and_cardinality_forgeries() {
    let cases = genuine_compound_root_split::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    });
    let mut accepted = 0;
    let mut rejected = 0;
    for case in cases {
        let plan = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        proof::prove(
            &plan,
            SCOPE,
            "BinaryStorage",
            &requests(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        accepted += 1;
        for at in [
            0,
            case.definition * 4096 + 16,
            case.definition * 4096 + 67,
            case.root * 4096 + 489,
            case.root * 4096 + 493,
            case.data * 4096 + 23,
            case.eof * 4096 + 4,
            case.eof * 4096 + 14,
            case.eof * 4096 + 3999,
            case.eof * 4096 + 4095,
            (case.eof + 1) * 4096 + 27,
            (case.eof + 1) * 4096 + 480,
            (case.eof + 2) * 4096 + 3000,
            (case.definition + 1) * 4096 + 4000,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: 1,
            };
            assert!(matches!(
                proof::prove(
                    &forged,
                    SCOPE,
                    "BinaryStorage",
                    &requests(),
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            rejected += 1;
        }
        let forged = NativeEditPlan {
            before: case.before.into(),
            after: case.after,
            changed_rows: 0,
        };
        assert!(matches!(
            proof::prove(
                &forged,
                SCOPE,
                "BinaryStorage",
                &requests(),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
        rejected += 1;
    }
    assert_eq!((accepted, rejected), (2, 30));
}
