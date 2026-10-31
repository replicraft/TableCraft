//! Original-only admission of the frozen public-builder scalar corpus.
use super::super::source;
use crate::{
    FileFormat, NativeImage, PropertyGroupKind, header::check, inspect_table_properties,
    read_attachment_column, read_stored_int32_calculation, read_table_projection,
};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);
const SOURCE_BYTES: u64 = 2 * 1024 * 1024;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn date(days: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(days).unwrap())
}

#[test]
#[ignore = "Explicit original-only admission after frozen creation; no expected constructor or writer"]
fn qualify_genuine_preserved_fixed_scalar_originals_before_expected_or_producer() {
    for (variable, format) in [
        (
            "TABLECRAFT_PRESERVED_FIXED_SCALAR_H5_SOURCE",
            FileFormat::Ace2016,
        ),
        (
            "TABLECRAFT_PRESERVED_FIXED_SCALAR_H6_SOURCE",
            FileFormat::Ace2019,
        ),
    ] {
        let path =
            PathBuf::from(std::env::var_os(variable).expect("explicit source path required"));
        assert!(path.is_absolute());
        qualify(&path, format, None);
    }
}

fn qualify(path: &Path, format: FileFormat, replacement: Option<(i32, usize, Value)>) {
    let mut input = File::open(path).unwrap();
    assert!(input.metadata().unwrap().len() <= SOURCE_BYTES);
    let image = NativeImage::capture(&mut input, SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(image.catalog().header().format(), format);
    let mut resources = budget();
    let qualified = source::capture(&image, "PreservedRows", &mut resources, &|| false).unwrap();
    let schema = qualified.table.schema();
    assert_eq!(schema.columns().len(), 11);
    assert_eq!(schema.layout.max_columns, 11);
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>(),
        [
            "ID",
            "Large_Number",
            "Date_Field",
            "Time_Field",
            "Currency_Field",
            "Base",
            "Cached",
            "Flag",
            "Memo",
            "Blob",
            "Files"
        ]
    );
    for (position, field) in schema.columns().iter().enumerate() {
        check(&mut resources, &|| false).unwrap();
        assert_eq!(field.number(), position);
    }
    for (number, native, kind) in [
        (1, 19, ScalarType::Int64),
        (2, 8, ScalarType::DateTime),
        (3, 8, ScalarType::DateTime),
        (4, 5, ScalarType::Currency),
    ] {
        let target = &schema.columns()[number];
        assert_eq!(target.scalar_type(), Some(kind));
        assert_eq!(
            (
                target.native_type(),
                target.storage_length(),
                target.flags(),
                target.extended_flags()
            ),
            (native, 8, 3, 0)
        );
        assert!(target.fixed());
        assert!(!qualified.dependencies.contains(&number));
        assert!(qualified.indexes.indexes().iter().all(|index| {
            index
                .columns()
                .iter()
                .all(|column| column.number() != number)
        }));
    }
    assert_eq!(qualified.dependencies, [5]);
    assert_eq!(qualified.records.len(), 3);
    let rows = read_table_projection(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        &[
            "ID",
            "Large_Number",
            "Date_Field",
            "Time_Field",
            "Currency_Field",
            "Base",
            "Flag",
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut expected_rows = [
        vec![
            Value::Int32(1),
            Value::Int64(1000),
            date(0.0),
            date(0.5),
            Value::Currency(Currency::from_scaled(12345)),
            Value::Int32(7),
            Value::Boolean(false),
        ],
        vec![
            Value::Int32(2),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Int32(11),
            Value::Boolean(true),
        ],
        vec![
            Value::Int32(3),
            Value::Int64(-2000),
            date(45351.0),
            date(0.25),
            Value::Currency(Currency::from_scaled(-20001)),
            Value::Int32(13),
            Value::Boolean(false),
        ],
    ];
    if let Some((identifier, column, value)) = replacement {
        let row = expected_rows
            .iter_mut()
            .find(|row| row[0] == Value::Int32(identifier))
            .unwrap();
        row[column] = value;
    }
    assert_eq!(rows.rows(), expected_rows);
    let payloads = read_table_projection(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        &["Memo", "Blob"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let pattern = (0..8192)
        .map(|index| (index * 37 + index / 251) as u8)
        .collect::<Vec<_>>();
    assert_eq!(
        payloads.rows(),
        [
            vec![
                Value::Text("preserved memo é🦀".into()),
                Value::Binary(pattern)
            ],
            vec![Value::Null, Value::Binary(Vec::new())],
            vec![Value::Text(String::new()), Value::Null],
        ]
    );
    let cached = read_stored_int32_calculation(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        "Cached",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(cached.values(), &[Some(999), Some(-333), Some(0)]);
    let attachments = read_attachment_column(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        "Files",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        attachments
            .links()
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for name in ["Large_Number", "Date_Field", "Time_Field", "Currency_Field"] {
        let properties = properties.group(PropertyGroupKind::Column, name).unwrap();
        assert!(!properties.get("Required").unwrap().boolean().unwrap());
    }
    let publish: Vec<_> = properties
        .groups()
        .iter()
        .filter(|group| group.kind() == PropertyGroupKind::Table)
        .flat_map(|group| group.properties())
        .filter(|property| property.name() == "PublishToWeb")
        .collect();
    assert_eq!(publish.len(), 1);
    assert_eq!(
        (
            publish[0].native_type(),
            publish[0].flags(),
            publish[0].raw_value()
        ),
        (2, 0, &[0][..])
    );
    let sentinel = read_table_projection(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &["ID", "Marker", "Bytes"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        sentinel.rows(),
        [vec![
            Value::Int32(23),
            Value::Text("unchanged sentinel".into()),
            Value::Binary(vec![199, 0, 255])
        ]]
    );
}

#[test]
#[ignore = "Explicit expected-image admission after pure construction, before any writer"]
fn qualify_genuine_preserved_fixed_scalar_expected_images_before_producer() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_PRESERVED_FIXED_SCALAR_EXPECTED_DIR")
            .expect("explicit expected-image directory required"),
    );
    assert!(directory.is_absolute());
    let mut admitted = 0;
    for (header, format) in [(5, FileFormat::Ace2016), (6, FileFormat::Ace2019)] {
        for family in ["int64", "currency", "date"] {
            for recipe in ["change", "null-to-value", "value-to-null", "null-noop"] {
                let (column, changed, inserted) = match family {
                    "int64" => (1, Value::Int64(i64::MIN), Value::Int64(i64::MAX)),
                    "currency" => (
                        4,
                        Value::Currency(Currency::from_scaled(i64::MIN)),
                        Value::Currency(Currency::from_scaled(i64::MAX)),
                    ),
                    "date" => (
                        2,
                        Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
                        Value::DateTime(LegacyDateTime::from_bits(0x3fd0_0000_0000_0000).unwrap()),
                    ),
                    _ => unreachable!(),
                };
                let (identifier, value) = match recipe {
                    "change" => (1, changed),
                    "null-to-value" => (2, inserted),
                    "value-to-null" => (1, Value::Null),
                    "null-noop" => (2, Value::Null),
                    _ => unreachable!(),
                };
                let path = directory
                    .join("expected")
                    .join(format!("h{header}-{family}-{recipe}.accdb"));
                qualify(&path, format, Some((identifier, column, value)));
                admitted += 1;
            }
        }
    }
    assert_eq!(admitted, 24);
}
