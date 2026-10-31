//! Strong read-only admission; no scalar mutation producer is called.
use super::{larger_capture_tests::preserved_model as rich, source};
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/fixed_scalar_sources/model.rs"]
pub(super) mod model;

use crate::{EditError, NativeImage, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

mod genuine;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_preserved_eight_byte_carrier_images_with_existing_source_owner() {
    let mut admitted = 0;
    let mut refused = 0;
    let mut required_sources = 0;
    for code in [5, 6] {
        for (target, recipe) in model::cases() {
            let case = model::source(code, target, recipe);
            model::assert_complement(&case);
            let views = std::iter::once((&case.before, case.initial)).chain(
                case.expected
                    .iter()
                    .map(|expected| (expected, case.replacement)),
            );
            for (bytes, values) in views {
                let image = NativeImage::capture(
                    &mut Cursor::new(bytes),
                    CatalogScope::new(1),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = source::capture(&image, "Items", &mut budget(), &|| false);
                if code == 5 {
                    assert_eq!(
                        result.err(),
                        Some(EditError::Unsupported(WriteBlocker::Column))
                    );
                    refused += 1;
                } else {
                    let original = result.unwrap();
                    assert_eq!(original.records.len(), 3);
                    let selected = original
                        .table
                        .columns()
                        .iter()
                        .position(|field| field.name() == target.name())
                        .unwrap();
                    let field = &original.table.schema().columns()[target.number()];
                    assert_eq!(field.native_type(), target.native());
                    assert_eq!(field.storage_length(), 8);
                    assert!(field.fixed());
                    assert!(!original.dependencies.contains(&target.number()));
                    assert!(original.indexes.indexes().iter().all(|index| {
                        index
                            .columns()
                            .iter()
                            .all(|key| key.number() != target.number())
                    }));
                    for (row, expected) in original.table.rows().iter().zip(values) {
                        assert_eq!(row[selected], target.value(expected));
                    }
                    admitted += 1;
                }
                assert_eq!(image.as_bytes(), bytes);
            }
            if recipe == model::Recipe::RequiredRefusal {
                assert_eq!(case.before[case.required_patch.unwrap()], 1);
                assert!(case.expected.is_none());
                required_sources += 1;
            }
        }
    }
    assert_eq!((admitted, refused, required_sources), (74, 74, 8));
}
