//! The public reader owns complete keys, not unrequested attachment payloads.
use super::*;
use tablecraft_ace::{AttachmentReader, ParentAttachments};
use tablecraft_testkit::{
    fixtures::access::{attachment_modern as modern, attachments as raw},
    support::ace::{
        mutable_source::MutableSource, page_reads::PageReads, resources::assert_public_boundary,
    },
};

const PAGE: usize = 4096;
const PAYLOAD_PAGE: usize = 23;

fn assert_first(parent: &ParentAttachments, modern: bool) {
    assert_eq!(parent.key(), Some(10));
    let mut attachments = parent.attachments();
    assert_eq!(attachments.len(), 2);
    let first = attachments.next().unwrap().unwrap();
    assert_eq!(first.identity(), 1);
    assert_eq!(
        first.file_name(),
        Some(if modern {
            "synthetic.bin"
        } else {
            "../é🦀\0.bin"
        })
    );
    assert_eq!(
        first.file_url(),
        Some(if modern {
            "relative?x"
        } else {
            "../unsafe?query"
        })
    );
    assert_eq!(first.file_type(), Some("field/type"));
    assert_eq!(first.encoded_file_data(), Some(raw::PAYLOAD));
    assert_eq!(first.file_flags(), Some(i32::MIN));
    assert_eq!(
        first.file_timestamp().unwrap().to_bits(),
        (-0.25_f64).to_bits()
    );
    let absent = attachments.next().unwrap().unwrap();
    assert_eq!(absent.identity(), 2);
    assert_eq!(absent.file_name(), None);
    assert_eq!(absent.encoded_file_data(), None);
    assert_eq!(absent.file_timestamp(), None);
    assert_eq!(absent.file_flags(), None);
}

