//! Literal DATE metadata/row source gate; never resolves a default or calls a creator.
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

const SCOPE: CatalogScope = CatalogScope::new(229);
const CASES: [(&str, &str, u64); 6] = [
    ("epoch-zero", "#1899-12-30 00:00:00#", 0x0000000000000000),
    (
        "before-epoch-quarter",
        "#1899-12-29 06:00:00#",
        0xbff4000000000000,
    ),
    ("leap-noon", "#2000-02-29 12:00:00#", 0x40e1dd3000000000),
    ("minimum", "#0100-01-01 00:00:00#", 0xc124103400000000),
    ("maximum", "#9999-12-31 00:00:00#", 0x4146924080000000),
    (
        "fractional-epoch",
        "#1899-12-30 12:34:56.125#",
        0x3fe0c6be635dad52,
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn date(bits: u64) -> Value {
    Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
}

fn raw_record(id: i32, bits: Option<u64>) -> Vec<u8> {
    let mut raw = Vec::from([2, 0]);
    raw.extend_from_slice(&id.to_le_bytes());
    raw.extend_from_slice(&bits.unwrap_or(0).to_le_bytes());
    raw.push(if bits.is_some() { 3 } else { 1 });
    raw
}

fn check(raw: &[u8], expression: &str, bits: u64, phase: &str) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let mut expected = vec![vec![Value::Int32(1), date(0x3ff4000000000000)]];
    let mut records = vec![raw_record(1, Some(0x3ff4000000000000))];
    match phase {
        "before" => {}
        "omitted" => {
            expected.push(vec![Value::Int32(2), date(bits)]);
            records.push(raw_record(2, Some(bits)));
        }
        "supplied-null" => {
            expected.push(vec![Value::Int32(3), Value::Null]);
            records.push(raw_record(3, None));
        }
        _ => panic!("closed source phase"),
    }
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), expected);
    assert_eq!(table.schema().definition_page(), 7);
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|field| (
                field.name(),
                field.native_type(),
                field.storage_length(),
                field.flags()
            ))
            .collect::<Vec<_>>(),
        [("Id", 4, 4, 3), ("When", 8, 8, 3)]
    );
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
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
    assert_eq!(
        properties.raw_bytes().len(),
        64 + expression.encode_utf16().count() * 2
    );
    assert_eq!(properties.groups().len(), 1);
    let group = &properties.groups()[0];
    assert_eq!(group.kind(), PropertyGroupKind::Column);
    assert_eq!(group.name(), "When");
    assert_eq!(group.properties().len(), 1);
    let value = group.get("DefaultValue").unwrap();
    assert_eq!((value.native_type(), value.flags()), (12, 1));
    assert_eq!(value.text().unwrap(), expression);
    assert_eq!(
        value.raw_value(),
        expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let source =
        super::super::normal_actions::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), expected);
    assert_eq!(source.records.len(), records.len());
    for (ordinal, (actual, expected)) in source.records.iter().zip(&records).enumerate() {
        assert_eq!(actual.reference, (8 << 8) | ordinal as u32);
        assert_eq!(actual.offset, 8 * 4096 + 4081 - ordinal * 15);
        assert_eq!(actual.data, *expected);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires independently frozen DATE source36; no creator or omitted resolver"]
fn date_default_literals_have_exact_bits_properties_presence_and_full_readonly_source() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_DATE_DEFAULT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut bindings = BTreeMap::new();
    for line in fs::read_to_string(directory.join("manifest.tsv"))
        .unwrap()
        .lines()
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        assert!(
            bindings
                .insert(
                    fields[0].to_owned(),
                    (fields[1].parse::<usize>().unwrap(), fields[2].to_owned())
                )
                .is_none()
        );
    }
    assert_eq!(bindings.len(), 36);
    let mut views = 0;
    for header in [5_u32, 6] {
        for (case, expression, bits) in CASES {
            let mut before = Vec::new();
            for phase in ["before", "omitted", "supplied-null"] {
                let name = format!("h{header}-{case}-{phase}.accdb");
                let raw = fs::read(directory.join(&name)).unwrap();
                let binding = &bindings[&name];
                assert_eq!(raw.len(), 9 * 4096);
                assert_eq!(raw.len(), binding.0);
                let hash = Sha256::digest(&raw)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert_eq!(hash, binding.1);
                assert_eq!(&raw[20..24], header.to_le_bytes());
                check(&raw, expression, bits, phase);
                if phase == "before" {
                    before = raw;
                } else {
                    for at in 0..raw.len() {
                        let mutable = (7 * 4096 + 16..7 * 4096 + 20).contains(&at)
                            || (8 * 4096 + 2..8 * 4096 + 4).contains(&at)
                            || (8 * 4096 + 12..8 * 4096 + 14).contains(&at)
                            || (8 * 4096 + 16..8 * 4096 + 18).contains(&at)
                            || (8 * 4096 + 4066..8 * 4096 + 4081).contains(&at);
                        if !mutable {
                            assert_eq!(raw[at], before[at]);
                        }
                    }
                }
                views += 1;
            }
        }
    }
    assert_eq!(views, 36);
}

#[test]
fn frozen_date_literal_and_alias_laws_keep_representation_separate_from_calendar() {
    for (_, expression, bits) in CASES {
        let contents = &expression[1..expression.len() - 1];
        let parsed = tablecraft_expressions::datetime::parse_query_literal(contents).unwrap();
        assert_eq!(parsed.to_bits(), bits);
        let calendar = parsed.calendar_rounded_to_nanos().unwrap();
        assert_eq!(
            LegacyDateTime::from_calendar(calendar).unwrap().to_bits(),
            bits
        );
    }
    for (bits, canonical) in [
        (0x8000000000000000, 0x0000000000000000),
        (0xbfd0000000000000, 0x3fd0000000000000),
        (0x0000000000000001, 0x0000000000000000),
        (0x3fefffffffffffff, 0x3ff0000000000000),
    ] {
        let original = LegacyDateTime::from_bits(bits).unwrap();
        let converted =
            LegacyDateTime::from_calendar(original.calendar_rounded_to_nanos().unwrap()).unwrap();
        assert_eq!(converted.to_bits(), canonical);
        assert_ne!(converted.to_bits(), original.to_bits());
    }
    for invalid in [
        "1900-02-29 00:00:00",
        "2024-02-29 24:00:00",
        "2024-02-29 23:59:60",
        "2024-02-29 00:00:00Z",
        "1/2",
        "Date()",
        "Now()",
    ] {
        assert!(tablecraft_expressions::datetime::parse_query_literal(invalid).is_err());
    }
}
