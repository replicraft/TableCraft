//! Read-only complete admission of fourteen original-derived fitting expectations.
use super::super::super::source;
use crate::{
    FileFormat, NativeImage, PropertyGroupKind, header::check, inspect_table_properties,
    read_attachment_column, read_stored_int32_calculation, read_table_projection,
};
use std::{fs::File, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);
const MAX_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
const PAGE_BYTES: usize = 4096;
const CASES: [&str; 7] = [
    "beta-flag",
    "text-noop-flag",
    "null-flag",
    "empty-null-flag",
    "disjoint-rows",
    "all-noops",
    "sql-empty",
];

fn selected_text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::Text(text.to_owned()))
}

// This literal matrix is bound to the independently frozen request inventory.
fn expected_rows(case: &str) -> ([Option<&'static str>; 3], [bool; 3]) {
    match case {
        "beta-flag" => ([Some("beta"), None, Some("omega")], [true, true, false]),
        "text-noop-flag" => ([Some("alpha"), None, Some("omega")], [true, true, false]),
        "null-flag" => ([None, None, Some("omega")], [true, true, false]),
        "empty-null-flag" => (
            [Some("alpha"), Some(""), Some("omega")],
            [false, false, false],
        ),
        "disjoint-rows" => ([Some("beta"), None, Some("omega")], [false, true, true]),
        "all-noops" => ([Some("alpha"), None, Some("omega")], [false, true, false]),
        "sql-empty" => ([Some("alpha"), None, Some("omega")], [false, true, false]),
        _ => unreachable!("closed reviewed cases"),
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit frozen expected directory; read-only Source admission without writer calls"]
fn qualify_genuine_preserved_joint_text_fixed_expectations_before_producer() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_PRESERVED_JOINT_TEXT_FIXED_EXPECTED_DIR")
            .expect("explicit frozen expected directory is required"),
    );
    assert!(directory.is_absolute());
    let mut admitted = 0;
    for (header, format, parent) in [
        (5, FileFormat::Ace2016, 100_usize),
        (6, FileFormat::Ace2019, 118_usize),
    ] {
        for case in CASES {
            let file = format!("h{header}-{case}.accdb");
            let mut input = File::open(directory.join(&file)).unwrap();
            assert!(input.metadata().unwrap().len() <= MAX_IMAGE_BYTES);
            let image = NativeImage::capture(&mut input, SCOPE, &mut budget(), &|| false).unwrap();
            assert_eq!(image.catalog().header().format(), format);
            let mut resources = budget();
            // This actual existing owner enforces the whole preservation policy,
            // including original source/property/metadata/GLOBAL/index/LVAL roles.
            // Neither stage, proof, public preview nor a no-op writer is invoked.
            let qualified =
                source::capture(&image, "PreservedRows", &mut resources, &|| false).unwrap();
            let schema = qualified.table.schema();
            assert_eq!(schema.columns().len(), 8);
            assert_eq!(schema.layout.max_columns, 8);
            assert_eq!(
                schema
                    .columns()
                    .iter()
                    .map(|field| field.name())
                    .collect::<Vec<_>>(),
                [
                    "ID",
                    "Short_Text",
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
            let target = &schema.columns()[1];
            assert_eq!(target.scalar_type(), Some(ScalarType::Text));
            assert_eq!(
                (
                    target.native_type(),
                    target.storage_length(),
                    target.flags(),
                    target.extended_flags()
                ),
                (10, 80, 2, 1)
            );
            assert!(target.is_ordinary_short_text());
            assert!(!target.fixed());
            assert!(!qualified.dependencies.contains(&1));
            assert_eq!(qualified.dependencies, [2]);
            assert!(
                qualified
                    .indexes
                    .indexes()
                    .iter()
                    .all(|index| index.columns().iter().all(|column| column.number() != 1))
            );
            let flag = &schema.columns()[4];
            assert_eq!(flag.name(), "Flag");
            assert_eq!(flag.scalar_type(), Some(ScalarType::Boolean));
            assert_eq!(
                (
                    flag.native_type(),
                    flag.storage_length(),
                    flag.flags(),
                    flag.extended_flags()
                ),
                (1, 0, 3, 0)
            );
            assert!(flag.fixed());
            assert!(!qualified.dependencies.contains(&4));
            assert!(
                qualified
                    .indexes
                    .indexes()
                    .iter()
                    .all(|index| { index.columns().iter().all(|column| column.number() != 4) })
            );
            assert_eq!(qualified.records.len(), 3);
            for (ordinal, (record, length)) in
                qualified.records.iter().zip([121, 66, 73]).enumerate()
            {
                assert_eq!(record.reference, ((parent as u32) << 8) | ordinal as u32);
                assert_eq!(record.data.len(), length);
            }
            let page = &image.as_bytes()[parent * PAGE_BYTES..(parent + 1) * PAGE_BYTES];
            assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 3);
            assert_eq!(u16::from_le_bytes(page[2..4].try_into().unwrap()), 3816);
            for ordinal in 0..3 {
                let at = 14 + ordinal * 2;
                let word = u16::from_le_bytes(page[at..at + 2].try_into().unwrap());
                assert_eq!(word & 0xe000, 0);
                assert_eq!(
                    qualified.records[ordinal].offset,
                    parent * PAGE_BYTES + usize::from(word)
                );
            }
            let rows = read_table_projection(
                &mut image.reader(),
                SCOPE,
                "PreservedRows",
                &["ID", "Short_Text", "Base", "Flag"],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let (texts, flags) = expected_rows(case);
            assert_eq!(
                rows.rows(),
                [
                    vec![
                        Value::Int32(1),
                        selected_text(texts[0]),
                        Value::Int32(7),
                        Value::Boolean(flags[0])
                    ],
                    vec![
                        Value::Int32(2),
                        selected_text(texts[1]),
                        Value::Int32(11),
                        Value::Boolean(flags[1])
                    ],
                    vec![
                        Value::Int32(3),
                        selected_text(texts[2]),
                        Value::Int32(13),
                        Value::Boolean(flags[2])
                    ],
                ]
            );
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
            let target_properties = properties
                .group(PropertyGroupKind::Column, "Short_Text")
                .unwrap();
            assert!(
                !target_properties
                    .get("Required")
                    .unwrap()
                    .boolean()
                    .unwrap()
            );
            assert!(
                target_properties
                    .get("AllowZeroLength")
                    .unwrap()
                    .boolean()
                    .unwrap()
            );
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
            admitted += 1;
        }
    }
    assert_eq!(admitted, 14);
}
