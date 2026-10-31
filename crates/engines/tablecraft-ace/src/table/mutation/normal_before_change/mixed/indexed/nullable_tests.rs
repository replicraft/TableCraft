//! Compatibility control: nullable index data remains in the unindexed profile.
use super::super::super::{Int32Update, NativeEditPlan};
use crate::{
    EditError, IndexCheckStatus, NativeImage, RecoverableEditError, WriteBlocker, check_table,
    check_table_constraints, read_table, read_table_index,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_nullable_index_before_change as model;

const SCOPE: CatalogScope = CatalogScope::new(104);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn nullable_index_originals_and_literal_images_qualify_before_route_correction() {
    for header in [5, 6] {
        let case = model::source(header);
        for (raw, score) in [(&case.before, 1000), (&case.expected, 1018)] {
            let mut expected = case.rows.clone();
            expected[0][5] = Value::Int32(score);
            let table = read_table(
                &mut Cursor::new(raw),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(table.rows(), expected);
            assert_eq!(table.rows()[2][0], Value::Null);

            let check = check_table(
                &mut Cursor::new(raw),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(check.indexes().len(), 1);
            assert_eq!(check.indexes()[0].status(), IndexCheckStatus::Checked);
            let constraints = check_table_constraints(
                &mut Cursor::new(raw),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), 3);
            let indexed = read_table_index(
                &mut Cursor::new(raw),
                SCOPE,
                "Items",
                "PrimaryId",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                indexed.rows(),
                &[
                    expected[2].clone(),
                    expected[0].clone(),
                    expected[1].clone()
                ]
            );

            // Use the actual original b47 preserved Source admission. This
            // proves the compatibility control belongs to the previous scope.
            let original = image(raw);
            let source =
                super::super::capture(&original, "Items", &mut budget(), &|| false).unwrap();
            assert_eq!(source.table.rows(), expected);
            assert_eq!(original.as_bytes(), raw);
        }
        assert_eq!(&case.before[9 * 4096..], &case.expected[9 * 4096..]);
    }
}

#[test]
fn previous_unindexed_proof_accepts_nullable_index_literal_images() {
    for header in [5, 6] {
        let case = model::source(header);
        let requests = [Int32Update::new(0, 1000, 17)];
        let plan = NativeEditPlan {
            before: case.before,
            after: case.expected,
            changed_rows: 1,
        };
        super::super::proof::prove(
            &plan,
            SCOPE,
            "Items",
            "Score",
            &requests,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
fn normal_route_preserves_previously_admitted_nullable_index_updates() {
    for header in [5, 6] {
        let case = model::source(header);
        let original = image(&case.before);
        original
            .check_native_before_change_int32_table("Items", &mut budget(), &|| false)
            .unwrap();
        let plan = original
            .plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 1000, 17)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.expected);
        assert_eq!(plan.changed_rows(), 1);
    }
}

#[test]
fn nullable_key_classification_never_grants_indexed_target_permission() {
    for header in [5, 6] {
        let case = model::source(header);
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Id",
                vec![Int32Update::new(0, 1, 7)],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::IndexedColumn
            )))
        ));

        let plan = NativeEditPlan {
            before: case.before,
            after: case.expected,
            changed_rows: 1,
        };
        assert!(matches!(
            super::prove(
                &plan,
                SCOPE,
                "Items",
                "Score",
                &[Int32Update::new(0, 1000, 17)],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
    }
}
