//! Actual complete preservation SOURCE admission, with no staging or writer.
#[path = "../../../../../../tests/ace16_preserved_scalar_updates_contract/int32_sources/nullable_sources/model.rs"]
pub(super) mod model;

use super::super::super::source;
use super::{budget, capture};

#[test]
fn qualify_preserved_nullable_int32_all_derivatives_through_actual_source_capture() {
    let mut originals = 0;
    let mut expected_images = 0;
    for name in model::CASES {
        let case = model::source(name);
        let image = capture(&case.before);
        let qualified = source::capture(&image, "Items", &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("Original {name}: {error}"));
        assert_eq!(qualified.records.len(), 3);
        assert_eq!(image.as_bytes(), case.before);
        assert_eq!(
            case.requested_rows,
            if name == "mixed_rows" { 3 } else { 1 }
        );
        assert_eq!(
            case.initial[0].is_none(),
            name.starts_with("null_") || name == "mixed_rows"
        );
        assert_eq!(case.required, name == "required_refusal");
        originals += 1;
        if let Some(expected) = &case.expected {
            let candidate_source = capture(expected);
            let qualified = source::capture(&candidate_source, "Items", &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("Expected source {name}: {error}"));
            assert_eq!(qualified.records.len(), 3);
            assert_eq!(candidate_source.as_bytes(), expected);
            assert!(!case.required);
            assert!(case.replacement.iter().any(Option::is_some));
            expected_images += 1;
        } else {
            assert!(case.required);
            assert!(case.initial.iter().all(Option::is_some));
        }
    }
    assert_eq!((originals, expected_images), (7, 6));
}
