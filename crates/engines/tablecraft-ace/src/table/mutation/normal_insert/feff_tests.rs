//! Literal source images qualify manual proof before producer comparisons.
use super::proof;
use crate::{EditError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker, read_table};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_ordinary_insert_feff::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(160);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(sources::values())]
}

#[test]
#[ignore = "requires six independently qualified FEFF source images"]
fn literal_escaped_images_pass_independent_proof_and_then_match_complete_plans() {
    for case in cases() {
        let literal = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        proof::prove(
            &literal,
            SCOPE,
            "Sentinel",
            &requests(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &requests(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);
    }
}

#[test]
#[ignore = "requires six independently qualified FEFF source images"]
fn readable_unescaped_alias_cannot_satisfy_the_original_feff_insert_request() {
    for case in cases() {
        let table = read_table(
            &mut Cursor::new(&case.alias),
            SCOPE,
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows()[1],
            vec![Value::Int32(24), Value::Text("new".into()), Value::Null]
        );
        let forged = NativeEditPlan {
            before: case.before,
            after: case.alias,
            changed_rows: 1,
        };
        assert!(matches!(
            proof::prove(
                &forged,
                SCOPE,
                "Sentinel",
                &requests(),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
    }
}

#[test]
#[ignore = "requires six independently qualified FEFF source images"]
fn wide_boundary_zero_remains_a_precise_unsupported_representation() {
    for case in cases() {
        for text in ["\u{feff}\u{0100}", "\u{feff}\0"] {
            let values = vec![Value::Int32(24), Value::Text(text.into()), Value::Null];
            let error = image(&case.before)
                .plan_ordinary_row_inserts(
                    "Sentinel",
                    &[RowInsert::new(values)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
}
