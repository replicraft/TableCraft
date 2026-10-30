use tablecraft_testkit::access::mutation_fixture as base;
#[path = "complex_schema/fixture.rs"]
#[allow(dead_code)]
mod complex_fixture;
#[path = "attachment_modern/fixture.rs"]
mod fixture;
#[path = "attachments/fixture.rs"]
#[allow(dead_code)]
mod old_attachment;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_ace::{
    ComplexReferenceViolation, InspectError, NativeReadSource, inspect_attachment_declarations,
    inspect_complex_schema, inspect_table_schema, read_attachment_column, read_attachment_metadata,
    read_complex_column_links, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);
const PAYLOAD_PAGE: usize = 23;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn full(bytes: &[u8]) -> Result<tablecraft_ace::AttachmentColumn, InspectError> {
    read_attachment_column(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn variable_keys_and_literal_footer_are_qualified_only_through_attachment_roles() {
    for code in [5, 6] {
        let bytes = fixture::database(code);
        let (start, end) = fixture::span(&bytes, 0);
        let row = &bytes[start..end];
        assert_eq!(row.len(), 164);
        assert_eq!(row[..2], [8, 0]);
        assert_eq!(row[14..18], 10_i32.to_le_bytes());
        assert_eq!(row[143..147], 1_i32.to_le_bytes());
        assert_eq!(
            &row[147..],
            &[
                147, 0, 143, 0, 111, 0, 91, 0, 65, 0, 18, 0, 14, 0, 6, 0, 255
            ]
        );
        let schema = inspect_complex_schema(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let column = &schema.columns()[0];
        assert_eq!(
            (column.foreign_key_number(), column.identity_number()),
            (0, 7)
        );
        assert_eq!(column.flat_table().columns().len(), 8);
        assert!(
            column
                .flat_table()
                .columns()
                .iter()
                .all(|field| field.scalar_type().is_none())
        );
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Children",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
        let declarations = inspect_attachment_declarations(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(declarations.len(), 6);
        assert!(
            declarations
                .iter()
                .all(|declaration| declaration.member().collection().as_str() == "Score")
        );
        for (declaration, (name, kind)) in declarations.iter().zip([
            ("FileURL", ScalarType::Text),
            ("FileName", ScalarType::Text),
            ("FileType", ScalarType::Text),
            ("FileData", ScalarType::Binary),
            ("FileTimeStamp", ScalarType::DateTime),
            ("FileFlags", ScalarType::Int32),
        ]) {
            assert_eq!(declaration.member().column().name(), name);
            assert_eq!(declaration.member().column().data_type(), kind);
        }
    }
}

#[test]
fn modern_links_and_metadata_preserve_masks_nulls_empty_and_exact_scalars() {
    let bytes = fixture::database(6);
    let original = bytes.clone();
    let links = read_complex_column_links(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        links
            .parents()
            .iter()
            .map(|parent| (parent.key(), parent.child_count()))
            .collect::<Vec<_>>(),
        [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
    );
    assert_eq!(
        links.child_rows(0).unwrap().next().unwrap()[7],
        Value::Int32(1)
    );
    let metadata = read_attachment_metadata(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        metadata.metadata_value(0, 0, "FileName"),
        Some(&Value::Text("synthetic.bin".into()))
    );
    assert_eq!(
        metadata.metadata_value(0, 1, "FileFlags"),
        Some(&Value::Null)
    );
    assert_eq!(
        metadata.metadata_value(0, 1, "FileTimeStamp"),
        Some(&Value::Null)
    );
    assert_eq!(
        metadata.metadata_value(1, 0, "FileName"),
        Some(&Value::Text(String::new()))
    );
    assert!(metadata.metadata_value(0, 0, "FileData").is_none());
    let collection = full(&bytes).unwrap();
    for (name, kind) in [
        ("FileURL", ScalarType::Text),
        ("FileName", ScalarType::Text),
        ("FileType", ScalarType::Text),
        ("FileData", ScalarType::Binary),
        ("FileTimeStamp", ScalarType::DateTime),
        ("FileFlags", ScalarType::Int32),
    ] {
        assert_eq!(collection.member_type(name), Some(kind));
        assert_eq!(metadata.member_type(name), Some(kind));
    }
    assert_eq!(collection.member_type("fIlEnAmE"), Some(ScalarType::Text));
    assert_eq!(metadata.member_type("fIlEnAmE"), Some(ScalarType::Text));
    for unknown in ["Owner", "ChildID", "FileSize", ""] {
        assert_eq!(collection.member_type(unknown), None);
        assert_eq!(metadata.member_type(unknown), None);
    }
    let mut first = collection.attachments(0).unwrap();
    let value = first.next().unwrap().unwrap();
    assert_eq!(value.identity(), 1);
    assert_eq!(value.file_flags(), Some(i32::MIN));
    assert_eq!(
        value.file_timestamp().unwrap().to_bits(),
        (-0.25_f64).to_bits()
    );
    assert_eq!(value.encoded_file_data(), Some(old_attachment::PAYLOAD));
    let decoded = value.decode(&mut budget(), &|| false).unwrap().unwrap();
    assert_eq!(decoded.data(), [0, 1, 2, 255]);
    assert_eq!(first.next().unwrap().unwrap().encoded_file_data(), None);
    assert_eq!(
        collection
            .attachments(1)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .encoded_file_data(),
        Some([].as_slice())
    );
    assert_eq!(bytes, original);
}

#[test]
fn modern_unknown_flags_wrong_roles_and_variable_key_widths_do_not_qualify() {
    let original = fixture::database(6);
    for (relative, value) in [
        (fixture::FIRST_FIELD + 16, 4),
        (fixture::ID_FIELD + 16, 8),
        (fixture::FIRST_FIELD + fixture::FIELD_BYTES + 16, 0x11),
        (fixture::FIRST_FIELD + fixture::FIELD_BYTES + 15, 6),
        (fixture::ID_FIELD + 23, 3),
    ] {
        let mut bytes = original.clone();
        bytes[fixture::FLAT * fixture::PAGE + relative] = value;
        assert!(full(&bytes).is_err());
    }
    let mut bytes = original;
    let (start, end) = fixture::span(&bytes, 0);
    // Shorten only the final variable identity to three bytes, retaining a
    // readable monotone footer and opaque capacity byte before that footer.
    bytes[end - 17..end - 15].copy_from_slice(&146_u16.to_le_bytes());
    assert_eq!(bytes[start..start + 2], [8, 0]);
    assert_eq!(full(&bytes).err(), Some(InspectError::Malformed));
}

#[test]
fn modern_late_orphans_and_null_or_duplicate_variable_keys_are_not_hidden() {
    let original = fixture::database(6);
    for mutation in 0..4 {
        let mut bytes = original.clone();
        let (start, end) = fixture::span(&bytes, 2);
        let expected = match mutation {
            0 => {
                bytes[start + 14..start + 18].copy_from_slice(&999_i32.to_le_bytes());
                ComplexReferenceViolation::MissingParent
            }
            1 => {
                bytes[end - 21..end - 17].copy_from_slice(&1_i32.to_le_bytes());
                ComplexReferenceViolation::DuplicateChild
            }
            2 => {
                bytes[end - 1] &= !1;
                ComplexReferenceViolation::NullChild
            }
            _ => {
                bytes[end - 1] &= !128;
                ComplexReferenceViolation::NullChild
            }
        };
        assert_eq!(
            full(&bytes).err(),
            Some(InspectError::ComplexReference(expected))
        );
    }
}

struct RefusePayload {
    bytes: Cursor<Vec<u8>>,
    reads: Cell<usize>,
}

impl NativeReadSource for RefusePayload {}

impl Read for RefusePayload {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.bytes.position() as usize / fixture::PAGE == PAYLOAD_PAGE {
            self.reads.set(self.reads.get() + 1);
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
fn modern_metadata_projection_does_not_visit_file_data_pages() {
    let mut descriptor = (0x40000000 | old_attachment::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((PAYLOAD_PAGE as u32 * 256).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = fixture::with_first_data(6, &descriptor);
    let mut page = support::data_page(0, &[(0, old_attachment::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[PAYLOAD_PAGE * fixture::PAGE..(PAYLOAD_PAGE + 1) * fixture::PAGE].copy_from_slice(&page);
    let mut source = RefusePayload {
        bytes: Cursor::new(bytes),
        reads: Cell::new(0),
    };
    let metadata =
        read_attachment_metadata(&mut source, SCOPE, "Items", "Score", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(metadata.parents()[0].child_count(), 2);
    assert_eq!(metadata.member_type("FileData"), Some(ScalarType::Binary));
    assert!(metadata.metadata_value(0, 0, "FileData").is_none());
    assert_eq!(source.reads.get(), 0);
    assert!(
        read_attachment_column(&mut source, SCOPE, "Items", "Score", &mut budget(), &|| {
            false
        })
        .is_err()
    );
    assert_eq!(source.reads.get(), 1);
}

#[test]
fn modern_snapshots_keep_shared_limits_late_cancellation_and_original_bytes() {
    let bytes = fixture::database(6);
    let calls = Cell::new(0_usize);
    read_attachment_column(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        "Score",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for threshold in [0, calls.get() / 2, calls.get() - 1] {
        let observed = Cell::new(0_usize);
        assert_eq!(
            read_attachment_column(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "Score",
                &mut budget(),
                &|| {
                    let at = observed.get();
                    observed.set(at + 1);
                    at >= threshold
                }
            )
            .err(),
            Some(InspectError::Cancelled)
        );
    }
    for mut limited in [
        ResourceBudget::new(0, 16384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16384, 0),
    ] {
        assert!(matches!(
            read_attachment_metadata(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "Score",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Children",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(schema.scalar_columns().is_err());
}
