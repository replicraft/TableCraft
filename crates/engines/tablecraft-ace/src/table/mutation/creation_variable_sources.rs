//! Original-only variable SOURCE qualification before producer admission.
use super::gates;
use crate::{NativeImage, read_table, table::mutation::variable_split::metadata::Metadata};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::{canonical_creation, canonical_variable_creation};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit fresh export of independent variable sources and reader control"]
fn export_variable_creation_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_VARIABLE_CREATION_SOURCE_DIR")
            .expect("an explicit fresh variable-source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();

    let sources = canonical_variable_creation::CASES
        .into_iter()
        .map(|case| (case.id(), canonical_variable_creation::source(case)))
        .chain(std::iter::once((
            "raw-bom-alias",
            canonical_variable_creation::raw_bom_alias_source(),
        )));
    for (name, source) in sources {
        let path = directory.join(format!("variable-h5-{name}.accdb"));
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

fn qualify(source: canonical_variable_creation::Source) {
    let envelope = canonical_creation::source(canonical_creation::Case::Empty);
    assert_eq!(source.bytes.len(), 36_864);
    for page in 0..7 {
        assert_eq!(
            &source.bytes[page * 4096..(page + 1) * 4096],
            &envelope.bytes[page * 4096..(page + 1) * 4096]
        );
    }
    let image = NativeImage::capture(
        &mut Cursor::new(&source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut resources = budget();
    let read = read_table(
        &mut image.reader(),
        SCOPE,
        source.table,
        &mut resources,
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows(), source.rows);
    assert_eq!(read.schema().columns().len(), 5);
    assert_eq!(read.schema().layout.variable_slots, 3);
    assert_eq!(read.schema().layout.fixed_end, 6);
    for ((field, expected), scalar) in read.schema().columns().iter().zip(source.columns).zip([
        ScalarType::Int32,
        ScalarType::Text,
        ScalarType::Binary,
        ScalarType::Decimal,
        ScalarType::Boolean,
    ]) {
        assert_eq!(field.name(), expected.name);
        assert_eq!(field.native_type(), expected.native_type);
        assert_eq!(field.scalar_type(), Some(scalar));
        assert_eq!(field.storage_length(), usize::from(expected.width));
        assert_eq!(field.field.variable, usize::from(expected.variable_index));
        assert_eq!(field.fixed(), expected.fixed_offset.is_some());
        assert_eq!(
            field.flags(),
            if expected.fixed_offset.is_some() {
                3
            } else {
                2
            }
        );
        assert_eq!(field.extended_flags(), 0);
    }
    assert_eq!(
        read.schema().columns()[3].decimal_parameters(),
        Some((28, 4))
    );
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

#[test]
fn qualify_variable_creation_sources_before_any_variable_producer() {
    let mut rows = 0;
    for case in canonical_variable_creation::CASES {
        let source = canonical_variable_creation::source(case);
        rows += source.rows.len();
        if case == canonical_variable_creation::Case::EscapedBom {
            let page = &source.bytes[8 * 4096..9 * 4096];
            let end = u16::from_le_bytes(page[14..16].try_into().unwrap()) as usize;
            let start = u16::from_le_bytes(page[16..18].try_into().unwrap()) as usize;
            let record = &page[start..end];
            let binary_start = u16::from_le_bytes(
                record[record.len() - 7..record.len() - 5]
                    .try_into()
                    .unwrap(),
            );
            let text_start = u16::from_le_bytes(
                record[record.len() - 5..record.len() - 3]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(binary_start - text_start, 19);
            assert_eq!(source.columns[1].width, 16);
        }
        qualify(source);
    }
    assert_eq!(rows, 12);
}

#[test]
fn raw_bom_alias_is_a_reader_observation_not_a_creation_expectation() {
    let source = canonical_variable_creation::raw_bom_alias_source();
    assert_eq!(source.rows[0][1], Value::Text(String::new()));
    assert_ne!(source.rows[0][1], Value::Text("\u{feff}".into()));
    // A marker-only payload aliases empty text in the actual reader. The
    // positive BOM source instead supplies an independently authored escape.
    qualify(source);
}

mod hash_anchor;
