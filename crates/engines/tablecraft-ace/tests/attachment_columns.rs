#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
use tablecraft_testkit::access as support;

use std::{cell::Cell, io::Cursor};

use tablecraft_ace::{
    AttachmentColumn, CollectionPageRequest, InspectError, TablePageRequest, TableSnapshotLimits,
    read_attachment_column, read_attachment_metadata, read_multivalue_column,
    read_stored_collection_table_page_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[path = "attachments/fixture.rs"]
mod fixture;
use fixture::*;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn inspect(bytes: &[u8]) -> Result<AttachmentColumn, InspectError> {
    read_attachment_column(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}
#[test]
fn typed_attachment_metadata_keeps_exact_labels_nulls_empty_and_scoped_identities() {
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let collection = inspect(&bytes).unwrap();
        let mut values = collection.attachments(0).unwrap();
        let first = values.next().unwrap().unwrap();
        assert_eq!(first.identity(), 1);
        assert_eq!(first.file_url(), Some("../unsafe?query"));
        assert_eq!(first.file_name(), Some("../é🦀\0.bin"));
        assert_eq!(first.file_type(), Some("field/type"));
        assert_eq!(first.encoded_file_data(), Some(PAYLOAD));
        assert_eq!(
            collection.stored_value(0, 0, "FileData"),
            Some(&Value::Binary(PAYLOAD.into()))
        );
        assert_eq!(
            collection.stored_value(0, 1, "FileData"),
            Some(&Value::Null)
        );
        assert_eq!(
            collection.stored_value(1, 0, "FileData"),
            Some(&Value::Binary(Vec::new()))
        );
        assert_eq!(
            first.file_timestamp().unwrap().to_bits(),
            (-0.25_f64).to_bits()
        );
        assert_eq!(first.file_flags(), Some(i32::MIN));
        let null = values.next().unwrap().unwrap();
        assert_eq!(null.identity(), 2);
        assert_eq!(null.file_url(), None);
        assert_eq!(null.file_name(), None);
        assert_eq!(null.file_type(), None);
        assert_eq!(null.encoded_file_data(), None);
        assert_eq!(null.file_timestamp(), None);
        assert_eq!(null.file_flags(), None);
        let empty = collection.attachments(1).unwrap().next().unwrap().unwrap();
        assert_eq!(empty.identity(), 3);
        assert_eq!(empty.file_url(), Some(""));
        assert_eq!(empty.file_name(), Some(""));
        assert_eq!(empty.encoded_file_data(), Some([].as_slice()));
        assert_eq!(collection.attachments(2).unwrap().len(), 0);
        assert!(collection.attachments(3).is_none());
    }
}

#[test]
fn payload_decoding_is_explicit_and_wrapper_labels_do_not_replace_field_metadata() {
    let bytes = database();
    let collection = inspect(&bytes).unwrap();
    let mut values = collection.attachments(0).unwrap();
    let first = values.next().unwrap().unwrap();
    let decoded = first.decode(&mut budget(), &|| false).unwrap().unwrap();
    assert_eq!(decoded.type_label(), "bin");
    assert_eq!(decoded.data(), [0, 1, 2, 255]);
    assert_eq!(first.file_type(), Some("field/type"));
    let null = values.next().unwrap().unwrap();
    assert!(null.decode(&mut budget(), &|| false).unwrap().is_none());
    assert!(matches!(
        null.decode(&mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let empty = collection.attachments(1).unwrap().next().unwrap().unwrap();
    assert!(empty.decode(&mut budget(), &|| false).is_err());
    assert!(matches!(
        first.decode(&mut ResourceBudget::new(0, 16_384, 1_000_000), &|| false),
        Err(InspectError::Limit(_))
    ));
}

#[test]
fn attachment_templates_are_named_strict_and_independent_of_conceptual_column_order() {
    let original = database();
    let mut bytes = original.clone();
    let mut fields = template();
    fields.reverse();
    bytes[TEMPLATE_PAGE * PAGE_BYTES..(TEMPLATE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition(&fields, 0, 18 << 8, true));
    assert_eq!(inspect(&bytes).unwrap().attachments(0).unwrap().len(), 2);
    for (at, value) in [
        (TEMPLATE_PAGE * PAGE_BYTES + 16, 1),
        (TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + 16, 0x11),
        (
            FLAT_PAGE * PAGE_BYTES + FIELD_START + 2 * FIELD_BYTES + 16,
            0x10,
        ),
        (
            TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + FIELD_BYTES + 15,
            6,
        ),
    ] {
        let mut changed = original.clone();
        changed[at] = value;
        assert!(inspect(&changed).is_err());
    }
    assert!(
        read_multivalue_column(
            &mut Cursor::new(&original),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn stored_attachment_views_keep_read_limits_cancellation_and_original_bytes() {
    let original = database();
    let mut source = Cursor::new(original.clone());
    let count = Cell::new(0);
    read_attachment_column(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [0, count.get() / 2, count.get() - 1] {
        let calls = Cell::new(0);
        assert!(matches!(
            read_attachment_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| {
                    let at = calls.get();
                    calls.set(at + 1);
                    at >= stop
                }
            ),
            Err(InspectError::Cancelled)
        ));
    }
    for mut limited in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            read_attachment_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(source.into_inner(), original);
}

#[test]
fn jet4_and_unqualified_ace_layouts_do_not_enable_typed_attachment_reads() {
    for code in [1, 3] {
        let mut bytes = database();
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        assert!(matches!(inspect(&bytes), Err(InspectError::Unsupported)));
    }
}

#[test]
fn mixed_pages_retain_typed_attachment_metadata_and_empty_memberships() {
    use tablecraft_ace::{
        StoredCollectionItem, StoredCollectionPageValue, TablePageRequest,
        read_stored_collection_table_page,
    };
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let page = read_stored_collection_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 4).unwrap(),
            false,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(page.row_count(), 3);
        assert!(page.reached_end());
        let StoredCollectionPageValue::Collection(cell) = page.cell(0, 1).unwrap() else {
            panic!("Expected typed attachments")
        };
        assert_eq!(cell.key(), Some(10));
        let mut items = cell.items().unwrap();
        let StoredCollectionItem::Attachment(first) = items.next().unwrap().unwrap() else {
            panic!("Expected stored attachment")
        };
        assert_eq!(first.file_name(), Some("../é🦀\0.bin"));
        assert_eq!(first.encoded_file_data(), Some(PAYLOAD));
        assert_eq!(
            first.file_timestamp().unwrap().to_bits(),
            (-0.25_f64).to_bits()
        );
        let StoredCollectionItem::Attachment(null) = items.next().unwrap().unwrap() else {
            panic!("Expected stored attachment")
        };
        assert!(null.encoded_file_data().is_none());
        let StoredCollectionPageValue::Collection(empty) = page.cell(2, 1).unwrap() else {
            panic!("Expected empty collection")
        };
        assert_eq!(empty.key(), Some(30));
        assert_eq!(empty.items().unwrap().len(), 0);
        assert!(page.cell(3, 1).is_none());
        bytes[TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + 16] = 0x11;
        assert!(matches!(
            read_stored_collection_table_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                TablePageRequest::new(u32::MAX, 1).unwrap(),
                false,
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn named_metadata_discovery_is_inert_and_borrowed_members_keep_exact_carriers() {
    use tablecraft_ace::inspect_attachment_members;
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let collection = inspect(&bytes).unwrap();
        let members = inspect_attachment_members(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            members
                .iter()
                .map(|member| member.column().name())
                .collect::<Vec<_>>(),
            [
                "FileURL",
                "FileName",
                "FileType",
                "FileData",
                "FileTimeStamp",
                "FileFlags"
            ]
        );
        assert!(members.iter().all(|member| {
            member.collection().as_str() == "Score" && member.column().nullable()
        }));
        assert_eq!(
            collection.metadata_value(0, 0, "filename"),
            Some(&Value::Text("../é🦀\0.bin".into()))
        );
        assert_eq!(
            collection.metadata_value(0, 0, "FileFlags"),
            Some(&Value::Int32(i32::MIN))
        );
        assert_eq!(
            collection.metadata_value(0, 1, "FileName"),
            Some(&Value::Null)
        );
        assert_eq!(
            collection.metadata_value(1, 0, "FileName"),
            Some(&Value::Text("".into()))
        );
        for (parent, child, field) in [
            (0, 0, "FileData"),
            (0, 0, "Unknown"),
            (2, 0, "FileName"),
            (usize::MAX, usize::MAX, "FileName"),
        ] {
            assert!(collection.metadata_value(parent, child, field).is_none());
        }
        bytes[CHILD_PAGE * PAGE_BYTES] = 0;
        assert_eq!(
            inspect_attachment_members(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .len(),
            6
        );
        assert!(inspect(&bytes).is_err());
    }
}

#[test]
fn explicit_attachment_limits_preserve_complete_parent_child_validation() {
    let bytes = database();
    for (cap, accepted) in [(2, false), (3, true)] {
        let result = tablecraft_ace::read_attachment_column_with_limits(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            tablecraft_ace::TableSnapshotLimits::new(cap, 1).unwrap(),
            &mut budget(),
            &|| false,
        );
        if accepted {
            assert_eq!(result.unwrap().attachments(0).unwrap().len(), 2);
        } else {
            assert_eq!(result.err(), Some(InspectError::StructureLimit));
        }
    }
    assert_eq!(bytes, database());
}

#[test]
fn explicit_attachment_pages_keep_typed_whole_table_limits_and_visible_ranges() {
    let bytes = database();
    let read = |cap| {
        read_stored_collection_table_page_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            CollectionPageRequest::new(
                TablePageRequest::new(1, 1).unwrap(),
                TableSnapshotLimits::new(cap, 1).unwrap(),
            ),
            &mut budget(),
            &|| false,
        )
    };
    assert_eq!(read(2).err(), Some(InspectError::StructureLimit));
    let page = read(3).unwrap();
    assert_eq!(page.row_count(), 1);
    assert_eq!(
        page.collection_column(1).unwrap().links().parents().len(),
        3
    );
    assert_eq!(bytes, database());
}

#[test]
fn signed_attachment_ids_preserve_full_and_metadata_only_values() {
    const CHILD_PAGE: usize = 22;
    const PAGE_BYTES: usize = 4096;
    const SLOT_DIRECTORY: usize = 14;
    const IDENTITY_OFFSET: usize = 6;

    for code in [2, 5, 6] {
        for identities in [[i32::MIN, -1, 0], [-7, -2, i32::MAX]] {
            let mut bytes = database();
            bytes[20] = code;
            for (slot, identity) in identities.into_iter().enumerate() {
                let at = CHILD_PAGE * PAGE_BYTES + SLOT_DIRECTORY + slot * 2;
                let start = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
                support::put32(
                    &mut bytes,
                    CHILD_PAGE * PAGE_BYTES + start + IDENTITY_OFFSET,
                    identity as u32,
                );
            }

            let full = inspect(&bytes).unwrap();
            let observed = (0..2)
                .flat_map(|parent| full.attachments(parent).unwrap())
                .map(|value| value.unwrap().identity())
                .collect::<Vec<_>>();
            assert_eq!(observed, identities);
            assert_eq!(
                full.attachments(0)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .encoded_file_data(),
                Some(PAYLOAD)
            );

            let metadata = read_attachment_metadata(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let snapshot = metadata.child_snapshot();
            let identity = snapshot
                .columns()
                .iter()
                .position(|column| column.name() == "ChildID")
                .unwrap();
            let observed = snapshot
                .rows()
                .iter()
                .map(|row| row[identity].clone())
                .collect::<Vec<_>>();
            assert_eq!(observed, identities.map(Value::Int32));
        }
    }
}
