//! Complete original Source admission, without any mutation call.
use super::super::{joint_fixed_sources::model as fixed_model, source};
use super::{model as text_model, zero_length_source};
#[path = "../../../../../../tests/ace16_preserved_scalar_updates_contract/short_text_sources/joint_fixed_sources/model.rs"]
pub(super) mod model;

use crate::{EditError, NativeImage, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_joint_text_fixed_source_roles_before_writer() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            model::assert_protected(&case);
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
                    let text = original
                        .table
                        .columns()
                        .iter()
                        .position(|field| field.name() == "Short_Text")
                        .unwrap();
                    for row in 0..3 {
                        assert_eq!(
                            original.table.rows()[row][text],
                            model::read_text(bytes, row).map_or(Value::Null, Value::Text)
                        );
                    }
                    for column in fixed_model::columns(case.flavor) {
                        let position = original
                            .table
                            .columns()
                            .iter()
                            .position(|field| field.name() == column.name())
                            .unwrap();
                        for row in 0..3 {
                            assert_eq!(
                                original.table.rows()[row][position],
                                fixed_model::read(bytes, column, row).value()
                            );
                        }
                    }
                    admitted += 1;
                }
                assert_eq!(image.as_bytes(), bytes);
            }
        }
    }
    assert_eq!((admitted, refused), (45, 45));
}
