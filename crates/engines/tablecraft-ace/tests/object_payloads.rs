#[path = "mutation/fixture.rs"]
mod base;
#[path = "object_payloads/fixture.rs"]
mod payload_fixture;
use tablecraft_testkit::access as support;

use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{InspectError, NativeImage, PayloadPresence};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn opaque_catalog_payloads_keep_native_order_exact_bytes_and_original_image() {
    let binary = (0..=255).collect::<Vec<u8>>();
    for code in [1, 2, 5, 6] {
        let bytes = payload_fixture::database(
            code,
            &[
                Some(payload_fixture::inline(b"body")),
                Some(payload_fixture::inline(&binary)),
                Some(payload_fixture::inline(b"extra")),
            ],
        );
        let image = capture(&bytes);
        let object = CatalogScope::new(7).object_id(10);
        let data = image
            .read_object_payloads(object, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(data.object_id(), object);
        assert_eq!(data.body().as_bytes(), Some(&b"body"[..]));
        assert_eq!(data.module().as_bytes(), Some(binary.as_slice()));
        assert_eq!(data.extra().as_bytes(), Some(&b"extra"[..]));
        for payload in [data.body(), data.module(), data.extra()] {
            assert_eq!(payload.presence(), PayloadPresence::Present);
        }
        assert_eq!(image.as_bytes(), bytes);
    }
}

#[test]
fn null_empty_and_undeclared_payloads_remain_distinct_and_foreign_ids_fail() {
    // Unrelated native property storage does not become an object body/module.
    let properties = capture(&base::with_rule("Caption", "inert property"));
    let properties = properties
        .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(properties.body().presence(), PayloadPresence::Absent);
    let bytes = payload_fixture::database(6, &[None, Some(payload_fixture::inline(&[])), None]);
    let image = capture(&bytes);
    let data = image
        .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(data.body().presence(), PayloadPresence::Absent);
    assert_eq!(data.body().as_bytes(), None);
    assert_eq!(data.module().presence(), PayloadPresence::Present);
    assert_eq!(data.module().as_bytes(), Some(&[][..]));
    assert_eq!(
        image
            .read_object_payloads(CatalogScope::new(8).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::UnknownObject)
    );
    let unknown = capture(&support::image(false, false));
    let data = unknown
        .read_object_payloads(CatalogScope::new(7).object_id(100), &mut budget(), &|| {
            false
        })
        .unwrap();
    for payload in [data.body(), data.module(), data.extra()] {
        assert_eq!(payload.presence(), PayloadPresence::Uninspected);
        assert!(payload.as_bytes().is_none());
    }
    let empty_definition = capture(&payload_fixture::database(6, &[None, Some(vec![]), None]));
    assert_eq!(
        empty_definition
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Unsupported)
    );
}

#[test]
fn external_and_chained_object_bytes_reuse_bounded_native_long_value_validation() {
    let payload = b"opaque external bytes";
    let mut bytes =
        payload_fixture::database(6, &[None, Some(payload_fixture::external(payload)), None]);
    bytes[13 * 4096..14 * 4096].copy_from_slice(&payload_fixture::lval(payload));
    assert_eq!(
        capture(&bytes)
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .unwrap()
            .module()
            .as_bytes(),
        Some(&payload[..])
    );
    let mut descriptor = payload_fixture::external(payload);
    descriptor[..4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let mut chain = payload_fixture::database(6, &[None, Some(descriptor), None]);
    chain.resize(15 * 4096, 0);
    let first = [(14_u32 << 8).to_le_bytes().as_slice(), &payload[..7]].concat();
    let last = [0_u32.to_le_bytes().as_slice(), &payload[7..]].concat();
    chain[13 * 4096..14 * 4096].copy_from_slice(&payload_fixture::lval(&first));
    chain[14 * 4096..15 * 4096].copy_from_slice(&payload_fixture::lval(&last));
    assert_eq!(
        capture(&chain)
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .unwrap()
            .module()
            .as_bytes(),
        Some(&payload[..])
    );
    chain[14 * 4096 + 4] = 0;
    assert_eq!(
        capture(&chain)
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn ambiguous_metadata_unknown_carriers_and_late_payload_failures_publish_no_data_set() {
    let original = payload_fixture::database(
        6,
        &[
            Some(payload_fixture::inline(b"valid")),
            Some(vec![0; 3]),
            None,
        ],
    );
    assert!(
        capture(&original)
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .is_err()
    );
    let mut unknown =
        payload_fixture::database(6, &[Some(payload_fixture::inline(b"valid")), None, None]);
    unknown[2 * 4096 + 63 + 6 * 25] = 10;
    assert_eq!(
        capture(&unknown)
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Unsupported)
    );
    let mut duplicate = base::database();
    let columns = [
        ("Id", 4, 0, 4),
        ("ParentId", 4, 4, 4),
        ("Name", 10, 0, 510),
        ("Type", 3, 8, 2),
        ("Flags", 4, 10, 4),
        ("LvProp", 11, 0, 0),
        ("Lv", 11, 0, 0),
        ("Lv", 11, 0, 0),
        ("LvModule", 11, 0, 0),
    ];
    duplicate[2 * 4096..3 * 4096].copy_from_slice(&base::definition(&columns, 3, 3 << 8));
    let image = capture(&duplicate);
    assert_eq!(
        image
            .inspect_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Malformed)
    );
    assert_eq!(
        image
            .read_object_payloads(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Malformed)
    );
    assert_eq!(
        image
            .plan_int32_updates(
                "Items",
                "Score",
                &[tablecraft_ace::Int32Update::new(1, 20, 21)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(tablecraft_ace::EditError::Inspect(InspectError::Malformed))
    );
}

#[test]
fn opaque_payload_reads_keep_budgets_and_late_cancellation_without_mutating_capture() {
    let bytes = payload_fixture::database(
        6,
        &[
            Some(payload_fixture::inline(b"body")),
            Some(payload_fixture::inline(b"module")),
            None,
        ],
    );
    let image = capture(&bytes);
    let object = CatalogScope::new(7).object_id(10);
    let calls = Cell::new(0_usize);
    image
        .read_object_payloads(object, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let current = Cell::new(0);
    assert_eq!(
        image
            .read_object_payloads(object, &mut budget(), &|| {
                current.set(current.get() + 1);
                current.get() >= calls.get() - 1
            })
            .err(),
        Some(InspectError::Cancelled)
    );
    assert!(matches!(
        image.read_object_payloads(object, &mut ResourceBudget::new(66, 1, 100), &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(image.as_bytes(), bytes);
}
