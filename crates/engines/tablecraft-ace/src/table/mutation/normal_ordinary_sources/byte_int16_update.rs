//! Literal span-only Byte/Int16 states; no UPDATE producer or output files.
use crate::{
    NativeImage, check_table, inspect_table_schema, read_table,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as declarations, canonical_ordinary_wide_insert as original,
};

const SCOPE: CatalogScope = CatalogScope::new(294);
const PAGE: usize = 4096;
const DATA: usize = 8;
const RECORD_BYTES: usize = 63;
const RECORD_AT: usize = DATA * PAGE + 3907;
const PINS: &[(u8, &str)] = &[
    (
        5,
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        6,
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn rows(state: &str) -> Vec<Vec<Value>> {
    let source = original::source(original::Case::FixedExact);
    let mut rows = source.rows_before;
    rows.push(source.values);
    match state {
        "original" => {}
        "endpoints" => {
            rows[2][1] = Value::Byte(0);
            rows[2][2] = Value::Int16(32767);
        }
        "null-clear" => {
            rows[2][1] = Value::Null;
            rows[2][2] = Value::Null;
        }
        _ => panic!("outside frozen literal span laws"),
    }
    rows
}

fn assert_original(raw: &[u8]) {
    assert_eq!(raw.len(), 9 * PAGE);
    assert_eq!(&raw[7 * PAGE + 16..7 * PAGE + 20], &3_u32.to_le_bytes());
    assert_eq!(&raw[DATA * PAGE..DATA * PAGE + 2], &[1, 1]);
    assert_eq!(&raw[DATA * PAGE + 4..DATA * PAGE + 8], &7_u32.to_le_bytes());
    assert_eq!(
        &raw[DATA * PAGE + 12..DATA * PAGE + 14],
        &3_u16.to_le_bytes()
    );
    assert_eq!(
        &raw[DATA * PAGE + 2..DATA * PAGE + 4],
        &3887_u16.to_le_bytes()
    );
    for slot in 0..3 {
        let directory = DATA * PAGE + 14 + 2 * slot;
        let start = u16::try_from(PAGE - RECORD_BYTES * (slot + 1)).unwrap();
        assert_eq!(&raw[directory..directory + 2], &start.to_le_bytes());
    }
    let record = &raw[RECORD_AT..RECORD_AT + RECORD_BYTES];
    assert_eq!(&record[..2], &10_u16.to_le_bytes());
    assert_eq!(&record[2..5], &[0xff, 0x00, 0x80]);
    assert_eq!(&record[61..63], &[0xff, 0x03]);
}

fn variant(before: &[u8], state: &str) -> Vec<u8> {
    let mut after = before.to_vec();
    match state {
        "endpoints" => {
            // Half-open record span 2..5: Byte(0), Int16(32767), little endian.
            after[RECORD_AT + 2..RECORD_AT + 5].copy_from_slice(&[0, 0xff, 0x7f]);
        }
        "null-clear" => {
            // Clear native presence bits 1 and 2; do not rewrite inactive payload.
            after[RECORD_AT + 61] = 0xf9;
            assert_eq!(&after[RECORD_AT + 2..RECORD_AT + 5], &[0xff, 0, 0x80]);
        }
        _ => panic!("only two independently specified span variants"),
    }
    for (offset, (&old, &new)) in before.iter().zip(&after).enumerate() {
        let selected = match state {
            "endpoints" => (RECORD_AT + 2..RECORD_AT + 5).contains(&offset),
            "null-clear" => offset == RECORD_AT + 61,
            _ => unreachable!(),
        };
        if !selected {
            assert_eq!(old, new, "protected complement at {offset}");
        }
    }
    assert_eq!(after.len(), before.len());
    assert_ne!(after, before);
    after
}

fn inspect(raw: &[u8], header: u8, state: &str) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(schema.definition_page(), 7);
    assert_eq!(schema.columns().len(), 10);
    for (number, (field, literal)) in schema
        .columns()
        .iter()
        .zip(&declarations::COLUMNS)
        .enumerate()
    {
        assert_eq!(field.number(), number);
        assert_eq!(field.name(), literal.name);
        assert_eq!(field.native_type(), literal.native_type);
        assert_eq!(field.storage_length(), usize::from(literal.width));
        assert!(field.fixed());
        assert_eq!(field.flags(), 3);
        assert_eq!(field.extended_flags(), 0);
    }
    assert_eq!(schema.columns()[1].native_type(), 2);
    assert_eq!(schema.columns()[2].native_type(), 3);
    let expected = rows(state);
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), expected);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());

    match NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false) {
        Ok(_) => eprintln!("BYTE_INT16_NO_REQUIRED header={header} state={state} ADMITTED"),
        Err(error) => {
            eprintln!("BYTE_INT16_NO_REQUIRED header={header} state={state} REFUSED {error:?}")
        }
    }
    match normal_ordinary::capture(&image, "Items", &mut budget(), &|| false) {
        Ok(source) => {
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.records.len(), 3);
            for (slot, record) in source.records.iter().enumerate() {
                assert_eq!(record.reference, ((DATA as u32) << 8) | slot as u32);
                let start = DATA * PAGE + PAGE - RECORD_BYTES * (slot + 1);
                assert_eq!(record.data, raw[start..start + RECORD_BYTES]);
            }
            eprintln!("BYTE_INT16_CANONICAL_SOURCE header={header} state={state} ADMITTED");
        }
        Err(error) => {
            eprintln!("BYTE_INT16_CANONICAL_SOURCE header={header} state={state} REFUSED {error:?}")
        }
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "two pinned originals, four in-memory span states; no mutable admission or writer"]
fn literal_byte_int16_spans_preserve_complement_and_observe_source_admission() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_BYTE_INT16_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    // Verify both complete original files before calling any source API.
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, pin)| {
            let raw =
                fs::read(directory.join(format!("h{header}-fixed-exact-after.accdb"))).unwrap();
            let digest = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(digest, pin);
            assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
            assert_original(&raw);
            (header, raw)
        })
        .collect();

    let mut contexts = 0;
    for (header, before) in images {
        inspect(&before, header, "original");
        contexts += 1;
        for state in ["endpoints", "null-clear"] {
            let after = variant(&before, state);
            inspect(&after, header, state);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 6);
}
