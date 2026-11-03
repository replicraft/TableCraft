//! Original-only read/Source qualification, with separate temporary capability observations.
use crate::{
    FixedColumnUpdates, IndexCheckStatus, NativeImage, RuleCheckStatus, check_table,
    check_table_constraints, inspect_table_index_counter, inspect_table_properties, read_table,
    read_table_index,
    table::mutation::{
        fixed_records::{binding, indexed::integral_updates::IntegralUpdates},
        normal_ordinary,
    },
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(251);
const TABLES: [&str; 2] = ["Int64Companions", "CurrencyCompanions"];
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
const RECORDS: [&str; 3] = [
    "0600010000000000000000000080110000006600690072007300740000ff1e001c00120002003f",
    "0600020000000000000000000080220000006d006900640064006c0065001e001e001200020035",
    "060003000000ffffffffffffff7f330000007400610069006c001a001a00120002001f",
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex(raw: &str) -> Vec<u8> {
    raw.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn images() -> Vec<(u8, Vec<u8>)> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_SIBLING_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    [
        (
            5,
            421888,
            "fe40ed5d67d0b82c31063b803717eaace70f36724c01951513215382c91e7ef3",
        ),
        (
            6,
            495616,
            "11d395698885c7a8ef4ea01149118eedba84cdcb00030a33c4923b18abc6de21",
        ),
    ]
    .into_iter()
    .map(|(header, length, hash)| {
        let raw =
            fs::read(directory.join(format!("h{header}-wide-sibling-original.accdb"))).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(digest(&raw), hash);
        (header, raw)
    })
    .collect()
}

fn geometry(header: u8, table: &str) -> (usize, usize, [usize; 3]) {
    match (header, table) {
        (5, "Int64Companions") => (89, 94, [91, 92, 93]),
        (5, "CurrencyCompanions") => (96, 101, [98, 99, 100]),
        (6, "Int64Companions") => (106, 111, [108, 109, 110]),
        (6, "CurrencyCompanions") => (113, 119, [115, 116, 117]),
        _ => panic!("outside observed four original table contexts"),
    }
}

fn rows(table: &str) -> Vec<Vec<Value>> {
    let numeric = |units| {
        if table == "Int64Companions" {
            Value::Int64(units)
        } else {
            Value::Currency(Currency::from_scaled(units))
        }
    };
    vec![
        vec![
            Value::Int32(1),
            numeric(i64::MIN),
            Value::Int32(17),
            Value::Boolean(true),
            Value::Text("first".into()),
            Value::Binary(vec![0, 255]),
        ],
        vec![
            Value::Int32(2),
            Value::Null,
            Value::Int32(34),
            Value::Boolean(false),
            Value::Text("middle".into()),
            Value::Binary(Vec::new()),
        ],
        vec![
            Value::Int32(3),
            numeric(i64::MAX),
            Value::Int32(51),
            Value::Boolean(true),
            Value::Text("tail".into()),
            Value::Null,
        ],
    ]
}

#[test]
#[ignore = "requires two independent SDK originals; no native writer or AFTER model"]
fn wider_numeric_sibling_originals_have_complete_raw_read_and_canonical_source() {
    let images = images();
    let mut contexts = 0;
    for (header, raw) in &images {
        for table in TABLES {
            let (definition, data, roots) = geometry(*header, table);
            let image =
                NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let snapshot =
                read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            let expected = rows(table);
            assert_eq!(snapshot.rows(), expected);
            let schema = snapshot.schema();
            assert_eq!(schema.definition_page() as usize, definition);
            assert_eq!(schema.columns().len(), 6);
            for (position, (name, kind, length, offset)) in [
                ("Id", 4, 4, 0),
                (
                    "KeyValue",
                    if table == "Int64Companions" { 19 } else { 5 },
                    8,
                    4,
                ),
                ("Marker", 4, 4, 12),
                // Raw declaration length is1, but Boolean has no payload bytes.
                ("Enabled", 1, 0, 0),
                ("Label", 10, 64, 0),
                ("Opaque", 9, 16, 0),
            ]
            .into_iter()
            .enumerate()
            {
                let field = &schema.columns()[position];
                assert_eq!(field.name(), name);
                assert_eq!(field.number(), position);
                assert_eq!(field.native_type(), kind);
                assert_eq!(field.storage_length(), length);
                assert_eq!(field.field.offset, offset);
                assert_eq!(field.flags(), if position < 4 { 3 } else { 2 });
                assert_eq!(field.extended_flags(), 0);
            }
            let td = definition * 4096;
            assert_eq!(&raw[td + 16..td + 24], &[3, 0, 0, 0, 0, 0, 0, 0]);
            let base = data * 4096;
            assert_eq!(&raw[base + 2..base + 4], &3963_u16.to_le_bytes());
            assert_eq!(
                &raw[base + 12..base + 20],
                &[3, 0, 217, 15, 178, 15, 143, 15]
            );
            for (ordinal, (start, end)) in [(4057, 4096), (4018, 4057), (3983, 4018)]
                .into_iter()
                .enumerate()
            {
                assert_eq!(&raw[base + start..base + end], hex(RECORDS[ordinal]));
            }
            let properties =
                inspect_table_properties(&mut image.reader(), SCOPE, table, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert!(properties.groups().is_empty());
            assert!(properties.raw_bytes().is_empty());
            let checked =
                check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.rows(), 3);
            assert_eq!(checked.indexes().len(), 6);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let constraints =
                check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert!(constraints.relationships().is_empty());
            assert!(
                constraints
                    .fields()
                    .iter()
                    .all(|field| field.required() == RuleCheckStatus::NotDeclared
                        && field.validation() == RuleCheckStatus::NotDeclared)
            );
            for (position, alias) in ALIASES.into_iter().enumerate() {
                let physical = position / 2;
                let root = roots[physical] * 4096;
                assert_eq!(&raw[root..root + 2], &[4, 1]);
                assert_eq!(
                    &raw[root + 2..root + 4],
                    &(if physical == 0 { 3589_u16 } else { 3585_u16 }).to_le_bytes()
                );
                assert_eq!(&raw[root + 20..root + 22], &[0, 0]);
                let order = if physical == 0 {
                    [0, 1, 2]
                } else if physical == 1 {
                    [1, 0, 2]
                } else {
                    [2, 0, 1]
                };
                let observed = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let ordered: Vec<_> = order.into_iter().map(|row| expected[row].clone()).collect();
                assert_eq!(observed.rows(), ordered);
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.value(), 3);
            }
            let source = normal_ordinary::capture(&image, table, &mut budget(), &|| false).unwrap();
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.records.len(), 3);
            for (ordinal, record) in source.records.iter().enumerate() {
                assert_eq!(record.reference, ((data as u32) << 8) | ordinal as u32);
                assert_eq!(record.data, hex(RECORDS[ordinal]));
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
}

#[test]
#[ignore = "diagnostic only: temporary write gates are not permanent source invariants"]
fn observe_wider_numeric_sibling_fixed_capability_and_target_facts() {
    for (header, raw) in images() {
        for table in TABLES {
            let image =
                NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let capability =
                image.check_native_ordinary_fixed_table(table, &mut budget(), &|| false);
            eprintln!(
                "OBSERVED_FIXED_CAPABILITY header={header} table={table} result={capability:?}"
            );
            match normal_ordinary::capture(&image, table, &mut budget(), &|| false) {
                Ok(source) => {
                    let target =
                        IntegralUpdates::capture(&image, &source, &mut budget(), &|| false)
                            .and_then(|facts| {
                                binding::all_integral(
                                    &image,
                                    &source,
                                    &[FixedColumnUpdates::new(table, "KeyValue", &[])],
                                    &facts,
                                    &mut budget(),
                                    &|| false,
                                )
                                .map(|_| ())
                            });
                    eprintln!(
                        "OBSERVED_EMPTY_WIDE_TARGET header={header} table={table} result={target:?}"
                    );
                }
                Err(error) => eprintln!(
                    "OBSERVED_CANONICAL_SOURCE header={header} table={table} result=REFUSED {error:?}"
                ),
            }
        }
    }
}
