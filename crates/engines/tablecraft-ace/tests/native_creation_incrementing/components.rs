//! Source-owned role substitutions, without a multi-table whole-image claim.
use super::{budget, columns, sources};
use tablecraft_ace::{CreateColumn, CreateTable, create_ace16_database_tables};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::{canonical_creation, canonical_native17_creation, data_page};

fn page(bytes: &[u8], number: usize) -> &[u8] {
    &bytes[number * 4096..(number + 1) * 4096]
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    let mut end = 4096;
    let mut result = Vec::new();
    for slot in 0..count {
        let at = 14 + slot * 2;
        let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
        assert!(start < end && start >= 14 + 2 * count);
        result.push(page[start..end].to_vec());
        end = start;
    }
    result
}

fn packed(owner: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let used = 14 + 2 * rows.len() + rows.iter().map(Vec::len).sum::<usize>();
    let records = rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>();
    let mut bytes = data_page(owner, &records);
    bytes[2..4].copy_from_slice(&u16::try_from(4096 - used).unwrap().to_le_bytes());
    bytes
}

#[test]
#[ignore = "requires qualified empty Increment source before matching creator"]
fn two_tables_preserve_independently_expected_catalog_maps_definitions_and_empty_leaf() {
    let sources = sources();
    let first = sources.bytes(5, "increment-zero-before");
    let plain = canonical_creation::source(canonical_creation::Case::Empty);
    let plain = canonical_native17_creation::source(plain.bytes);
    let mut first_definition = page(first, 7).to_vec();
    assert_eq!(&first_definition[177..181], &0x316_u32.to_le_bytes());
    assert_eq!(&first_definition[181..185], &9_u32.to_le_bytes());
    put(&mut first_definition, 177, 0x318);
    put(&mut first_definition, 181, 11);
    let mut second_definition = page(&plain, 7).to_vec();
    put(&mut second_definition, 55, 0x316);
    put(&mut second_definition, 59, 0x317);
    let mut second_data = page(&plain, 8).to_vec();
    put(&mut second_data, 4, 9);
    let mut catalog = records(page(first, 4));
    let plain_catalog = records(page(&plain, 4));
    assert_eq!(&catalog[..7], &plain_catalog[..7]);
    let mut other = plain_catalog[7].clone();
    assert_eq!(&other[2..6], &7_u32.to_le_bytes());
    assert_eq!(&other[32..42], b"I\0t\0e\0m\0s\0");
    put(&mut other, 2, 9);
    other[32..42].copy_from_slice(b"O\0t\0h\0e\0r\0");
    catalog.push(other);
    let expected_catalog = packed(2, &catalog);
    let mut catalog_definition = page(first, 2).to_vec();
    put(&mut catalog_definition, 16, 9);
    let mut maps = records(page(first, 3));
    assert_eq!(&maps[..22], &records(page(&plain, 3))[..22]);
    maps.truncate(22);
    for bit in [4, 4, 8] {
        let mut map = vec![0; 13];
        map[6] = bit;
        maps.push(map);
    }
    let expected_maps = packed(0, &maps);
    let expected_leaf = page(first, 9).to_vec();
    assert_eq!(&expected_leaf[..8], &[4, 1, 32, 14, 7, 0, 0, 0]);
    let generated = columns(false);
    let ordinary = [
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    let rows: [&[Value]; 0] = [];
    let tables = [
        CreateTable::new("Items", &generated, &rows),
        CreateTable::new("Other", &ordinary, &rows),
    ];
    let created = create_ace16_database_tables(&tables, &mut budget(), &|| false).unwrap();
    let actual = created.as_bytes();
    assert_eq!(actual.len(), 12 * 4096);
    assert_eq!(
        (
            created.table_count(),
            created.column_count(),
            created.row_count()
        ),
        (2, 4, 0)
    );
    for (number, expected) in [
        (2, catalog_definition),
        (3, expected_maps),
        (4, expected_catalog),
        (7, first_definition),
        (9, second_definition),
        (10, second_data),
        (11, expected_leaf),
    ] {
        assert_eq!(page(actual, number), expected);
    }
    assert_eq!(page(actual, 8), page(first, 8));
    assert_eq!(&actual[..8192], &first[..8192]);
    assert_eq!(&actual[5 * 4096..7 * 4096], &first[5 * 4096..7 * 4096]);
}