#[test]
fn complete_parent_membership_owns_typed_metadata_and_encoded_payloads() {
    for code in [5, 6] {
        for modern_layout in [false, true] {
            let mut bytes = if modern_layout {
                modern::database(code)
            } else {
                raw::database()
            };
            bytes[20] = code;
            let first;
            {
                let mut source = Cursor::new(&bytes);
                let mut allowance = budget();
                let mut reader = AttachmentReader::open(
                    &mut source,
                    CatalogScope::new(1),
                    "Items",
                    "sCoRe",
                    TableSnapshotLimits::default(),
                    &mut allowance,
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    reader
                        .parents()
                        .iter()
                        .map(|link| (link.key(), link.child_count()))
                        .collect::<Vec<_>>(),
                    [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
                );
                assert!(matches!(
                    reader.read_parent(3),
                    Err(InspectError::InvalidPageRange)
                ));
                assert!(!reader.is_failed());
                first = reader.read_parent(0).unwrap();
                assert_first(&first, modern_layout);
                let second = reader.read_parent(1).unwrap();
                let empty = second.attachments().next().unwrap().unwrap();
                assert_eq!(empty.identity(), 3);
                assert_eq!(empty.encoded_file_data(), Some([].as_slice()));
                let assigned_empty = reader.read_parent(2).unwrap();
                assert_eq!(assigned_empty.key(), Some(30));
                assert_eq!(assigned_empty.attachments().len(), 0);
            }
            assert_first(&first, modern_layout);
            let file = first.attachments().next().unwrap().unwrap();
            let decoded = file.decode(&mut budget(), &|| false).unwrap().unwrap();
            assert_eq!(decoded.data(), [0, 1, 2, 255]);
        }
    }
}

fn bad_external() -> Vec<u8> {
    let mut descriptor = (0x40000000u32 | raw::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend(((PAYLOAD_PAGE as u32) << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut empty = 0x80000000u32.to_le_bytes().to_vec();
    empty.extend([0; 8]);
    let mut bytes = raw::with_data_fields(&descriptor, &empty);
    bytes[20] = 6;
    bytes.resize((PAYLOAD_PAGE + 1) * PAGE, 0);
    // A represented external payload whose native DATA page is malformed.
    bytes
}

#[test]
fn unselected_file_data_is_unread_but_selected_storage_failures_are_terminal() {
    let mut source = PageReads::new(bad_external());
    {
        let mut allowance = budget();
        let mut reader = AttachmentReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert_eq!(reader.parents()[0].child_count(), 2);
        assert_eq!(reader.read_parent(1).unwrap().attachments().len(), 1);
    }
    assert_eq!(source.count(PAYLOAD_PAGE as u64), 0);
    {
        let mut allowance = budget();
        let mut reader = AttachmentReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert!(reader.read_parent(0).is_err());
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(1),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    assert!(source.count(PAYLOAD_PAGE as u64) > 0);
}

#[test]
fn unknown_file_wrappers_remain_encoded_until_explicit_decode() {
    let bytes = raw::with_payloads(&[255, 1, 2], &[]);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let parent = reader.read_parent(0).unwrap();
    let file = parent.attachments().next().unwrap().unwrap();
    assert_eq!(file.encoded_file_data(), Some([255, 1, 2].as_slice()));
    assert!(file.decode(&mut budget(), &|| false).is_err());
    assert!(!reader.is_failed());
}

#[test]
fn changed_unselected_child_membership_invalidates_the_original_directory() {
    let mut source = MutableSource::new(raw::database());
    let image = source.image();
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    {
        let mut bytes = image.borrow_mut();
        let at = 22 * PAGE;
        let start =
            usize::from(u16::from_le_bytes(bytes[at + 14..at + 16].try_into().unwrap()) & 0x1fff);
        bytes[at + start + 2..at + start + 6].copy_from_slice(&20i32.to_le_bytes());
    }
    assert!(reader.read_parent(1).is_err());
    assert!(reader.is_failed());
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::CollectionReaderFailed)
    ));
}

#[test]
fn full_public_attachment_reader_preserves_exact_resources_and_cancellation() {
    let bytes = modern::database(6);
    assert_public_boundary(
        budget,
        |allowance, cancelled| {
            let mut source = Cursor::new(&bytes);
            let mut reader = AttachmentReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                TableSnapshotLimits::default(),
                allowance,
                cancelled,
            )?;
            reader.read_parent(0)
        },
        |parent| assert_first(parent, true),
        |parent| assert_first(parent, true),
    );
}

#[test]
fn nullable_keys_and_metadata_refusals_remain_independent_of_requested_payload() {
    let mut bytes = raw::database();
    let word = 12 * PAGE + 18;
    let start = usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
    bytes[12 * PAGE + start + 10] = 1;
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let empty = reader.read_parent(2).unwrap();
    assert_eq!(empty.key(), None);
    assert_eq!(empty.attachments().len(), 0);
    drop(reader);

    // The actual FileData declaration is invalid even if only an empty parent
    // would be requested. No generic scalar or empty-result shortcut can pass.
    let mut malformed = raw::database();
    malformed[20 * PAGE + 63 + 5 * 25 + 16] = 0x80;
    let mut source = Cursor::new(malformed);
    let mut allowance = budget();
    assert!(
        AttachmentReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .is_err()
    );
}

#[test]
fn forwarded_parent_and_child_roots_keep_logical_membership_and_signed_identities() {
    let mut bytes = modern::database(6);
    bytes.resize(25 * PAGE, 0);
    let empty = modern::inline(&[]);
    let first = modern::row(10, -7, 0, &modern::inline(raw::PAYLOAD));
    let null = modern::row(10, i32::MIN, 1, &[]);
    let third = modern::row(20, 3, 2, &empty);
    let roots = [
        (0, first),
        (0x8000, modern::row(99, 99, 1, &[])),
        (0x4000, (23u32 << 8).to_le_bytes().to_vec()),
        (0, third),
    ];
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&support::data_page(20, &roots));
    bytes[23 * PAGE..24 * PAGE].copy_from_slice(&support::data_page(20, &[(0x8000, null)]));
    let parent_row = |identity: i32, key: i32| {
        let mut row = 2u16.to_le_bytes().to_vec();
        row.extend(identity.to_le_bytes());
        row.extend(key.to_le_bytes());
        row.push(3);
        row
    };
    let roots = [
        (0, parent_row(1, 10)),
        (0x8000, parent_row(99, 99)),
        (0x4000, (24u32 << 8).to_le_bytes().to_vec()),
        (0, parent_row(3, 30)),
    ];
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &roots));
    bytes[24 * PAGE..25 * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, parent_row(2, 20))]));
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let first = reader.read_parent(0).unwrap();
    assert_eq!(
        first
            .attachments()
            .map(|file| file.unwrap().identity())
            .collect::<Vec<_>>(),
        [-7, i32::MIN]
    );
    let second = reader.read_parent(1).unwrap();
    assert_eq!(second.row_reference().page_number(), 12);
    assert_eq!(second.row_reference().slot_number(), 2);
    assert_eq!(second.attachments().next().unwrap().unwrap().identity(), 3);
    assert_eq!(
        reader.read_parent(2).unwrap().row_reference().slot_number(),
        3
    );
}

#[test]
fn cancellation_after_open_is_terminal_without_publishing_a_parent() {
    let stopped = Cell::new(false);
    let cancelled = || stopped.get();
    let mut source = Cursor::new(modern::database(6));
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &cancelled,
    )
    .unwrap();
    stopped.set(true);
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::Cancelled)
    ));
    assert!(reader.is_failed());
    stopped.set(false);
    assert!(matches!(
        reader.read_parent(1),
        Err(InspectError::CollectionReaderFailed)
    ));
}
