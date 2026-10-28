use tablecraft_testkit::access as support;

use std::{cell::Cell, io::Cursor, path::Path};
use tablecraft_ace::{DependencyCoverage, InspectError, NativeImage, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn complete_vba_fixture_is_retained_and_read_without_executing_bodies() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let mut source = Cursor::new(&bytes);
    let image =
        NativeImage::capture(&mut source, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    assert_eq!(image.as_bytes(), bytes);
    assert_eq!(image.catalog().catalog().objects().len(), 146);
    assert!(
        image
            .dependencies()
            .any(|object| object.coverage() == DependencyCoverage::ModuleExecutionUnavailable)
    );
    assert!(
        image
            .dependencies()
            .any(|object| object.coverage() == DependencyCoverage::ApplicationBehaviorUnavailable)
    );
    assert!(
        image
            .dependencies()
            .any(|object| object.coverage() == DependencyCoverage::Uninspected)
    );
    let table = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), 38);
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn unknown_catalog_and_unused_page_bytes_survive_capture() {
    let mut bytes = support::image(false, false);
    // Independent catalog kind offset: replace the second object's form type
    // with an unknown declaration. The last page is outside the catalog map.
    let start = usize::from(u16::from_le_bytes(
        bytes[4 * 4096 + 16..4 * 4096 + 18].try_into().unwrap(),
    )) & 0x1fff;
    bytes[4 * 4096 + start + 10..4 * 4096 + start + 12].copy_from_slice(&1234_i16.to_le_bytes());
    bytes[6 * 4096..].fill(0xa5);
    let image = NativeImage::capture(
        &mut Cursor::new(&bytes),
        CatalogScope::new(2),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(image.as_bytes(), bytes);
    assert_eq!(
        image.dependencies().nth(1).unwrap().coverage(),
        DependencyCoverage::Uninspected
    );
}

#[test]
fn capture_limits_and_midstream_cancellation_publish_no_image() {
    let bytes = support::image(false, false);
    assert_eq!(
        NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut ResourceBudget::new(100, 100, 100),
            &|| false
        )
        .err(),
        Some(InspectError::Limit(tablecraft_core::limits::LimitError {
            resource: tablecraft_core::limits::Resource::Bytes,
            remaining: 34
        }))
    );
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 4
    };
    assert_eq!(
        NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &cancelled
        )
        .err(),
        Some(InspectError::Cancelled)
    );
}

#[test]
fn missing_payload_metadata_remains_uninspected_and_foreign_ids_fail() {
    use tablecraft_ace::PayloadPresence;
    let image = NativeImage::capture(
        &mut Cursor::new(support::image(false, false)),
        CatalogScope::new(2),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let payloads = image
        .inspect_object_payloads(CatalogScope::new(2).object_id(100), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(
        (payloads.body(), payloads.module(), payloads.extra()),
        (
            PayloadPresence::Uninspected,
            PayloadPresence::Uninspected,
            PayloadPresence::Uninspected
        )
    );
    assert_eq!(
        image
            .inspect_object_payloads(CatalogScope::new(3).object_id(100), &mut budget(), &|| {
                false
            })
            .err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(
        image
            .inspect_object_payloads(CatalogScope::new(2).object_id(100), &mut budget(), &|| true)
            .err(),
        Some(InspectError::Cancelled)
    );
}

#[test]
fn explicit_owned_objects_are_visible_without_claiming_complete_dependency_discovery() {
    let bytes = support::image(false, false);
    let scope = CatalogScope::new(9);
    let image =
        NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false).unwrap();
    let children = image
        .inspect_object_children(scope.object_id(100), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].object().name().as_str(), "Form");
    assert_eq!(
        children[0].coverage(),
        DependencyCoverage::ApplicationBehaviorUnavailable
    );
    assert_eq!(image.as_bytes(), bytes);
    assert_eq!(
        image
            .inspect_object_children(CatalogScope::new(10).object_id(100), &mut budget(), &|| {
                false
            })
            .err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(
        image
            .inspect_object_children(scope.object_id(100), &mut budget(), &|| true)
            .err(),
        Some(InspectError::Cancelled)
    );
    assert!(matches!(
        image.inspect_object_children(
            scope.object_id(100),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
