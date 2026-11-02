//! Original-only qualification; no generic creation producer is invoked.
use super::gates;
use crate::{
    FileFormat, NativeImage, check_table, read_table,
    table::mutation::variable_split::metadata::Metadata,
};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_fixed_creation};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit fresh export of independent fixed-scalar originals; no production creator"]
fn export_independent_fixed_creation_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_FIXED_CREATION_SOURCE_DIR")
            .expect("an explicit fresh source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();

    for case in canonical_fixed_creation::CASES {
        let source = canonical_fixed_creation::source(case);
        let path = directory.join(format!("fixed-{}.accdb", case.id()));
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

#[test]
fn qualify_independent_fixed_creation_sources_before_generic_creator() {
    let mut total_rows = 0;
    for case in canonical_fixed_creation::CASES {
        let source = canonical_fixed_creation::source(case);
        let envelope = canonical_creation::source(canonical_creation::Case::Empty);
        assert_eq!(source.bytes.len(), 36_864);
        for page in [0, 1, 2, 4, 5, 6] {
            assert_eq!(
                &source.bytes[page * 4096..(page + 1) * 4096],
                &envelope.bytes[page * 4096..(page + 1) * 4096]
            );
        }
        let expected_free = 4096 - 14 - source.rows.len() * 65;
        let mut expected_maps = envelope.bytes[3 * 4096..4 * 4096].to_vec();
        // Independent literal packing law: candidate row 7 is the eighth 13-byte
        // record, and page 8 occupies byte 1 of its eight-byte bitmap.
        expected_maps[4096 - 8 * 13 + 5 + 1] = if expected_free >= 65 { 1 } else { 0 };
        assert_eq!(&source.bytes[3 * 4096..4 * 4096], expected_maps);

        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(image.catalog().header().format(), FileFormat::Ace2016);
        assert_eq!(image.catalog().catalog().objects().len(), 8);
        let mut resources = budget();
        let snapshot = read_table(
            &mut image.reader(),
            SCOPE,
            source.table,
            &mut resources,
            &|| false,
        )
        .unwrap();
        assert_eq!(snapshot.rows(), source.rows);
        assert_eq!(snapshot.schema().columns().len(), 10);
        assert_eq!(snapshot.schema().layout.fixed_end, 61);
        assert_eq!(snapshot.schema().layout.variable_slots, 0);
        let expected_types = [
            ScalarType::Boolean,
            ScalarType::Byte,
            ScalarType::Int16,
            ScalarType::Int32,
            ScalarType::Int64,
            ScalarType::Currency,
            ScalarType::Single,
            ScalarType::Double,
            ScalarType::DateTime,
            ScalarType::Guid,
        ];
        for ((column, declared), expected) in snapshot
            .schema()
            .columns()
            .iter()
            .zip(source.columns)
            .zip(expected_types)
        {
            assert_eq!(column.name(), declared.name);
            assert_eq!(column.native_type(), declared.native_type);
            assert_eq!(column.storage_length(), usize::from(declared.width));
            assert_eq!(column.field.offset, usize::from(declared.fixed_offset));
            assert_eq!(column.scalar_type(), Some(expected));
            assert_eq!((column.flags(), column.extended_flags()), (3, 0));
        }

        let checked = check_table(
            &mut image.reader(),
            SCOPE,
            source.table,
            &mut resources,
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), source.rows.len());
        assert!(checked.indexes().is_empty());
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

        let page = &source.bytes[8 * 4096..9 * 4096];
        assert_eq!(
            u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize,
            expected_free
        );
        assert_eq!(image.as_bytes(), source.bytes);
        total_rows += source.rows.len();
    }
    assert_eq!(total_rows, 70);
}

mod hash_anchor;
