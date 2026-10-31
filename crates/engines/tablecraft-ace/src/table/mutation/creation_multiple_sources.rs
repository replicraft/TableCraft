//! Read-only multi-table SOURCE admission; no creator or write preview is invoked.
mod hash_anchor;
use super::gates;
use crate::{NativeImage, read_table, table::mutation::variable_split::metadata::Metadata};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{canonical_creation, canonical_multiple_creation};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn u16_at(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[test]
fn qualify_multiple_table_originals_before_any_multiple_table_creator() {
    let base = canonical_creation::source(canonical_creation::Case::Empty);
    let mut total_tables = 0;
    let mut total_rows = 0;
    for case in canonical_multiple_creation::CASES {
        let source = canonical_multiple_creation::source(case);
        assert_eq!(source.case, case);
        assert_eq!(source.bytes.len(), (7 + 2 * source.tables.len()) * 4096);
        for number in [0, 1, 5, 6] {
            assert_eq!(
                &source.bytes[number * 4096..(number + 1) * 4096],
                &base.bytes[number * 4096..(number + 1) * 4096]
            );
        }
        for offset in 0..4096 {
            if !(16..20).contains(&offset) {
                assert_eq!(
                    source.bytes[2 * 4096 + offset],
                    base.bytes[2 * 4096 + offset]
                );
            }
        }
        assert_eq!(
            u32_at(&source.bytes, 2 * 4096 + 16),
            (7 + source.tables.len()) as u32
        );
        assert_eq!(source.map_members.len(), 16 + 2 * (source.tables.len() - 1));
        let maps = &source.bytes[3 * 4096..4 * 4096];
        assert_eq!(u32_at(maps, 4), 0);
        assert_eq!(u16_at(maps, 12), source.map_members.len());
        assert_eq!(u16_at(maps, 2), 4096 - 14 - 15 * source.map_members.len());
        let mut end = 4096;
        for (slot, member) in source.map_members.iter().enumerate() {
            let start = u16_at(maps, 14 + 2 * slot);
            assert_eq!(end - start, 13);
            let mut expected = [0; 13];
            if let Some(number) = member {
                expected[5 + *number as usize / 8] = 1 << (*number % 8);
            }
            assert_eq!(&maps[start..end], &expected);
            end = start;
        }
        assert!(
            maps[14 + 2 * source.map_members.len()..end]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(source.map_members[0], Some(4));
        assert_eq!(source.map_members[4], Some(4));
        for slot in [1, 2, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(source.map_members[slot], None);
        }

        let mut resources = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut resources,
            &|| false,
        )
        .unwrap();
        for (index, table) in source.tables.iter().enumerate() {
            assert_eq!(table.definition_page, (7 + 2 * index) as u32);
            assert_eq!(table.data_page, (8 + 2 * index) as u32);
            let pair = if index == 0 {
                (3, 7)
            } else {
                (16 + 2 * (index - 1), 17 + 2 * (index - 1))
            };
            assert_eq!(usize::from(table.owned_slot), pair.0);
            assert_eq!(usize::from(table.candidate_slot), pair.1);
            let definition = &source.bytes[table.definition_page as usize * 4096
                ..(table.definition_page as usize + 1) * 4096];
            let data = &source.bytes
                [table.data_page as usize * 4096..(table.data_page as usize + 1) * 4096];
            assert_eq!(definition[40], 0x4e);
            assert_eq!(
                u32_at(definition, 55),
                (3 << 8) | u32::from(table.owned_slot)
            );
            assert_eq!(
                u32_at(definition, 59),
                (3 << 8) | u32::from(table.candidate_slot)
            );
            assert_eq!(u32_at(data, 4), table.definition_page);
            assert_eq!(u16_at(data, 2), usize::from(table.free_bytes));
            assert_eq!(source.map_members[pair.0], Some(table.data_page));
            assert_eq!(
                source.map_members[pair.1],
                table.candidate.then_some(table.data_page)
            );
            for offset in 0..4096 {
                if !(55..63).contains(&offset) {
                    assert_eq!(definition[offset], table.original_definition[offset]);
                }
                if !(4..8).contains(&offset) {
                    assert_eq!(data[offset], table.original_data[offset]);
                }
            }

            let read = read_table(
                &mut image.reader(),
                SCOPE,
                &table.name,
                &mut resources,
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), table.rows);
            assert_eq!(read.schema().definition_page(), table.definition_page);
            assert_eq!(read.schema().columns().len(), table.columns.len());
            for (position, (actual, expected)) in read
                .schema()
                .columns()
                .iter()
                .zip(&table.columns)
                .enumerate()
            {
                assert_eq!(actual.number(), position);
                assert_eq!(actual.name(), expected.name);
                assert_eq!(actual.native_type(), expected.native_type);
                assert_eq!(actual.storage_length(), usize::from(expected.width));
                assert_eq!(actual.fixed(), expected.fixed_offset.is_some());
                assert_eq!(
                    actual.field.offset,
                    usize::from(expected.fixed_offset.unwrap_or(0))
                );
                assert_eq!(actual.field.variable, usize::from(expected.variable_index));
                assert_eq!(
                    actual.flags(),
                    if expected.fixed_offset.is_some() {
                        3
                    } else {
                        2
                    }
                );
                assert_eq!(actual.extended_flags(), 0);
                assert_eq!(actual.decimal_parameters(), expected.decimal);
            }
            let structural = gates::before(&image, &table.name, &mut resources, &|| false).unwrap();
            let metadata = Metadata::capture_for_insert(
                &image,
                structural.schema(),
                structural.indexes(),
                &mut resources,
                &|| false,
            )
            .unwrap();
            metadata
                .require_source(&image, table.definition_page)
                .unwrap();
            total_tables += 1;
            total_rows += table.rows.len();
        }
        assert_eq!(image.as_bytes(), source.bytes);
    }
    assert_eq!((total_tables, total_rows), (22, 63));
}

#[test]
#[ignore = "Explicit fresh export of independently qualified multi-table originals"]
fn export_multiple_table_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_MULTIPLE_TABLE_SOURCE_DIR")
            .expect("an explicit fresh multiple-table source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    for case in canonical_multiple_creation::CASES {
        let source = canonical_multiple_creation::source(case);
        let path = directory.join(format!("multiple-tables-h5-{}.accdb", case.id()));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        output.write_all(&source.bytes).unwrap();
        output.flush().unwrap();
        assert_eq!(fs::read(path).unwrap(), source.bytes);
    }
}
