use super::{Expected, capture, require_inline};
use crate::{NativeImage, QueryAttribute, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_queries::fitting_select::storage_pair;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 1_000_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(47),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn inline_binding_uses_exact_utf16_and_descriptor_not_display_values() {
    let literal = [4, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 52, 0, 50, 0];
    require_inline(&literal, "42", &mut budget(), &|| false).unwrap();
    assert!(require_inline(&literal, "43", &mut budget(), &|| false).is_err());
    assert!(require_inline(&literal, "042", &mut budget(), &|| false).is_err());
    let unicode = [
        6, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 45, 78, 62, 216, 128, 221,
    ];
    require_inline(&unicode, "中🦀", &mut budget(), &|| false).unwrap();
    assert!(require_inline(&literal, "42", &mut budget(), &|| true).is_err());
}

#[test]
fn raw_source_binding_requires_exact_native_key_and_expected_text() {
    let [before, _] = storage_pair();
    let native = image(&before);
    let schema = inspect_table_schema(
        &mut native.reader(),
        CatalogScope::new(47),
        "MSysQueries",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = Expected {
        native_id: -2147483525,
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 1],
        expression: "42",
    };
    let found = capture(&native, &schema, &[expected], &mut budget(), &|| false).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].request, 0);
    // Five original fragments have an exact third-index projection position.
    assert_eq!(found[0].ordinal, 3);
    let wrong = Expected {
        native_id: -2147483524,
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 1],
        expression: "42",
    };
    assert!(capture(&native, &schema, &[wrong], &mut budget(), &|| false).is_err());
    let wrong_order = Expected {
        native_id: -2147483525,
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 2],
        expression: "42",
    };
    assert!(capture(&native, &schema, &[wrong_order], &mut budget(), &|| false).is_err());
}

#[test]
#[ignore = "private hash-pinned first-party SDK images; independent evidence is retained under .cache"]
fn sdk_roots_bind_multiple_definitions_pages_forwarding_and_retired_targets() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let cases = [
        (
            "querydef-existing-save-sdk-20261108/files/multi-page-before.accdb",
            "42",
            "33e0e41c2152f4e7e5aadb18e0978b4f224ee225d8042dc08d70528f37cbca05",
        ),
        (
            "querydef-repeat-save-sdk-20261108/files/repeat-grow.accdb",
            "1000",
            "9c8570f86bdf1687410bc5a670773549d5e1c4c5ae75f336527051ba37099a7b",
        ),
        (
            "querydef-repeat-save-sdk-20261108/files/repeat-shrink.accdb",
            "7",
            "c6f06698c520fb52d3e96426f900f9586b92c1c97ad9472dc1aa02e277ff8bae",
        ),
        (
            "querydef-repeat-save-sdk-20261108/files/repeat-grow-again.accdb",
            "1000000",
            "bdc88830640664cf212ffbe024cb98851835c70b938ffd205295399a7e4df0da",
        ),
    ];
    for (file, expression, hash) in cases {
        let bytes = std::fs::read(root.join(".cache").join(file)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), hash);
        let native = image(&bytes);
        let schema = inspect_table_schema(
            &mut native.reader(),
            CatalogScope::new(47),
            "MSysQueries",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let requests = [Expected {
            native_id: -2147476548,
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 1],
            expression,
        }];
        let selected = capture(&native, &schema, &requests, &mut budget(), &|| false).unwrap();
        assert_eq!(selected[0].reference, (95 << 8) | 42);
        let roots = crate::table::mutation::record_roots::capture(
            &native,
            &schema,
            &selected,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(roots.selected.len(), 1);
        assert_eq!(roots.selected[0].reference, selected[0].reference);
    }
}

#[test]
fn shared_attribute_order_selects_exact_object_and_forwarded_roots_are_live_once() {
    use tablecraft_testkit::fixtures::access::query_expression_edits as raw;
    let before = raw::multiple_objects();
    let native = image(&before);
    let schema = inspect_table_schema(
        &mut native.reader(),
        CatalogScope::new(47),
        "MSysQueries",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let requests = [Expected {
        native_id: -2147483525,
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 1],
        expression: "42",
    }];
    let found = capture(&native, &schema, &requests, &mut budget(), &|| false).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].ordinal, 1);
    for (expected, before) in [
        ("1000", raw::replace(&raw::source(), "1000")),
        (
            "7",
            raw::replace(&raw::replace(&raw::source(), "1000"), "7"),
        ),
        (
            "1000000",
            raw::replace(
                &raw::replace(&raw::replace(&raw::source(), "1000"), "7"),
                "1000000",
            ),
        ),
    ] {
        let native = image(&before);
        let schema = inspect_table_schema(
            &mut native.reader(),
            CatalogScope::new(47),
            "MSysQueries",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let requests = [Expected {
            native_id: -2147483525,
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 1],
            expression: expected,
        }];
        let found = capture(&native, &schema, &requests, &mut budget(), &|| false).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].reference, (8 << 8) | 3);
        let roots = crate::table::mutation::record_roots::capture(
            &native,
            &schema,
            &found,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(roots.selected.len(), 1);
    }
}
