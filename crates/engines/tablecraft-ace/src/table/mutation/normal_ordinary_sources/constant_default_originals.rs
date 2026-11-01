//! Genuine inert default/counter metadata sources, never a resolver or writer.
use crate::{
    IndexCheckStatus, NativeImage, check_table, inspect_table_properties, inspect_table_schema,
    read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(180);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires the two SHA-pinned inert original constant-default SDK sources"]
fn constant_defaults_are_typed_metadata_and_original_values_remain_explicit() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_DEFAULT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    for (header, length, digest, definitions, data) in [
        (
            5,
            446464,
            "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
            [89_usize, 94, 99],
            [92_usize, 97, 103],
        ),
        (
            6,
            520192,
            "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
            [106, 111, 117],
            [109, 115, 121],
        ),
    ] {
        let raw = fs::read(directory.join(format!("h{header}-default-original.accdb"))).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hash(&raw), digest);
        originals.push((raw, definitions, data));
    }

    for (raw, definitions, data) in originals {
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let source = crate::table::mutation::normal_ordinary::capture(
            &image,
            "Defaults",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = vec![vec![
            Value::Int32(1),
            Value::Int32(-1),
            Value::Text("original".to_owned()),
            Value::Boolean(false),
            Value::Currency(Currency::from_scaled(25000)),
            Value::Null,
        ]];
        assert_eq!(source.table.rows(), expected);
        assert_eq!(source.records.len(), 1);
        assert_eq!(source.indexes.indexes().len(), 2);
        assert_eq!(
            source.table.schema().definition_page() as usize,
            definitions[0]
        );
        assert_eq!(source.records[0].reference, (data[0] as u32) << 8);
        let properties = inspect_table_properties(
            &mut image.reader(),
            SCOPE,
            "Defaults",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(properties.object_id(), source.table.schema().object_id());
        for (field, text) in [
            ("Score", "7"),
            ("Caption", "\"seed\""),
            ("Flag", "True"),
            ("Money", "-1.0001"),
        ] {
            let group = properties
                .groups()
                .iter()
                .find(|group| group.name() == field)
                .unwrap();
            let property = group.get("DefaultValue").unwrap();
            assert_eq!(property.native_type(), 12);
            assert_eq!(property.flags(), 1);
            assert_eq!(property.text().unwrap(), text);
        }
        assert!(
            properties
                .groups()
                .iter()
                .filter(|group| group.name() == "Optional")
                .all(|group| group.get("DefaultValue").is_none())
        );
        assert!(
            properties
                .groups()
                .iter()
                .all(|group| group.get("NewValues").is_none())
        );
        let checked = check_table(
            &mut image.reader(),
            SCOPE,
            "Defaults",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 1);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        for name in ["PrimaryID", "PrimaryAlias"] {
            let rows = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Defaults",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), expected);
        }

        // These tables are metadata/read witnesses only. Flags07 and LAST3/1
        // do not grant Increment mode or any generated INSERT capability.
        for (position, name, last, expected) in [
            (
                1,
                "Numbered",
                3_i32,
                vec![
                    vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
                    vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
                ],
            ),
            (
                2,
                "NonPrimary",
                1,
                vec![vec![Value::Int32(50), Value::Int32(1)]],
            ),
        ] {
            let schema =
                inspect_table_schema(&mut image.reader(), SCOPE, name, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(schema.definition_page() as usize, definitions[position]);
            assert_eq!(schema.columns()[1].flags(), 7);
            assert_eq!(schema.columns()[1].extended_flags(), 0);
            let base = definitions[position] * 4096;
            let actual_last = i32::from_le_bytes(raw[base + 20..base + 24].try_into().unwrap());
            assert_eq!(actual_last, last);
            let properties =
                inspect_table_properties(&mut image.reader(), SCOPE, name, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert!(
                properties
                    .groups()
                    .iter()
                    .all(|group| group.get("NewValues").is_none())
            );
            let rows =
                read_table(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
            assert_eq!(rows.rows(), expected);
        }
        let numbered_data = data[1] * 4096;
        let count = u16::from_le_bytes(
            raw[numbered_data + 12..numbered_data + 14]
                .try_into()
                .unwrap(),
        );
        assert_eq!(count, 3);
        let deleted = u16::from_le_bytes(
            raw[numbered_data + 18..numbered_data + 20]
                .try_into()
                .unwrap(),
        );
        assert_eq!(deleted & 0xc000, 0xc000);
        assert_eq!(image.as_bytes(), raw);
    }
}
