//! Explicit read-only admission of frozen public-builder originals, never a preview.
use super::super::source;
use crate::{
    FileFormat, NativeImage, PropertyGroupKind, header::check, inspect_table_properties,
    read_attachment_column, read_stored_int32_calculation, read_table_projection,
};
use std::{fs::File, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);
const SOURCE_BYTES: u64 = 2 * 1024 * 1024;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit finite SOURCE admission after frozen public-builder creation; no writer calls"]
fn qualify_genuine_preserved_short_text_originals_before_expected_or_producer() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_PRESERVED_SHORT_TEXT_SOURCE_DIR")
            .expect("explicit frozen source directory is required"),
    );
    for (file, format) in [
        ("h5.accdb", FileFormat::Ace2016),
        ("h6.accdb", FileFormat::Ace2019),
    ] {
        let mut input = File::open(directory.join(file)).unwrap();
        assert!(input.metadata().unwrap().len() <= SOURCE_BYTES);
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
        assert_eq!(qualified.records.len(), 3);
        let rows = read_table_projection(
            &mut image.reader(),
            SCOPE,
            "PreservedRows",
            &["ID", "Short_Text", "Base", "Flag"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            [
                vec![
                    Value::Int32(1),
                    Value::Text("alpha".into()),
                    Value::Int32(7),
                    Value::Boolean(false)
                ],
                vec![
                    Value::Int32(2),
                    Value::Null,
                    Value::Int32(11),
                    Value::Boolean(true)
                ],
                vec![
                    Value::Int32(3),
                    Value::Text("omega".into()),
                    Value::Int32(13),
                    Value::Boolean(false)
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
    }
}
