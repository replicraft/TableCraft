//! Strongest original/candidate admission, without any growing producer calls.
use super::{larger_capture_tests::preserved_model as rich, source};
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/growing_sources/model.rs"]
pub(super) mod model;

use crate::{EditError, NativeImage, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn qualify_preserved_text_growth_images_with_existing_source_authority() {
    let mut admitted = 0;
    let mut refused = 0;
    for shape in model::SHAPES {
        let case = model::source(shape);
        model::assert_placement(&case, shape);
        for (bytes, values) in [
            (&case.before, [Some("alpha"), None, Some("omega")]),
            (&case.expected, case.values),
        ] {
            for code in [5, 6] {
                let mut bytes = bytes.clone();
                bytes[20] = code;
                let image = NativeImage::capture(
                    &mut Cursor::new(&bytes),
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
                    let position = original
                        .table
                        .columns()
                        .iter()
                        .position(|field| field.name() == "Short_Text")
                        .unwrap();
                    for (row, text) in original.table.rows().iter().zip(values) {
                        assert_eq!(
                            row[position],
                            text.map_or(Value::Null, |text| Value::Text(text.into()))
                        );
                    }
                    admitted += 1;
                }
                assert_eq!(image.as_bytes(), bytes);
            }
        }
    }
    assert_eq!((admitted, refused), (16, 16));
}

#[test]
fn qualify_preserved_text_growth_authority_contradictions_before_writer() {
    for kind in [
        model::Contradiction::TerminalAlias,
        model::Contradiction::IndexOwnsParentData,
        model::Contradiction::GlobalSaysParentFree,
    ] {
        let bytes = model::contradiction(kind);
        let image = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            source::capture(&image, "Items", &mut budget(), &|| false).is_err(),
            "{kind:?}"
        );
        assert_eq!(image.as_bytes(), bytes);
    }
}
