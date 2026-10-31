//! Expected-only genuine joint admission; no constructor or mutation call.
use super::super::source;
use crate::{
    FileFormat, NativeImage, PropertyGroupKind, inspect_table_properties, read_attachment_column,
    read_stored_int32_calculation, read_table_projection,
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
const PROFILES: [&str; 6] = [
    "same-row",
    "promote-null",
    "clear-values",
    "overlap-rows",
    "mixed-noops",
    "sql-empty",
];
// Must match the separately raw-bound original Date_Field on ID 3. This is
// retained carrier authority, not a calendar-to-DATE conversion.
const ORIGINAL_THIRD_DATE_BITS: u64 = 0x40e6_24e0_0000_0000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn date(bits: u64) -> Value {
    Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
}

fn currency(scaled: i64) -> Value {
    Value::Currency(Currency::from_scaled(scaled))
}

fn pattern() -> Vec<u8> {
    (0..8192)
        .map(|index| (index * 37 + index / 251) as u8)
        .collect()
}

fn rows(profile: &str) -> [Vec<Value>; 3] {
    let mut rows = [
        vec![
            Value::Int32(1),
            Value::Int64(1000),
            date(0),
            date(0x3fe0_0000_0000_0000),
            currency(12345),
            Value::Int32(7),
            Value::Boolean(false),
            Value::Text("preserved memo é🦀".into()),
            Value::Binary(pattern()),
        ],
        vec![
            Value::Int32(2),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Int32(11),
            Value::Boolean(true),
            Value::Null,
            Value::Binary(Vec::new()),
        ],
        vec![
            Value::Int32(3),
            Value::Int64(-2000),
            date(ORIGINAL_THIRD_DATE_BITS),
            date(0x3fd0_0000_0000_0000),
            currency(-20001),
            Value::Int32(13),
            Value::Boolean(false),
            Value::Text(String::new()),
            Value::Null,
        ],
    ];
    match profile {
        "same-row" => {
            rows[0][1] = Value::Int64(i64::MIN);
            rows[0][2] = date(0x8000_0000_0000_0000);
            rows[0][3] = date(0x3fd0_0000_0000_0000);
            rows[0][4] = currency(i64::MIN);
            rows[0][6] = Value::Boolean(true);
        }
        "promote-null" => {
            rows[1][1] = Value::Int64(i64::MAX);
            rows[1][2] = date(0x3fd0_0000_0000_0000);
            rows[1][3] = date(0x3fe0_0000_0000_0000);
            rows[1][4] = currency(i64::MAX);
            rows[1][6] = Value::Boolean(false);
        }
        "clear-values" => {
            for value in &mut rows[0][1..5] {
                *value = Value::Null;
            }
            rows[0][6] = Value::Boolean(true);
        }
        "overlap-rows" => {
            rows[0][1] = Value::Int64(-101);
            rows[0][3] = date(0x3fd0_0000_0000_0000);
            rows[0][4] = Value::Null;
            rows[0][6] = Value::Boolean(true);
            rows[1][1] = Value::Int64(i64::MAX);
            rows[1][2] = date(0x3fd0_0000_0000_0000);
            rows[1][4] = currency(-10001);
            rows[1][6] = Value::Boolean(false);
            rows[2][2] = Value::Null;
            rows[2][3] = date(0x3fe0_0000_0000_0000);
            rows[2][6] = Value::Boolean(true);
        }
        "mixed-noops" | "sql-empty" => {}
        _ => unreachable!("the expected gate has six static profile names"),
    }
    rows
}

fn qualify(path: &Path, format: FileFormat, profile: &str) {
    let mut input = File::open(path).unwrap();
    assert!(input.metadata().unwrap().len() <= SOURCE_BYTES);
    let image = NativeImage::capture(&mut input, SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(image.catalog().header().format(), format);
    let qualified = source::capture(&image, "PreservedRows", &mut budget(), &|| false).unwrap();
    let schema = qualified.table.schema();
    assert_eq!(schema.layout.max_columns, 11);
    assert_eq!(schema.columns().len(), 11);
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
        ],
    );
    for (number, field) in schema.columns().iter().enumerate() {
        assert_eq!(field.number(), number);
    }
    for (number, native, kind, width) in [
        (1, 19, ScalarType::Int64, 8),
        (2, 8, ScalarType::DateTime, 8),
        (3, 8, ScalarType::DateTime, 8),
        (4, 5, ScalarType::Currency, 8),
        (7, 1, ScalarType::Boolean, 0),
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
            (native, width, 3, 0)
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
    let mut keys = qualified
        .indexes
        .indexes()
        .iter()
        .map(|index| {
            assert_eq!(index.columns().len(), 1);
            index.columns()[0].number()
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, [0, 5]);
    let projected = read_table_projection(
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
            "Memo",
            "Blob",
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.rows(), rows(profile));
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
    for (parent, child, identity, name, file_type, encoded_length) in [
        (0, 0, 1, "empty.txt", "txt", 31),
        (0, 1, 2, "payload.zip", "zip", 8220),
        (1, 0, 3, "unicode.txt", "txt", 38),
    ] {
        let item = attachments
            .attachments(parent)
            .unwrap()
            .nth(child)
            .unwrap()
            .unwrap();
        assert_eq!(item.identity(), identity);
        assert_eq!(item.file_url(), Some(""));
        assert_eq!(item.file_name(), Some(name));
        assert_eq!(item.file_type(), Some(file_type));
        assert_eq!(item.file_flags(), Some(0));
        assert_eq!(item.encoded_file_data().unwrap().len(), encoded_length);
    }
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "PreservedRows",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for name in ["Large_Number", "Date_Field", "Time_Field", "Currency_Field"] {
        assert!(
            !properties
                .group(PropertyGroupKind::Column, name)
                .unwrap()
                .get("Required")
                .unwrap()
                .boolean()
                .unwrap()
        );
    }
    let publish = properties
        .groups()
        .iter()
        .filter(|group| group.kind() == PropertyGroupKind::Table)
        .flat_map(|group| group.properties())
        .filter(|property| property.name() == "PublishToWeb")
        .collect::<Vec<_>>();
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
#[ignore = "Explicit twelve expected-image SOURCE admissions after frozen construction, before writer"]
fn qualify_genuine_preserved_joint_fixed_expectations_before_producer() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_PRESERVED_JOINT_FIXED_EXPECTED_DIR")
            .expect("explicit expected-image directory required"),
    );
    assert!(directory.is_absolute());
    let mut admitted = 0;
    for (header, format) in [(5, FileFormat::Ace2016), (6, FileFormat::Ace2019)] {
        for profile in PROFILES {
            let path = directory.join(format!("h{header}-{profile}.accdb"));
            qualify(&path, format, profile);
            admitted += 1;
        }
    }
    assert_eq!(admitted, 12);
}
