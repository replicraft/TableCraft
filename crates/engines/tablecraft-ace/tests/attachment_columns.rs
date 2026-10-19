#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
mod support;

use std::{cell::Cell, io::Cursor};
use support::{data_page, put16};
use tablecraft_ace::{
    AttachmentColumn, InspectError, read_attachment_column, read_multivalue_column,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const TEMPLATE_PAGE: usize = 17;
const FLAT_PAGE: usize = 20;
const CHILD_PAGE: usize = 22;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
// Independent Python-zlib golden retained by the original wrapper tests.
const PAYLOAD: &[u8] = &[
    1, 0, 0, 0, 24, 0, 0, 0, 120, 94, 19, 97, 96, 96, 96, 4, 98, 22, 32, 78, 98, 200, 100, 200, 3,
    210, 12, 140, 76, 255, 1, 15, 116, 2, 85,
];

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
fn definition(
    fields: &[(&str, u8, u16, u16)],
    records: u32,
    usage: u32,
    conceptual: bool,
) -> Vec<u8> {
    let mut page = base::definition(fields, records, usage);
    let mut variable = 0;
    for (i, (_, kind, _, _)) in fields.iter().enumerate() {
        let at = FIELD_START + i * FIELD_BYTES;
        if matches!(kind, 10..=12) {
            page[at + 15] = 2;
            put16(&mut page, at + 7, variable);
            variable += 1;
        }
        if conceptual {
            page[at + 16] = 0x10;
        }
    }
    put16(&mut page, 43, variable);
    page
}
fn template() -> Vec<(&'static str, u8, u16, u16)> {
    vec![
        ("FileURL", 12, 0, 0),
        ("FileName", 10, 0, 510),
        ("FileType", 10, 0, 510),
        ("FileData", 11, 0, 0),
        ("FileTimeStamp", 8, 0, 8),
        ("FileFlags", 4, 8, 4),
    ]
}
fn inline(data: &[u8]) -> Vec<u8> {
    let mut value = (0x8000_0000 | data.len() as u32).to_le_bytes().to_vec();
    value.extend([0; 8]);
    value.extend(data);
    value
}
fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn database() -> Vec<u8> {
    let mut bytes = complex_fixture::with_mappings(vec![complex_fixture::mapping(7, 10, 20, 17)]);
    bytes[10 * PAGE_BYTES + FIELD_START + FIELD_BYTES + 15] = 7;
    bytes[TEMPLATE_PAGE * PAGE_BYTES..(TEMPLATE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition(&template(), 0, 18 << 8, true));
    let fields = [
        ("Owner", 4, 0, 4),
        ("ChildID", 4, 4, 4),
        ("FileURL", 12, 0, 0),
        ("FileName", 10, 0, 510),
        ("FileType", 10, 0, 510),
        ("FileData", 11, 0, 0),
        ("FileTimeStamp", 8, 8, 8),
        ("FileFlags", 4, 16, 4),
    ];
    let mut flat = definition(&fields, 3, 21 << 8, false);
    flat[FIELD_START + 16] = 8;
    flat[FIELD_START + FIELD_BYTES + 15] = 7;
    bytes[FLAT_PAGE * PAGE_BYTES..(FLAT_PAGE + 1) * PAGE_BYTES].copy_from_slice(&flat);
    let rows = (0..3)
        .map(|i| {
            let mut row = 8_u16.to_le_bytes().to_vec();
            row.extend((if i == 2 { 20_i32 } else { 10 }).to_le_bytes());
            row.extend((i + 1_i32).to_le_bytes());
            row.extend((-0.25_f64).to_bits().to_le_bytes());
            row.extend(i32::MIN.to_le_bytes());
            let fields = match i {
                0 => [
                    inline(&text("../unsafe?query")),
                    text("../é🦀\0.bin"),
                    text("field/type"),
                    inline(PAYLOAD),
                ],
                1 => std::array::from_fn(|_| Vec::new()),
                _ => [inline(&[]), Vec::new(), Vec::new(), inline(&[])],
            };
            let mut offsets = vec![row.len() as u16];
            for field in fields {
                row.extend(field);
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(4_u16.to_le_bytes());
            row.push(if i == 1 { 3 } else { 0xff });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &rows));
    bytes
}

#[test]
fn typed_attachment_metadata_keeps_exact_labels_nulls_empty_and_scoped_identities() {
    for code in [5, 6] {
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
