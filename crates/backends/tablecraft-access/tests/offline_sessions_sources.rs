//! Qualify independently authored complete images before the session runs.
#[path = "offline_sessions/model.rs"]
mod model;

use std::io::Cursor;
use tablecraft_ace::{FileFormat, NativeImage, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn original_and_two_successive_images_are_complete_readable_native_sources() {
    let original = model::original();
    for (bytes, score) in [
        (original.clone(), 10),
        (model::with_score(12), 12),
        (model::with_score(17), 17),
    ] {
        assert_eq!(bytes.len(), original.len());
        let offset = 12 * 4096 + 4091;
        assert_eq!(&bytes[..offset], &original[..offset]);
        assert_eq!(&bytes[offset + 4..], &original[offset + 4..]);

        let image = NativeImage::capture(
            &mut Cursor::new(&bytes),
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
        assert_eq!(checked.rows(), 3);
        assert!(checked.indexes().is_empty());

        let table = read_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(score)],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ]
        );
    }
}
