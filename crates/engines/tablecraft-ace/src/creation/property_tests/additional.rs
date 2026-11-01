//! Additional creation checks use only frozen source record/field bytes.
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, ScalarType, budget, columns,
    create_ace16_database_tables, sources,
};

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn catalog_records(bytes: &[u8], count: usize) -> Vec<Vec<u8>> {
    let page = &bytes[4 * 4096..5 * 4096];
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(&page[4..8], &2_u32.to_le_bytes());
    assert_eq!(word(page, 12), count);
    let mut end = 4096;
    let mut records = Vec::new();
    for slot in 0..count {
        let start = word(page, 14 + 2 * slot);
        assert!(start < end && start >= 14 + 2 * count);
        records.push(page[start..end].to_vec());
        end = start;
    }
    assert_eq!(word(page, 2), end - (14 + 2 * count));
    records
}

#[test]
fn distinct_tables_retain_their_own_literal_property_records_and_user_pages() {
    for (first, second) in [
        (sources::Case::Absent, sources::Case::AllowEmptyTrue),
        (sources::Case::RequiredFlag, sources::Case::RequiredText),
        (sources::Case::AllowEmptyFalse, sources::Case::RequiredId),
    ] {
        let first = sources::source(first);
        let second = sources::source(second);
        assert!(first.violation.is_none() && second.violation.is_none());
        let first_columns = columns(&first);
        let second_columns = columns(&second);
        let first_rows = first.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let second_rows = second.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let tables = [
            CreateTable::new("Items", &first_columns, &first_rows),
            CreateTable::new("Other", &second_columns, &second_rows),
        ];
        let created = create_ace16_database_tables(&tables, &mut budget(), &|| false).unwrap();
        assert_eq!(created.table_count(), 2);
        assert_eq!(created.column_count(), 10);
        assert_eq!(created.row_count(), first.rows.len() + second.rows.len());
        let bytes = created.as_bytes();
        assert_eq!(bytes.len(), 11 * 4096);

        // Complete user catalog records remain independently sourced. The new
        // table's five-unit name and Id are the only identity substitutions.
        let first_catalog = catalog_records(&first.bytes, 8);
        let second_catalog = catalog_records(&second.bytes, 8);
        assert_eq!(&first_catalog[..7], &second_catalog[..7]);
        let actual = catalog_records(bytes, 9);
        assert_eq!(&actual[..8], first_catalog.as_slice());
        let mut other = second_catalog[7].clone();
        assert_eq!(&other[2..6], &7_u32.to_le_bytes());
        assert_eq!(&other[32..42], b"I\0t\0e\0m\0s\0");
        other[2..6].copy_from_slice(&9_u32.to_le_bytes());
        other[32..42].copy_from_slice(b"O\0t\0h\0e\0r\0");
        assert_eq!(actual[8], other);

        // The first table keeps its physical identity. The second table has
        // independently known TD9/DATA10 and map cells3:22/3:23. No property
        // writer, parser or production layout constant supplies expected bytes.
        assert_eq!(&bytes[7 * 4096..9 * 4096], &first.bytes[7 * 4096..9 * 4096]);
        let mut definition = second.bytes[7 * 4096..8 * 4096].to_vec();
        definition[55..59].copy_from_slice(&0x0000_0316_u32.to_le_bytes());
        definition[59..63].copy_from_slice(&0x0000_0317_u32.to_le_bytes());
        assert_eq!(&bytes[9 * 4096..10 * 4096], definition.as_slice());
        let mut data = second.bytes[8 * 4096..9 * 4096].to_vec();
        data[4..8].copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(&bytes[10 * 4096..11 * 4096], data.as_slice());
    }
}

#[test]
fn empty_text_policy_requires_the_explicit_short_text_declaration_for_both_booleans() {
    for scalar in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Currency,
        ScalarType::Decimal,
        ScalarType::Text,
        ScalarType::Binary,
        ScalarType::Guid,
        ScalarType::DateTime,
        ScalarType::DateTimeExtended,
    ] {
        for allow in [false, true] {
            assert!(matches!(
                CreateColumn::new("Value", scalar).with_allow_zero_length(allow),
                Err(CreateDatabaseError::Declaration)
            ));
        }
    }
    for allow in [false, true] {
        let text = CreateColumn::short_text("Value", 8)
            .unwrap()
            .with_allow_zero_length(allow)
            .unwrap();
        assert_eq!(text.allow_zero_length(), Some(allow));
        assert_eq!(text.required(), None);
    }
}
