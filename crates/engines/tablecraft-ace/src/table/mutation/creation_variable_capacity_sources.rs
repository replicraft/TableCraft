//! Read-only admission of literal variable candidate-map boundary originals.
use super::gates;
use crate::{NativeImage, read_table, table::mutation::variable_split::metadata::Metadata};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_variable_capacity_creation, canonical_variable_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_variable_capacity_originals_before_candidate_map_policy() {
    let empty = canonical_variable_creation::source(canonical_variable_creation::Case::Empty);
    for case in canonical_variable_capacity_creation::CASES {
        let original = canonical_variable_capacity_creation::source(case);
        let source = &original.source;
        assert_eq!(source.bytes.len(), 36_864);
        // The complete complement is protected, including catalog/system/LVAL
        // metadata, GLOBAL membership, unused pages and every other map entry.
        for (offset, (before, after)) in empty.bytes.iter().zip(&source.bytes).enumerate() {
            let data = (8 * 4096..9 * 4096).contains(&offset);
            let row_count = (7 * 4096 + 16..7 * 4096 + 20).contains(&offset);
            let candidate_record = u16::from_le_bytes(
                empty.bytes[3 * 4096 + 28..3 * 4096 + 30]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let candidate_byte = offset == 3 * 4096 + candidate_record + 6;
            if !data && !row_count && !candidate_byte {
                assert_eq!(before, after, "case {} offset {offset}", case.id());
            }
        }
        let data = &source.bytes[8 * 4096..9 * 4096];
        assert_eq!(
            u16::from_le_bytes(data[2..4].try_into().unwrap()),
            original.free_bytes
        );
        assert_eq!(source.rows.len(), if original.candidate { 57 } else { 58 });
        assert_eq!(original.candidate, original.free_bytes >= 19);
        let maps = &source.bytes[3 * 4096..4 * 4096];
        for (slot, expected) in [(3, 1), (7, u8::from(original.candidate))] {
            let index = 14 + 2 * slot;
            let start = u16::from_le_bytes(maps[index..index + 2].try_into().unwrap()) as usize;
            assert_eq!(maps[start + 6], expected);
        }
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let read = read_table(
            &mut image.reader(),
            SCOPE,
            source.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(read.rows(), source.rows);
        assert_eq!(read.schema().columns().len(), 5);
        assert_eq!(read.schema().layout.fixed_end, 6);
        assert_eq!(read.schema().layout.variable_slots, 3);
        for (field, literal) in read.schema().columns().iter().zip(source.columns) {
            assert_eq!(field.name(), literal.name);
            assert_eq!(field.native_type(), literal.native_type);
            assert_eq!(field.storage_length(), usize::from(literal.width));
            assert_eq!(
                field.flags(),
                if literal.fixed_offset.is_some() { 3 } else { 2 }
            );
            assert_eq!(field.extended_flags(), 0);
        }
        assert_eq!(
            read.schema().columns()[3].decimal_parameters(),
            Some((28, 4))
        );
        let mut resources = budget();
        let structural = gates::before(&image, source.table, &mut resources, &|| false).unwrap();
        let metadata = Metadata::capture_for_insert(
            &image,
            structural.schema(),
            structural.indexes(),
            &mut resources,
            &|| false,
        )
        .unwrap();
        metadata
            .require_source(&image, structural.schema().definition_page())
            .unwrap();
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

#[test]
#[ignore = "Explicit fresh export of independently qualified variable capacity sources"]
fn export_variable_capacity_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_VARIABLE_CAPACITY_SOURCE_DIR")
            .expect("an explicit fresh variable-capacity source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();

    for case in canonical_variable_capacity_creation::CASES {
        let original = canonical_variable_capacity_creation::source(case);
        let path = directory.join(format!("variable-capacity-h5-{}.accdb", case.id()));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        output.write_all(&original.source.bytes).unwrap();
        output.flush().unwrap();
        assert_eq!(fs::read(path).unwrap(), original.source.bytes);
    }
}

mod hash_anchor;
