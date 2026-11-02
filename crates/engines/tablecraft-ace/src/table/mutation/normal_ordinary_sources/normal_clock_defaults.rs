//! Read-only original-presence clock sources; no resolver, clock or writer.
use crate::{
    NativeImage, PropertyGroupKind, check_table, check_table_constraints, inspect_table_properties,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(252);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn record(identity: i32, bits: Option<u64>) -> Vec<u8> {
    let mut raw = Vec::from([2, 0]);
    raw.extend_from_slice(&identity.to_le_bytes());
    raw.extend_from_slice(&bits.unwrap_or(0).to_le_bytes());
    raw.push(if bits.is_some() { 3 } else { 1 });
    raw
}

fn check(raw: &[u8], expression: &str, added: &[(i32, Option<u64>)]) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let mut rows = vec![vec![
        Value::Int32(1),
        Value::DateTime(LegacyDateTime::from_bits(0x3ff4000000000000).unwrap()),
    ]];
    let mut records = vec![record(1, Some(0x3ff4000000000000))];
    for &(identity, bits) in added {
        rows.push(vec![
            Value::Int32(identity),
            bits.map_or(Value::Null, |bits| {
                Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
            }),
        ]);
        records.push(record(identity, bits));
    }
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), rows);
    assert_eq!(table.schema().definition_page(), 7);
    let fields = table
        .schema()
        .columns()
        .iter()
        .map(|column| {
            (
                column.name(),
                column.native_type(),
                column.flags(),
                column.storage_length(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(fields, [("Id", 4, 3, 4), ("When", 8, 3, 8)]);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert!(checked.indexes().is_empty());
    assert!(
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap()
        .relationships()
        .is_empty()
    );
    let properties =
        inspect_table_properties(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
    assert_eq!(properties.raw_bytes().len(), 64 + 2 * expression.len());
    assert_eq!(properties.groups().len(), 1);
    let group = &properties.groups()[0];
    assert_eq!(group.kind(), PropertyGroupKind::Column);
    assert_eq!(group.name(), "When");
    assert_eq!(group.properties().len(), 1);
    let property = group.get("DefaultValue").unwrap();
    assert_eq!((property.native_type(), property.flags()), (12, 1));
    assert_eq!(property.text().unwrap(), expression);
    assert_eq!(
        property.raw_value(),
        expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let source =
        super::super::normal_actions::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), rows);
    assert_eq!(source.records.len(), records.len());
    for (ordinal, (actual, expected)) in source.records.iter().zip(&records).enumerate() {
        assert_eq!(actual.reference, (8 << 8) | ordinal as u32);
        assert_eq!(actual.offset, 8 * 4096 + 4081 - ordinal * 15);
        assert_eq!(actual.data, *expected);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires frozen independent clock source18; no clock/default/native writer calls"]
fn date_now_sources_have_exact_raw_bits_presence_properties_and_complete_action_source() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_CLOCK_DEFAULT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut bindings = BTreeMap::new();
    for line in fs::read_to_string(directory.join("manifest.tsv"))
        .unwrap()
        .lines()
    {
        let (digest, name) = line.split_once('\t').unwrap();
        assert_eq!(digest.len(), 64);
        assert!(
            bindings
                .insert(name.to_owned(), digest.to_owned())
                .is_none()
        );
    }
    assert_eq!(bindings.len(), 18);
    let mut views = 0;
    for header in [5_u32, 6] {
        for (function, expression, leap, prior) in [
            ("date", "Date()", 0x40e1dd2000000000, 0xbff0000000000000),
            ("now", "Now()", 0x40e1dd3000000000, 0xbff4000000000000),
        ] {
            let mut cases = vec![
                ("before", vec![]),
                ("leap-noon", vec![(2, Some(leap))]),
                ("before-epoch-six", vec![(2, Some(prior))]),
                ("supplied-null", vec![(3, None)]),
            ];
            if function == "now" {
                cases.push(("leap-batch", vec![(2, Some(leap)), (4, Some(leap))]));
            }
            let mut before = Vec::new();
            for (phase, rows) in cases {
                let name = format!("h{header}-{function}-{phase}.accdb");
                let raw = fs::read(directory.join(&name)).unwrap();
                assert_eq!(raw.len(), 9 * 4096);
                assert_eq!(&raw[20..24], header.to_le_bytes());
                assert_eq!(
                    Sha256::digest(&raw)
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    bindings[&name]
                );
                check(&raw, expression, &rows);
                if phase == "before" {
                    before = raw;
                } else {
                    let end = 4081 - rows.len() * 15;
                    for at in 0..raw.len() {
                        let mutable = (7 * 4096 + 16..7 * 4096 + 20).contains(&at)
                            || (8 * 4096 + 2..8 * 4096 + 4).contains(&at)
                            || (8 * 4096 + 12..8 * 4096 + 14).contains(&at)
                            || (8 * 4096 + 16..8 * 4096 + 16 + rows.len() * 2).contains(&at)
                            || (8 * 4096 + end..8 * 4096 + 4081).contains(&at);
                        if !mutable {
                            assert_eq!(raw[at], before[at]);
                        }
                    }
                }
                views += 1;
            }
        }
    }
    assert_eq!(views, 18);
}
