use super::{CapturePolicy, FixedUpdateImageLimits, NativeImage, NativeImageSource};
use crate::InspectError;
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

const SCOPE: CatalogScope = CatalogScope::new(761);
const BYTES: u64 = 32 * 1024 * 1024;
const ROWS: u64 = 100_000;
const STEPS: u64 = 1_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, ROWS, STEPS)
}

fn original() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    )
}

#[test]
fn owned_capture_and_repeated_reviews_share_whole_original_allocation() {
    let bytes = original();
    let pointer = bytes.as_ptr();
    let source = NativeImageSource::from_vec(bytes, &mut budget(), &|| false).unwrap();
    let first =
        NativeImage::capture_shared(source.clone(), SCOPE, &mut budget(), &|| false).unwrap();
    let second =
        NativeImage::capture_shared(source.clone(), SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(first.as_bytes().as_ptr(), pointer);
    assert_eq!(second.as_bytes().as_ptr(), pointer);
    assert_eq!(first.as_bytes(), original());
    assert_eq!(first.catalog().header(), second.catalog().header());

    // A read cursor cannot obtain mutable ownership while these views exist.
    assert!(source.shared_bytes().try_into_mut().is_err());
    let mut independent = source.as_bytes().to_vec();
    independent[0] ^= 1;
    assert_ne!(independent[0], first.as_bytes()[0]);
}

#[test]
fn streamed_capture_remains_isolated_from_subsequent_mutable_source_changes() {
    let mut reader = Cursor::new(original());
    let image = NativeImage::capture(&mut reader, SCOPE, &mut budget(), &|| false).unwrap();
    assert_ne!(image.as_bytes().as_ptr(), reader.get_ref().as_ptr());
    reader.get_mut()[0] ^= 1;
    assert_eq!(image.as_bytes(), original());
}

#[test]
fn shared_capture_charges_unused_capacity_and_exact_inspection_budgets() {
    let mut bytes = original();
    bytes.reserve_exact(bytes.len());
    let capacity = bytes.capacity();
    assert!(capacity > bytes.len());
    let source = NativeImageSource::from_vec(bytes, &mut budget(), &|| false).unwrap();
    assert_eq!(source.retained_capacity(), capacity);
    let mut short = ResourceBudget::new(capacity as u64 - 1, ROWS, STEPS);
    assert!(matches!(
        NativeImage::capture_shared(source.clone(), SCOPE, &mut short, &|| false),
        Err(InspectError::Limit(_))
    ));

    let mut measured = budget();
    NativeImage::capture_shared(source.clone(), SCOPE, &mut measured, &|| false).unwrap();
    for (resource, available) in [
        (Resource::Bytes, BYTES),
        (Resource::Rows, ROWS),
        (Resource::Steps, STEPS),
    ] {
        let needed = available - measured.remaining(resource);
        assert!(needed > 0);
        for (quota, admitted) in [(needed, true), (needed - 1, false)] {
            let mut limits = match resource {
                Resource::Bytes => ResourceBudget::new(quota, ROWS, STEPS),
                Resource::Rows => ResourceBudget::new(BYTES, quota, STEPS),
                Resource::Steps => ResourceBudget::new(BYTES, ROWS, quota),
            };
            let result = NativeImage::capture_shared(source.clone(), SCOPE, &mut limits, &|| false);
            assert_eq!(result.is_ok(), admitted);
        }
    }
}

#[test]
fn ownership_and_shared_capture_check_cancellation_and_keep_fixed_purpose() {
    assert!(matches!(
        NativeImageSource::from_vec(original(), &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let source = NativeImageSource::from_vec(original(), &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeImage::capture_shared(source.clone(), SCOPE, &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let fixed = NativeImage::capture_shared_for_fixed_updates(
        source,
        SCOPE,
        FixedUpdateImageLimits::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(fixed.capture, CapturePolicy::FixedUpdate(_)));
    assert!(matches!(
        fixed.require_standard_capture(),
        Err(InspectError::StructureLimit)
    ));
}
