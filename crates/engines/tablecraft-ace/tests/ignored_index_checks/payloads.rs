use super::{SCOPE, budget, ignored_page, index_fixture, support};
use std::{fs, io::Cursor, ops::Bound, path::Path};
use tablecraft_ace::{
    BinaryIndexSelection, IndexCheckStatus, InspectError, TextIndexSelection, check_table,
    inspect_table_indexes, read_table, read_table_binary_index_range, read_table_index,
    read_table_text_index_range,
};
use tablecraft_core::values::Value;

const PAGE_BYTES: usize = 4096;
const HEADER_BYTES: usize = 63;
const FIELD_BYTES: usize = 25;
const COLUMN_COUNT_OFFSET: usize = 45;
const PHYSICAL_COUNT_OFFSET: usize = 51;
const INDEX_COUNT_BYTES: usize = 12;
const INDEX_BYTES: usize = 52;
const INDEX_FLAGS_OFFSET: usize = 46;
const NAME_LENGTH_BYTES: usize = 2;
const BINARY_KEY_TYPE: usize = 10 * PAGE_BYTES + 100;
const BINARY_INDEX_FLAGS: usize = 10 * PAGE_BYTES + 189;
const BINARY_ROOT: usize = 13;

fn original() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

fn ignored_copy(bytes: &mut [u8], table: &str, name: &str) {
    let metadata = inspect_table_indexes(
        &mut Cursor::new(&*bytes),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected = metadata
        .indexes()
        .iter()
        .find(|index| index.name() == name)
        .unwrap();
    assert!(selected.constraints().is_some());
    let rows = read_table(
        &mut Cursor::new(&*bytes),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let positions: Vec<_> = selected
        .columns()
        .iter()
        .map(|key| {
            metadata
                .schema()
                .columns()
                .iter()
                .position(|column| column.number() == key.number())
                .unwrap()
        })
        .collect();
    assert!(rows.rows().iter().all(|row| {
        positions
            .iter()
            .any(|&column| !matches!(row[column], Value::Null))
    }));

    // This is a synthetic flag control on a disposable registered copy. No entry
    // needs removal because independent stored values contain no all-NULL key.
    let base = metadata.schema().definition_page() as usize * PAGE_BYTES;
    let columns = u16::from_le_bytes(
        bytes[base + COLUMN_COUNT_OFFSET..base + COLUMN_COUNT_OFFSET + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    let physical = u32::from_le_bytes(
        bytes[base + PHYSICAL_COUNT_OFFSET..base + PHYSICAL_COUNT_OFFSET + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut offset = base + HEADER_BYTES + physical * INDEX_COUNT_BYTES + columns * FIELD_BYTES;
    for _ in 0..columns {
        let length = u16::from_le_bytes(
            bytes[offset..offset + NAME_LENGTH_BYTES]
                .try_into()
                .unwrap(),
        ) as usize;
        offset += NAME_LENGTH_BYTES + length;
    }
    let flags = offset + selected.data_number() as usize * INDEX_BYTES + INDEX_FLAGS_OFFSET;
    assert_eq!(bytes[flags], selected.flags());
    bytes[flags] |= 0x02;
}

#[test]
fn single_general_text_and_encoded_text_tuples_use_the_existing_physical_codecs() {
    let immutable = original();
    for (table, name, count) in [
        ("TAG_GRP_TB", "TAG_GRP_NME", 19),
        ("MSysObjects", "ParentIdName", 146),
    ] {
        let mut copy = immutable.clone();
        ignored_copy(&mut copy, table, name);
        let report = check_table(
            &mut Cursor::new(&copy),
            SCOPE,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.rows(), count);
        assert_eq!(
            report
                .indexes()
                .iter()
                .find(|index| index.index().name() == name)
                .unwrap()
                .status(),
            IndexCheckStatus::Checked
        );
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&copy),
                SCOPE,
                table,
                name,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
        if table == "TAG_GRP_TB" {
            assert_eq!(
                read_table_text_index_range(
                    &mut Cursor::new(&copy),
                    SCOPE,
                    table,
                    name,
                    TextIndexSelection::EqualityCandidates("VBA"),
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
        }
    }
    assert_eq!(original(), immutable);
}

#[test]
fn fixed_binary_physical_checks_do_not_enable_public_binary_seeks() {
    for code in [1, 2, 5, 6] {
        let mut bytes = index_fixture::database(false, false, false);
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        bytes[BINARY_KEY_TYPE] = 0x09;
        bytes[BINARY_INDEX_FLAGS] = 0x82;
        let entries: Vec<_> = (0..3_u32)
            .map(|slot| {
                let mut bytes = vec![0x7f, ((slot + 1) * 10) as u8, 0, 0, 0, 0, 0, 0, 0, 4];
                bytes.extend(((12_u32 << 8) | slot).to_be_bytes());
                bytes
            })
            .collect();
        bytes[BINARY_ROOT * PAGE_BYTES..(BINARY_ROOT + 1) * PAGE_BYTES]
            .copy_from_slice(&ignored_page::page(true, &entries, 0, 0, 0));
        let report = check_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.rows(), 3);
        assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
        assert_eq!(
            read_table_binary_index_range(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "ScoreKey",
                BinaryIndexSelection::Range {
                    lower: Bound::Included(&[10]),
                    upper: Bound::Included(&[10])
                },
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
        support::put32(&mut bytes, BINARY_ROOT * PAGE_BYTES + 4, 99);
        assert_eq!(
            check_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Malformed)
        );
    }
}
