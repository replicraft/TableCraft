//! Literal source-pair acceptance; source gate must precede these producers.
use super::proof;
use crate::{
    EditError, NativeCompleteInsertIntent, NativeEditPlan, NativeImage, RecoverableEditError,
    RowInsert, WriteBlocker,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_root_split::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(220);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(226),
        Value::Binary(Vec::new()),
    ])]
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| assert_eq!(hash(raw), expected))
}

#[test]
#[ignore = "requires four root-split source views qualified before normal producers"]
fn complete_requests_match_both_literal_split_images_and_keep_original_tcna4_data() {
    let cases = cases();
    for case in cases {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "BinaryStorage",
                requests(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(plan.intent().table(), "BinaryStorage");
        assert_eq!(plan.intent().rows().len(), 1);
        assert_eq!(plan.intent().rows()[0].values(), requests()[0].values());
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&wire[..8], b"TCNA0004");
        let decoded = NativeCompleteInsertIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        decoded
            .verify_before(&case.before, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        // The old explicit fixed-length API still has no root-split contract.
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_inserts(
                "BinaryStorage",
                requests(),
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::IndexedColumn
            )))
        ));
    }
}

#[test]
#[ignore = "requires four independent root-split images; no planner supplies proof inputs"]
fn literal_direct_proof_rejects_wrong_children_maps_history_tails_records_and_geometry() {
    let cases = cases();
    for case in cases {
        let plan = NativeEditPlan {
            before: case.before.clone(),
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
        for at in [
            0,
            case.root * 4096 + 485,
            case.root * 4096 + 489,
            case.root * 4096 + 493,
            case.eof * 4096 + 4095,
            (case.eof + 1) * 4096 + 4095,
            case.definition * 4096 + 67,
            case.data * 4096 + 4095,
            4096 + 4032 + case.eof / 8,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert!(
                proof::prove(
                    &forged,
                    SCOPE,
                    "BinaryStorage",
                    &requests(),
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        for (length, count) in [
            (case.after.len(), 0),
            (case.after.len() - 1, 1),
            (case.after.len() + 4096, 1),
        ] {
            let mut after = case.after.clone();
            after.resize(length, 0);
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: count,
            };
            assert!(
                proof::prove(
                    &forged,
                    SCOPE,
                    "BinaryStorage",
                    &requests(),
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
    }
}
