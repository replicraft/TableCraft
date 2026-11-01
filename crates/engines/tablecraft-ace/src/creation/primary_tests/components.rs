//! Two-table component expectations from independent original source pages.
use super::{CreateColumn, CreateTable, budget, create_ace16_database_tables, sources};
use crate::{IndexCheckStatus, NativeImage, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::{canonical_creation, canonical_native17_creation, data_page};

const SCOPE: CatalogScope = CatalogScope::new(223);

struct Component {
    bytes: Vec<u8>,
    rows: Vec<Vec<Value>>,
    primary: bool,
}

fn primary(case: sources::Case) -> Component {
    let source = sources::source(case);
    assert!(source.violation.is_none());
    Component {
        bytes: source.bytes,
        rows: source.rows,
        primary: true,
    }
}

fn plain() -> Component {
    let source = canonical_creation::source(canonical_creation::Case::Empty);
    Component {
        bytes: canonical_native17_creation::source(source.bytes),
        rows: Vec::new(),
        primary: false,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn page(bytes: &[u8], number: usize) -> &[u8] {
    &bytes[number * 4096..(number + 1) * 4096]
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(&page[..2], &[1, 1]);
    let count = word(page, 12);
    let mut end = 4096;
    let mut rows = Vec::new();
    for slot in 0..count {
        let start = word(page, 14 + 2 * slot);
        assert!(start < end && start >= 14 + 2 * count);
        rows.push(page[start..end].to_vec());
        end = start;
    }
    assert_eq!(word(page, 2), end - (14 + 2 * count));
    rows
}

fn packed(owner: u32, records: &[Vec<u8>]) -> Vec<u8> {
    let used = 14 + 2 * records.len() + records.iter().map(Vec::len).sum::<usize>();
    let rows = records
        .iter()
        .cloned()
        .map(|row| (0, row))
        .collect::<Vec<_>>();
    let mut result = data_page(owner, &rows);
    result[2..4].copy_from_slice(&u16::try_from(4096 - used).unwrap().to_le_bytes());
    result
}

fn columns(primary: bool) -> [CreateColumn<'static>; 2] {
    let id = CreateColumn::new("Id", ScalarType::Int32);
    let id = if primary {
        id.with_primary_key("PrimaryId").unwrap()
    } else {
        id
    };
    [id, CreateColumn::new("Score", ScalarType::Int32)]
}

fn definition(source: &Component, second: bool, index: Option<(u32, u32)>) -> Vec<u8> {
    let mut result = page(&source.bytes, 7).to_vec();
    if second {
        put(&mut result, 55, 0x0000_0316);
        put(&mut result, 59, 0x0000_0317);
    }
    assert_eq!(source.primary, index.is_some());
    if let Some((root, map)) = index {
        // Frozen two-I32 source declarations place the physical descriptor143.
        assert_eq!(&result[143..147], &1923_u32.to_le_bytes());
        assert_eq!(&result[177..181], &0x0000_0316_u32.to_le_bytes());
        assert_eq!(&result[181..185], &9_u32.to_le_bytes());
        put(&mut result, 177, map);
        put(&mut result, 181, root);
    }
    result
}

fn leaf(source: &Component, definition: u32, data: u32) -> Vec<u8> {
    assert!(source.primary);
    let mut result = page(&source.bytes, 9).to_vec();
    assert_eq!(&result[..2], &[4, 1]);
    assert_eq!(&result[4..8], &7_u32.to_le_bytes());
    let active = 3616 - word(&result, 2);
    assert_eq!(active, source.rows.len() * 9);
    put(&mut result, 4, definition);
    for entry in result[480..480 + active].as_chunks_mut::<9>().0 {
        assert_eq!(entry[0], 127);
        let reference = u32::from_be_bytes(entry[5..9].try_into().unwrap());
        assert_eq!(reference >> 8, 8);
        let slot = reference & 255;
        assert!(usize::try_from(slot).unwrap() < source.rows.len());
        entry[5..9].copy_from_slice(&((data << 8) | slot).to_be_bytes());
    }
    result
}

#[test]
fn multiple_manual_keys_match_catalog_map_and_user_page_components_without_whole_image_claims() {
    let scenarios = [
        (
            primary(sources::Case::Empty),
            primary(sources::Case::One),
            [Some((11, 0x0000_0318)), Some((12, 0x0000_0319))],
            vec![8, 16],
        ),
        (
            primary(sources::Case::Boundary),
            primary(sources::Case::SignedUnsorted),
            [Some((11, 0x0000_0318)), Some((12, 0x0000_0319))],
            vec![8, 16],
        ),
        (
            plain(),
            primary(sources::Case::SignedUnsorted),
            [None, Some((11, 0x0000_0318))],
            vec![8],
        ),
        (
            primary(sources::Case::SignedUnsorted),
            plain(),
            [Some((11, 0x0000_0318)), None],
            vec![8],
        ),
    ];
    for (first, second, indexes, map_bits) in scenarios {
        // Every expectation is formed before a creator call. Source physical
        // pages/records are retained; literal role substitutions are explicit.
        let expected_first = definition(&first, false, indexes[0]);
        let expected_second = definition(&second, true, indexes[1]);
        let mut second_data = page(&second.bytes, 8).to_vec();
        put(&mut second_data, 4, 9);
        let first_catalog = records(page(&first.bytes, 4));
        let second_catalog = records(page(&second.bytes, 4));
        assert_eq!(&first_catalog[..7], &second_catalog[..7]);
        let mut catalog = first_catalog;
        let mut other = second_catalog[7].clone();
        assert_eq!(&other[2..6], &7_u32.to_le_bytes());
        assert_eq!(&other[32..42], b"I\0t\0e\0m\0s\0");
        put(&mut other, 2, 9);
        other[32..42].copy_from_slice(b"O\0t\0h\0e\0r\0");
        catalog.push(other);
        let expected_catalog = packed(2, &catalog);
        let mut catalog_td = page(&first.bytes, 2).to_vec();
        put(&mut catalog_td, 16, 9);
        let mut maps = records(page(&first.bytes, 3));
        let second_maps = records(page(&second.bytes, 3));
        assert_eq!(&maps[..22], &second_maps[..22]);
        maps.truncate(22);
        let mut table_map = vec![0; 13];
        table_map[6] = 4; // DATA10, for the second table's owned/candidate cells.
        maps.push(table_map.clone());
        maps.push(table_map);
        for bit in map_bits {
            let mut index_map = vec![0; 13];
            index_map[6] = bit; // Root11 or12, independent of table ordinal.
            maps.push(index_map);
        }
        let expected_maps = packed(0, &maps);
        let expected_leaves = indexes
            .iter()
            .enumerate()
            .filter_map(|(ordinal, role)| {
                role.map(|(root, _)| {
                    let source = if ordinal == 0 { &first } else { &second };
                    let (definition, data) = if ordinal == 0 { (7, 8) } else { (9, 10) };
                    (root, leaf(source, definition, data))
                })
            })
            .collect::<Vec<_>>();
        let first_columns = columns(first.primary);
        let second_columns = columns(second.primary);
        let first_rows = first.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let second_rows = second.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let tables = [
            CreateTable::new("Items", &first_columns, &first_rows),
            CreateTable::new("Other", &second_columns, &second_rows),
        ];
        let created = create_ace16_database_tables(&tables, &mut budget(), &|| false).unwrap();
        let bytes = created.as_bytes();
        assert_eq!(bytes.len(), (11 + expected_leaves.len()) * 4096);
        assert_eq!(created.table_count(), 2);
        assert_eq!(created.column_count(), 4);
        assert_eq!(created.row_count(), first.rows.len() + second.rows.len());
        assert_eq!(&bytes[..2 * 4096], &first.bytes[..2 * 4096]);
        assert_eq!(page(bytes, 2), catalog_td);
        assert_eq!(page(bytes, 3), expected_maps);
        assert_eq!(page(bytes, 4), expected_catalog);
        assert_eq!(&bytes[5 * 4096..7 * 4096], &first.bytes[5 * 4096..7 * 4096]);
        assert_eq!(page(bytes, 7), expected_first);
        assert_eq!(page(bytes, 8), page(&first.bytes, 8));
        assert_eq!(page(bytes, 9), expected_second);
        assert_eq!(page(bytes, 10), second_data);
        for (root, expected) in expected_leaves {
            assert_eq!(page(bytes, root as usize), expected);
        }
        let image =
            NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
        for (name, expected) in [("Items", &first), ("Other", &second)] {
            let snapshot =
                read_table(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
            assert_eq!(snapshot.rows(), expected.rows);
            let checked =
                check_table(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.indexes().len(), usize::from(expected.primary));
            for index in checked.indexes() {
                assert_eq!(index.status(), IndexCheckStatus::Checked);
            }
            #[cfg(feature = "journal")]
            image
                .check_native_ordinary_fixed_table(name, &mut budget(), &|| false)
                .unwrap();
        }
    }
}
