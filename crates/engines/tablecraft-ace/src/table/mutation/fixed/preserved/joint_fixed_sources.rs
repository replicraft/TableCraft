//! Complete original preservation admission, independent of any joint writer.
use super::{
    fixed_scalar_sources::model as fixed_scalar_model, int32::tests::model as int32_model,
    larger_capture_tests::preserved_model as rich, source,
};
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/joint_fixed_sources/model.rs"]
pub(super) mod model;

use crate::{EditError, NativeImage, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_joint_fixed_source_roles_before_producer_installation() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            model::assert_complement(&case);
            if let Some(patch) = case.required_patch {
                assert_eq!(case.before[patch], 1);
                for row in 0..3 {
                    assert!(matches!(
                        model::read(&case.before, model::Column::Currency, row),
                        model::Cell::Currency(_)
                    ));
                }
            }
            for bytes in std::iter::once(&case.before).chain(case.expected.iter()) {
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
                    for column in model::columns(case.flavor) {
                        let field = &original.table.schema().columns()[column.number()];
                        assert_eq!(field.name(), column.name());
                        assert_eq!(field.native_type(), column.native());
                        assert_eq!(field.storage_length(), column.width());
                        let selected = original
                            .table
                            .columns()
                            .iter()
                            .position(|field| field.name() == column.name())
                            .unwrap();
                        for (ordinal, row) in original.table.rows().iter().enumerate() {
                            assert_eq!(row[selected], model::read(bytes, column, ordinal).value());
                        }
                        let indexed = original.indexes.indexes().iter().any(|index| {
                            index
                                .columns()
                                .iter()
                                .any(|key| key.number() == column.number())
                        });
                        assert_eq!(indexed, column == model::Column::IndexedNumber);
                        assert_eq!(
                            original.dependencies.contains(&column.number()),
                            column == model::Column::IndexedNumber
                        );
                    }
                    admitted += 1;
                }
                assert_eq!(image.as_bytes(), bytes);
            }
        }
    }
    assert_eq!((admitted, refused), (46, 46));
}

mod native_expected;
