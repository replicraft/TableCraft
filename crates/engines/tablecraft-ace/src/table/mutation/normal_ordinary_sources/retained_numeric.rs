//! Original/expected reading only; no native mutation, qualification token or replay.
use crate::{NativeImage, check_table, inspect_table_indexes, read_table};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(137);
const TABLES: [(&str, usize, u8, usize); 4] = [
    ("ByteKeys", 900, 2, 1),
    ("Int16Keys", 900, 3, 2),
    ("Int64Keys", 400, 19, 8),
    ("CurrencyKeys", 400, 5, 8),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_view(raw: &[u8], changed: Option<&str>) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    for (name, rows, native, _) in TABLES {
        let table = read_table(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
        assert_eq!(table.rows().len(), rows);
        let fields = table.schema().columns();
        assert_eq!(fields.len(), 3);
        assert_eq!((fields[0].name(), fields[0].native_type()), ("ID", 4));
        assert_eq!(
            (fields[1].name(), fields[1].native_type()),
            ("KeyValue", native)
        );
        assert_eq!((fields[2].name(), fields[2].native_type()), ("Marker", 4));
        let key = match native {
            2 => Value::Byte(0),
            3 => Value::Int16(0),
            19 => Value::Int64(0),
            5 => Value::Currency(Currency::from_scaled(0)),
            _ => unreachable!(),
        };
        for (ordinal, row) in table.rows().iter().enumerate() {
            let id = i32::try_from(ordinal + 1).unwrap();
            let marker = if changed == Some(name) && ordinal == 0 {
                42
            } else {
                id * 17
            };
            assert_eq!(
                row,
                &vec![Value::Int32(id), key.clone(), Value::Int32(marker)]
            );
        }
        let indexes =
            inspect_table_indexes(&mut image.reader(), SCOPE, name, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(indexes.indexes().len(), 4);
        for index in indexes.indexes() {
            assert_eq!(index.columns().len(), 1);
            assert_ne!(index.columns()[0].number(), 2);
        }
        check_table(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires explicit pinned original and expected retained-numeric directories"]
fn genuine_numeric_indexes_and_literal_marker_overlays_read_before_any_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_RETAINED_NUMERIC_ORIGINALS_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_RETAINED_NUMERIC_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut views = 0;
    for (profile, header, length, digest, data) in [
        (
            "V2016",
            5_u8,
            757760,
            "8e976662f1c2ddf2e7616870082a71da3a618017ab0cca6faf78f7fe301350a6",
            [94, 114, 133, 149],
        ),
        (
            "V2019",
            6_u8,
            827392,
            "dcb7f14d9eab21dd9f518ddd3aafdc90b06f9325a20b5bbe60cdc44303f39dd7",
            [111, 131, 150, 166],
        ),
    ] {
        let path = originals.join(format!("{profile}-before.accdb"));
        let before = fs::read(&path).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        assert_eq!(&before[20..24], &u32::from(header).to_le_bytes());
        read_view(&before, None);
        views += 1;
        for ((table, _, _, width), page) in TABLES.into_iter().zip(data) {
            let after = fs::read(expected.join(format!("{profile}-{table}-marker.accdb"))).unwrap();
            let start = (page + 1) * 4096 - (11 + width);
            assert_eq!(&before[start..start + 6], &[3, 0, 1, 0, 0, 0]);
            assert!(
                before[start + 6..start + 6 + width]
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert_eq!(
                &before[start + 6 + width..start + 11 + width],
                &[17, 0, 0, 0, 7]
            );
            let marker = start + 6 + width;
            let mut literal = before.clone();
            literal[marker..marker + 4].copy_from_slice(&[42, 0, 0, 0]);
            assert_eq!(after, literal);
            read_view(&after, Some(table));
            views += 1;
        }
        assert_eq!(fs::read(path).unwrap(), before);
    }
    assert_eq!(views, 10);
}
