//! Original-only catalog/definition name qualification before creator repair.
use super::gates;
use crate::{NativeImage, read_table, table::mutation::variable_split::metadata::Metadata};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{canonical_catalog_names, canonical_creation};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit fresh export of independent catalog-name sources before creator repair"]
fn export_catalog_name_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_CATALOG_NAME_SOURCE_DIR")
            .expect("an explicit fresh catalog-name source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();

    for case in canonical_catalog_names::CASES {
        let source = canonical_catalog_names::source(case);
        let path = directory.join(format!("catalog-name-h5-{}.accdb", case.id()));
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
fn qualify_catalog_name_sources_before_creator_name_repair() {
    for case in canonical_catalog_names::CASES {
        let source = canonical_catalog_names::source(case);
        let envelope = canonical_creation::source(canonical_creation::Case::Empty);
        assert_eq!(source.bytes.len(), envelope.bytes.len());
        for offset in 0..source.bytes.len() {
            let catalog = (4 * 4096..5 * 4096).contains(&offset);
            let column = case == canonical_catalog_names::Case::ColumnRaw
                && (7 * 4096 + 121..7 * 4096 + 131).contains(&offset);
            if !catalog && !column {
                assert_eq!(source.bytes[offset], envelope.bytes[offset]);
            }
        }
        let catalog = &source.bytes[4 * 4096..5 * 4096];
        let original_catalog = &envelope.bytes[4 * 4096..5 * 4096];
        assert_eq!(&catalog[14..28], &original_catalog[14..28]);
        let unchanged_start =
            u16::from_le_bytes(original_catalog[26..28].try_into().unwrap()) as usize;
        assert_eq!(
            &catalog[unchanged_start..],
            &original_catalog[unchanged_start..]
        );
        let start = u16::from_le_bytes(catalog[28..30].try_into().unwrap()) as usize;
        let end = u16::from_le_bytes(catalog[26..28].try_into().unwrap()) as usize;
        let user = &catalog[start..end];
        assert_eq!(&user[32..user.len() - 24], source.catalog_name_payload);

        let expected_units = match case {
            canonical_catalog_names::Case::TableBom => 1,
            canonical_catalog_names::Case::TableMixed => 7,
            canonical_catalog_names::Case::TableBoundary => 64,
            canonical_catalog_names::Case::ColumnRaw => 5,
        };
        assert_eq!(source.table.encode_utf16().count(), expected_units);
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let objects = image.catalog().catalog().objects();
        assert_eq!(objects.len(), 8);
        assert_eq!(objects[7].name().as_str(), source.table);
        let mut resources = budget();
        let read = read_table(
            &mut image.reader(),
            SCOPE,
            &source.table,
            &mut resources,
            &|| false,
        )
        .unwrap();
        assert!(read.rows().is_empty());
        assert_eq!(read.schema().columns().len(), 2);
        for (field, name) in read.schema().columns().iter().zip(&source.columns) {
            assert_eq!(field.name(), name);
            assert_eq!(
                (
                    field.native_type(),
                    field.flags(),
                    field.extended_flags(),
                    field.storage_length()
                ),
                (4, 3, 0, 4)
            );
        }
        let structural = gates::before(&image, &source.table, &mut resources, &|| false).unwrap();
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

mod hash_anchor;
