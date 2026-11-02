//! Independent companion BEFORE images only; no planner, creator or resolver.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table, read_table_index,
    table::mutation::{autonumber_counter, record_insertion},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(257);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn rows(family: &str, state: &str) -> Vec<Vec<Value>> {
    if state == "empty" {
        return Vec::new();
    }
    let inactive = state == "companion-null-high";
    match family {
        "text" => vec![vec![
            Value::Int32(1),
            if inactive {
                Value::Null
            } else {
                Value::Text("old".into())
            },
            Value::Int32(10),
        ]],
        "binary" => vec![vec![
            Value::Int32(1),
            if inactive {
                Value::Null
            } else {
                Value::Binary(vec![0xc7, 0, 0xff])
            },
            Value::Int32(10),
        ]],
        "fixed" => vec![vec![
            Value::Int32(1),
            Value::Boolean(true),
            if inactive {
                Value::Null
            } else {
                Value::Int64(i64::MIN)
            },
        ]],
        _ => panic!("closed original family"),
    }
}

fn raw_record(family: &str, inactive: bool) -> Vec<u8> {
    match family {
        "text" => vec![
            3,
            0,
            1,
            0,
            0,
            0,
            10,
            0,
            0,
            0,
            0x6f,
            0,
            0x6c,
            0,
            0x64,
            0,
            16,
            0,
            10,
            0,
            1,
            0,
            if inactive { 5 } else { 7 },
        ],
        "binary" => vec![
            3,
            0,
            1,
            0,
            0,
            0,
            10,
            0,
            0,
            0,
            0xc7,
            0,
            0xff,
            13,
            0,
            10,
            0,
            1,
            0,
            if inactive { 5 } else { 7 },
        ],
        "fixed" => vec![
            3,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0x80,
            if inactive { 3 } else { 7 },
        ],
        _ => panic!("closed original family"),
    }
}

fn schema_fields(family: &str) -> Vec<(&'static str, u8, u8, usize)> {
    match family {
        "text" => vec![("Id", 4, 7, 4), ("Label", 10, 2, 80), ("Score", 4, 3, 4)],
        "binary" => vec![("Id", 4, 7, 4), ("Bytes", 9, 2, 40), ("Score", 4, 3, 4)],
        "fixed" => vec![("Id", 4, 7, 4), ("Flag", 1, 3, 0), ("Amount", 19, 3, 8)],
        _ => panic!("closed original family"),
    }
}

fn check(raw: &[u8], header: u32, family: &str, state: &str) {
    assert_eq!(raw.len(), 40960);
    assert_eq!(&raw[20..24], header.to_le_bytes());
    let empty = state == "empty";
    let expected = rows(family, state);
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), expected);
    let schema = table.schema();
    assert_eq!(schema.definition_page(), 7);
    assert_eq!(schema.declared_rows() as usize, expected.len());
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| (
                field.name(),
                field.native_type(),
                field.flags(),
                field.storage_length()
            ))
            .collect::<Vec<_>>(),
        schema_fields(family)
    );
    for (number, field) in schema.columns().iter().enumerate() {
        assert_eq!(field.number(), number);
        assert_eq!(field.extended_flags(), 0);
    }
    assert_eq!(schema.layout.max_columns, 3);
    assert_eq!(schema.layout.variable_slots, usize::from(family != "fixed"));
    assert_eq!(
        schema.layout.fixed_end,
        if family == "fixed" { 14 } else { 10 }
    );

    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    let [index] = checked.indexes() else {
        panic!("one complete primary owner");
    };
    assert_eq!(index.status(), IndexCheckStatus::Checked);
    assert_eq!(index.index().name(), "PrimaryId");
    assert_eq!(index.index().root_page(), 9);
    assert_eq!(
        read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            "PrimaryId",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        expected
    );
    assert_eq!(
        inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            "Items",
            "PrimaryId",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .value(),
        if empty { 0 } else { 1 }
    );
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
    assert!(properties.raw_bytes().is_empty());
    assert!(properties.groups().is_empty());

    // This is the actual native17/NoRequired/ownership admission boundary,
    // independently of the existing shape-restricted generation consumer.
    let source = super::super::capture_source(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), expected);
    assert_eq!(source.records.len(), expected.len());
    let counter =
        autonumber_counter::capture(&image, source.table.schema(), 0, &mut budget(), &|| false)
            .unwrap();
    assert_eq!(counter.column, 0);
    assert_eq!(counter.last, if empty { 0 } else { 137 });
    assert_eq!(counter.offset, 7 * 4096 + 20);
    assert!(matches!(
        super::mode::inspect(&image, source.table.schema(), &mut budget(), &|| false).unwrap(),
        super::mode::Mode::Increment
    ));
    let variables = record_insertion::layout::incrementing_variables(
        source.table.schema(),
        0,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let shape = record_insertion::proof::layout::capture_incrementing(
        source.table.schema(),
        0,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        variables,
        if family == "fixed" {
            Vec::new()
        } else {
            vec![1]
        }
    );
    assert_eq!(shape.variables, variables);
    assert_eq!(shape.fixed_end, if family == "fixed" { 14 } else { 10 });
    assert_eq!(shape.mask_bytes, 1);
    let data = &raw[8 * 4096..9 * 4096];
    assert_eq!(&data[12..14], &(expected.len() as u16).to_le_bytes());
    if empty {
        assert_eq!(&data[2..4], &4082_u16.to_le_bytes());
        assert_eq!(&data[14..], &vec![0; 4082]);
    } else {
        let record = raw_record(family, state == "companion-null-high");
        let start = 4096 - record.len();
        assert_eq!(&data[14..16], &(start as u16).to_le_bytes());
        assert_eq!(&data[2..4], &((start - 16) as u16).to_le_bytes());
        assert_eq!(&data[start..], record);
        assert_eq!(source.records[0].reference, 8 << 8);
        assert_eq!(source.records[0].offset, 8 * 4096 + start);
        assert_eq!(source.records[0].data, record);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires pure authored companion BEFORE sources; no generated-row or mutation producer"]
fn companion_originals_preserve_variable_fixed_and_inactive_carriers_before_generation() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_COMPANION_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read_to_string(directory.join("manifest.tsv")).unwrap();
    let mut pins = BTreeMap::new();
    for line in manifest.lines() {
        let (digest, name) = line.split_once('\t').unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(pins.insert(name.to_owned(), digest.to_owned()).is_none());
    }
    assert_eq!(pins.len(), 18);
    let mut originals = Vec::new();
    for header in [5, 6] {
        for family in ["text", "binary", "fixed"] {
            for state in ["empty", "high", "companion-null-high"] {
                let name = format!("h{header}-increment-{family}-{state}-before.accdb");
                let wanted = pins.remove(&name).unwrap();
                let raw = fs::read(directory.join(name)).unwrap();
                assert_eq!(
                    Sha256::digest(&raw)
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    wanted
                );
                originals.push((header, family, state, raw));
            }
        }
    }
    assert!(pins.is_empty());
    // All 18 source files are pinned before the first native capture.
    for (header, family, state, raw) in originals {
        check(&raw, header, family, state);
    }
}
