#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "attachments/fixture.rs"]
#[allow(dead_code)]
mod fixture;
mod support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_ace::{
    AttachmentMetadataColumn, ComplexReferenceViolation, InspectError, NativeReadSource,
    read_attachment_column, read_attachment_metadata,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const PAYLOAD_PAGE: usize = 23;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn read(bytes: &[u8]) -> Result<AttachmentMetadataColumn, InspectError> {
    read_attachment_metadata(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}

fn external() -> Vec<u8> {
    // Independent twelve-byte single-row LVAL descriptor; the file wrapper
    // remains byte-for-byte the original fixture payload.
    let mut descriptor = (0x40000000 | fixture::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend(((PAYLOAD_PAGE as u32) << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = fixture::with_data_fields(&descriptor, &[]);
    bytes.resize((PAYLOAD_PAGE + 1) * PAGE_BYTES, 0);
    let mut page = support::data_page(0, &[(0, fixture::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[PAYLOAD_PAGE * PAGE_BYTES..].copy_from_slice(&page);
    bytes
}

struct RefusePayload {
    bytes: Cursor<Vec<u8>>,
    calls: Cell<usize>,
}

impl NativeReadSource for RefusePayload {}

impl Read for RefusePayload {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.bytes.position() as usize / PAGE_BYTES == PAYLOAD_PAGE {
            self.calls.set(self.calls.get() + 1);
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        self.bytes.read(target)
    }
}

impl Seek for RefusePayload {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn metadata_projection_keeps_membership_nulls_and_exact_values_without_file_data() {
    for code in [2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = code;
        let snapshot = read(&bytes).unwrap();
        assert_eq!(
            snapshot
                .parents()
                .iter()
                .map(|parent| (parent.key(), parent.child_count()))
                .collect::<Vec<_>>(),
            [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
        );
        assert_eq!(
            snapshot.metadata_value(0, 0, "FileName"),
            Some(&Value::Text("../é🦀\0.bin".into()))
        );
        assert_eq!(
            snapshot.metadata_value(0, 1, "FileName"),
            Some(&Value::Null)
        );
        assert_eq!(
            snapshot.metadata_value(1, 0, "FileName"),
            Some(&Value::Text(String::new()))
        );
        let Some(Value::DateTime(timestamp)) = snapshot.metadata_value(0, 0, "FileTimeStamp")
        else {
            panic!("expected exact timestamp metadata");
        };
        assert_eq!(timestamp.to_bits(), (-0.25_f64).to_bits());
        assert!(snapshot.metadata_value(0, 0, "FileData").is_none());
        assert!(snapshot.metadata_value(2, 0, "FileName").is_none());
        assert_eq!(snapshot.child_snapshot().columns().len(), 7);
        assert_eq!(snapshot.child_snapshot().schema().columns().len(), 8);
        assert!(
            !snapshot
                .child_snapshot()
                .columns()
                .iter()
                .any(|column| column.name() == "FileData")
        );
    }
}

#[test]
fn metadata_does_not_visit_external_payload_pages_but_full_reads_do() {
    let bytes = external();
    let full = read_attachment_column(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        full.attachments(0)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .encoded_file_data(),
        Some(fixture::PAYLOAD)
    );

    let mut source = RefusePayload {
        bytes: Cursor::new(bytes),
        calls: Cell::new(0),
    };
    let metadata = read_attachment_metadata(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(metadata.parents()[0].child_count(), 2);
    assert_eq!(source.calls.get(), 0);
    assert!(
        read_attachment_column(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(source.calls.get(), 1);
}

#[test]
fn unread_payload_defects_do_not_hide_invalid_child_references() {
    let mut descriptor = 0x40200001_u32.to_le_bytes().to_vec();
    descriptor.extend((255_u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let original = fixture::with_data_fields(&descriptor, &[]);
    assert!(read(&original).is_ok());
    assert!(
        read_attachment_column(
            &mut Cursor::new(&original),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        )
        .is_err()
    );

    for (offset, value, expected) in [
        (2, 999, ComplexReferenceViolation::MissingParent),
        (6, 1, ComplexReferenceViolation::DuplicateChild),
    ] {
        let mut bytes = original.clone();
        let at = fixture::CHILD_PAGE * PAGE_BYTES;
        let start = u16::from_le_bytes(bytes[at + 16..at + 18].try_into().unwrap()) as usize;
        support::put32(&mut bytes, at + start + offset, value);
        assert_eq!(
            read(&bytes).err(),
            Some(InspectError::ComplexReference(expected))
        );
    }
}

#[test]
fn metadata_completion_checks_cancellation_budgets_and_source_immutability() {
    let bytes = external();
    let mut source = Cursor::new(bytes.clone());
    let count = Cell::new(0);
    read_attachment_metadata(
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
    for stop in [1, count.get() / 2, count.get()] {
        let observed = Cell::new(0);
        assert!(matches!(
            read_attachment_metadata(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| {
                    observed.set(observed.get() + 1);
                    observed.get() >= stop
                }
            ),
            Err(InspectError::Cancelled)
        ));
    }
    assert!(matches!(
        read_attachment_metadata(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(source.into_inner(), bytes);
}
