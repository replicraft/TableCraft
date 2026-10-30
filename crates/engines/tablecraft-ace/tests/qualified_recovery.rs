#![cfg(feature = "journal")]

#[path = "qualified_recovery/model.rs"]
mod model;

#[path = "qualified_recovery/api.rs"]
mod api;

use model::{Marker, Shape};
use std::io::Cursor;
use tablecraft_ace::{
    FileFormat, IndexCheckStatus, NativeImage, check_table, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn qualify_jet4_recovery_literal_sources_before_recovery() {
    for shape in model::SHAPES {
        let case = model::case(shape);
        assert_eq!(
            &case.before[case.changed_span.clone()],
            &10_i32.to_le_bytes()
        );
        assert_eq!(
            &case.after[case.changed_span.clone()],
            &case.value.to_le_bytes()
        );
        for (bytes, score) in [(&case.before, 10), (&case.after, case.value)] {
            let image = NativeImage::capture(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(image.catalog().header().format(), FileFormat::Jet4);
            let checked = check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 3, "{shape:?}");
            assert_eq!(checked.indexes().len(), case.indexes);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let rows = read_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows(),
                &[
                    vec![Value::Int32(1), Value::Int32(score)],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)],
                ]
            );
            for name in ["ScoreKey", "ScoreAlias"].into_iter().take(case.indexes) {
                let indexed = read_table_index(
                    &mut image.reader(),
                    CatalogScope::new(1),
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let expected = if case.descending {
                    [30, 20, score]
                } else {
                    [score, 20, 30]
                };
                let actual = indexed
                    .rows()
                    .iter()
                    .map(|row| row[1].clone())
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual,
                    expected.into_iter().map(Value::Int32).collect::<Vec<_>>()
                );
            }
        }
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let journal = model::journal(&case.before, &case.after, marker);
            assert_eq!(&journal[..8], b"TCIMG001");
            assert_eq!(
                u64::from_le_bytes(journal[8..16].try_into().unwrap()),
                case.before.len() as u64
            );
            assert_eq!(&journal[48..48 + case.before.len()], &case.before);
            assert_eq!(
                &journal[48 + case.before.len()..48 + 2 * case.before.len()],
                &case.after
            );
        }
    }
}

#[test]
fn qualify_jet4_recovery_refusal_sources_before_recovery() {
    for code in [5, 6] {
        let bytes = model::real_ace(code);
        let image = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            image.catalog().header().format(),
            if code == 5 {
                FileFormat::Ace2016
            } else {
                FileFormat::Ace2019
            }
        );
        assert_eq!(
            check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .rows(),
            3
        );
    }
    let unknown = model::unknown_property();
    let mut forged = model::case(Shape::Plain);
    forged.after[13 * 4096] ^= 1;
    for bytes in [
        &unknown.before,
        &unknown.after,
        &forged.before,
        &forged.after,
    ] {
        let image = NativeImage::capture(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .rows(),
            3
        );
    }
}
