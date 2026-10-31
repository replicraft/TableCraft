//! Direct read-only admission of the immutable rich Short Text source corpus.
use super::{larger_capture_tests::preserved_model as rich, source};
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/short_text_sources/model.rs"]
pub(super) mod model;
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/short_text_sources/zero_length_source.rs"]
pub(super) mod zero_length_source;

use crate::{EditError, NativeImage, WriteBlocker, inspect_table_properties};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn qualify_preserved_short_text_complete_images_with_original_source_capture() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            assert_eq!(case.admitted_source, code == 6);
            assert_eq!(case.framing.count, 16);
            assert_eq!(case.framing.present, case.original.is_some());
            if let Some(payload) = case.payload {
                assert!(payload.len() <= case.framing.capacity);
            }
            let views = std::iter::once((&case.before, case.original)).chain(
                case.expected
                    .iter()
                    .map(|expected| (expected, case.replacement)),
            );
            for (bytes, expected_value) in views {
                let image = NativeImage::capture(
                    &mut Cursor::new(bytes),
                    CatalogScope::new(1),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let captured = source::capture(&image, "Items", &mut budget(), &|| false);
                if code == 5 {
                    // Date/Time Extended declarations remain unavailable under
                    // header 5. Basic selected projections do not grant the
                    // complete rich preservation profile.
                    assert_eq!(
                        captured.err(),
                        Some(EditError::Unsupported(WriteBlocker::Column))
                    );
                    refused += 1;
                } else {
                    let captured = captured.unwrap();
                    let position = captured
                        .table
                        .columns()
                        .iter()
                        .position(|column| column.name() == "Short_Text")
                        .unwrap();
                    assert_eq!(
                        captured.table.rows()[case.framing.row][position],
                        expected_value.map_or(Value::Null, |text| Value::Text(text.to_owned()))
                    );
                    assert_eq!(captured.records.len(), 3);
                    admitted += 1;
                }
                assert_eq!(image.as_bytes(), bytes);
            }
        }
    }
    assert_eq!((admitted, refused), (17, 17));
}

#[test]
fn qualify_preserved_short_text_zero_length_prohibition_as_healthy_original() {
    let case = model::source(6, model::Recipe::Empty);
    let scope = CatalogScope::new(1);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        scope,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let (before, patch) = zero_length_source::disallow_empty(&case.before, properties.raw_bytes());
    let image =
        NativeImage::capture(&mut Cursor::new(&before), scope, &mut budget(), &|| false).unwrap();
    let captured = source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(captured.records.len(), 3);
    assert_eq!(before[patch], 0);
    assert_eq!(image.as_bytes(), before);
}

#[path = "short_text_sources/joint_fixed_sources.rs"]
pub(super) mod joint_fixed_sources;
