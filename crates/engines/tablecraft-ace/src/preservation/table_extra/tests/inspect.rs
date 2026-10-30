#[path = "../../../../tests/object_payloads/fixture.rs"]
pub(super) mod payload_fixture;

use super::{budget, fixture};
use crate::{
    InspectError, NativeImage, PayloadPresence, preservation::table_extra::TableExtraCoverage,
    test_support::base,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn local_database(extra: Option<Vec<u8>>, duplicate_name: bool) -> Vec<u8> {
    // Unreadable body/module definitions prove the Extra-only projection.
    let mut bytes = payload_fixture::database(
        6,
        &[
            Some(vec![1]),
            Some(vec![2]),
            extra.map(|value| payload_fixture::inline(&value)),
        ],
    );
    let page = &bytes[4 * 4096..5 * 4096];
    let start = usize::from(u16::from_le_bytes([page[18], page[19]])) & 0x0fff;
    let end = usize::from(u16::from_le_bytes([page[16], page[17]])) & 0x0fff;
    let mut selected = page[start..end].to_vec();
    support::put16(&mut selected, 10, 1);
    support::put32(&mut selected, 6, 42);
    let mut records = vec![
        (
            0,
            support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        ),
        (
            0,
            support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
        (0, selected.clone()),
    ];
    if duplicate_name {
        // Same name, distinct original native ID/parent/source slot and bytes.
        support::put32(&mut selected, 2, 11);
        support::put32(&mut selected, 6, 43);
        records.push((0, selected));
    }
    bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(2, &records));
    support::put32(&mut bytes, 2 * 4096 + 16, records.len() as u32);
    bytes
}

#[test]
fn exact_scoped_identity_retains_native_parent_and_catalog_slot() {
    let raw = fixture::golden();
    let bytes = local_database(Some(raw.clone()), true);
    let image = capture(&bytes);
    for (native_id, parent_id, slot) in [(10, 42, 2), (11, 43, 3)] {
        let object = CatalogScope::new(7).object_id(native_id);
        let result = image
            .inspect_table_extra(object, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(result.object_id(), object);
        assert_eq!(result.native_id(), native_id as i32);
        assert_eq!(result.parent_id(), parent_id);
        assert_eq!(result.catalog_page(), 4);
        assert_eq!(result.catalog_slot(), slot);
        assert_eq!(result.presence(), PayloadPresence::Present);
        assert_eq!(result.raw(), Some(raw.as_slice()));
        assert_eq!(result.coverage(), TableExtraCoverage::WideMr2);
    }
    assert_eq!(
        image
            .inspect_table_extra(CatalogScope::new(8).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(
        image
            .inspect_table_extra(CatalogScope::new(7).object_id(999), &mut budget(), &|| {
                false
            })
            .err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn missing_null_empty_opaque_and_malformed_payloads_do_not_collapse() {
    let missing = capture(&support::image(false, false));
    let result = missing
        .inspect_table_extra(CatalogScope::new(7).object_id(100), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(result.presence(), PayloadPresence::Uninspected);
    assert_eq!(result.coverage(), TableExtraCoverage::NotPresent);
    assert_eq!(result.raw(), None);

    for (value, presence, coverage) in [
        (
            None,
            PayloadPresence::Absent,
            TableExtraCoverage::NotPresent,
        ),
        (
            Some(vec![]),
            PayloadPresence::Present,
            TableExtraCoverage::Empty,
        ),
        (
            Some(b"future".to_vec()),
            PayloadPresence::Present,
            TableExtraCoverage::Opaque,
        ),
    ] {
        let bytes = local_database(value.clone(), false);
        let result = capture(&bytes)
            .inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(result.presence(), presence);
        assert_eq!(result.coverage(), coverage);
        assert_eq!(result.raw(), value.as_deref());
    }
    let malformed = local_database(Some(b"MR2\0\x05\0\0\0\x80\0".to_vec()), false);
    assert_eq!(
        capture(&malformed)
            .inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Malformed)
    );
    let nonlocal = payload_fixture::database(
        6,
        &[
            None,
            None,
            Some(payload_fixture::inline(&fixture::golden())),
        ],
    );
    assert_eq!(
        capture(&nonlocal)
            .inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Unsupported)
    );
}

#[test]
fn selected_payload_read_is_bounded_and_late_cancel_returns_no_result() {
    let bytes = local_database(Some(fixture::golden()), false);
    let image = capture(&bytes);
    let object = CatalogScope::new(7).object_id(10);
    let mut measured = budget();
    image
        .inspect_table_extra(object, &mut measured, &|| false)
        .unwrap();
    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        let consumed = budget().remaining(resource) - measured.remaining(resource);
        assert!(consumed > 0);
        let mut limits = [32 * 1024 * 1024, 16_384, 1_000_000];
        limits[position] = consumed - 1;
        assert!(matches!(
            image.inspect_table_extra(object, &mut ResourceBudget::new(limits[0], limits[1], limits[2]), &|| false),
            Err(InspectError::Limit(error)) if error.resource == resource
        ));
    }
    let calls = Cell::new(0);
    image
        .inspect_table_extra(object, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert!(calls.get() > 2);
    let stop = calls.get();
    calls.set(0);
    assert_eq!(
        image
            .inspect_table_extra(object, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            })
            .err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn external_extra_uses_existing_long_value_reader_without_changing_narrow_properties() {
    let raw = fixture::golden();
    let mut bytes =
        payload_fixture::database(6, &[None, None, Some(payload_fixture::external(&raw))]);
    let page = &bytes[4 * 4096..5 * 4096];
    let row = usize::from(u16::from_le_bytes([page[18], page[19]])) & 0x0fff;
    support::put16(&mut bytes, 4 * 4096 + row + 10, 1);
    bytes[13 * 4096..14 * 4096].copy_from_slice(&payload_fixture::lval(&raw));
    let image = capture(&bytes);
    let result = image
        .inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(result.raw(), Some(raw.as_slice()));
    assert_eq!(result.coverage(), TableExtraCoverage::WideMr2);
    assert_eq!(image.as_bytes(), bytes);

    let narrow = base::with_rule("Caption", "inert property");
    let properties = crate::inspect_table_properties(
        &mut Cursor::new(&narrow),
        CatalogScope::new(7),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let property = properties.groups()[0].get("Caption").unwrap();
    assert_eq!(property.text().unwrap(), "inert property");
}
