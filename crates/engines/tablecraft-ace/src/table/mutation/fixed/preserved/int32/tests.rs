//! Reuse the existing loaded rich owner and the exact public SOURCE derivative.
use super::super::larger_capture_tests::preserved_model as rich;
#[path = "../../../../../../tests/ace16_preserved_scalar_updates_contract/int32_sources/model.rs"]
mod model;

use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn preserved_int32_authority_rejects_correlated_alternative_valid_candidate() {
    assert!(model::CASES.contains(&(6, -101)));
    let original = model::source(6, -101);
    assert_eq!(original.value, -101);
    let alternative = model::source(6, 101);
    let requested = [FixedScalarUpdate::new(
        0,
        Value::Int32(1000),
        Value::Int32(-101),
    )];
    let groups = [FixedColumnUpdates::new("Items", "Large_Number", &requested)];
    let healthy = capture(&original.before)
        .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), original.expected);
    let source = capture(&original.before);
    super::proof::prove(
        &source,
        &groups,
        healthy.candidate_bytes(),
        1,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let changed = [FixedScalarUpdate::new(
        0,
        Value::Int32(1000),
        Value::Int32(101),
    )];
    let valid_alternative = capture(&original.before)
        .preview_ace16_fixed_updates("Items", "Large_Number", &changed, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(valid_alternative.candidate_bytes(), alternative.expected);
    assert_eq!(
        super::proof::prove(
            &source,
            &groups,
            valid_alternative.candidate_bytes(),
            1,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
}

#[test]
fn preserved_int32_authority_rejects_padding_opaque_bytes_and_wrong_count() {
    let case = model::source(6, -101);
    let requests = [FixedScalarUpdate::new(
        0,
        Value::Int32(1000),
        Value::Int32(-101),
    )];
    let groups = [FixedColumnUpdates::new("Items", "Large_Number", &requests)];
    let source = capture(&case.before);
    super::proof::prove(&source, &groups, &case.expected, 1, &mut budget(), &|| {
        false
    })
    .unwrap();
    for offset in [case.patch + 4, 31 * 4096] {
        let mut forged = case.expected.clone();
        forged[offset] ^= 1;
        assert_eq!(
            super::proof::prove(&source, &groups, &forged, 1, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
    }
    assert_eq!(
        super::proof::prove(&source, &groups, &case.expected, 0, &mut budget(), &|| {
            false
        }),
        Err(EditError::Preservation)
    );
}
