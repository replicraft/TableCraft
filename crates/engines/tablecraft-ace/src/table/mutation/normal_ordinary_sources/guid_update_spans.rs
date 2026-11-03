//! Four literal unindexed GUID states; original native contexts belong to a550.
use crate::{
    NativeImage, check_table, inspect_table_schema, read_table,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as declarations, canonical_ordinary_wide_insert as original,
};

const SCOPE: CatalogScope = CatalogScope::new(297);
const PAGE: usize = 4096;
const DATA: usize = 8;
const RECORD_AT: usize = DATA * PAGE + 3907;
const RECORD_BYTES: usize = 63;
const OLD_NATIVE: [u8; 16] = [
    0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const NEW_CANONICAL: [u8; 16] = [
    0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
];
const NEW_NATIVE: [u8; 16] = [
    0x40, 0xfc, 0x29, 0x6b, 0x47, 0xca, 0x67, 0x10, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
];
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

fn expected_rows(null: bool) -> Vec<Vec<Value>> {
    let source = original::source(original::Case::FixedExact);
    let mut rows = source.rows_before;
    rows.push(source.values);
    rows[2][9] = if null {
        Value::Null
    } else {
        Value::Guid(Guid::from_bytes(NEW_CANONICAL))
    };
    rows
}

fn literal_state(before: &[u8], null: bool) -> Vec<u8> {
    let mut after = before.to_vec();
    if null {
        after[RECORD_AT + 62] = 1;
        assert_eq!(&after[RECORD_AT + 45..RECORD_AT + 61], &OLD_NATIVE);
        assert_eq!(&after[RECORD_AT + 61..RECORD_AT + 63], &[0xff, 1]);

        // Restore has no third new state: logical NULL plus the original Guid
        // sets only its presence bit and returns the already-pinned original.
        let mut restored = after.clone();
        restored[RECORD_AT + 62] = 3;
        assert_eq!(restored, before);
    } else {
        // Half-open record span 45..61, with independent literal mixed-endian bytes.
        after[RECORD_AT + 45..RECORD_AT + 61].copy_from_slice(&NEW_NATIVE);
        assert_eq!(&after[RECORD_AT + 61..RECORD_AT + 63], &[0xff, 3]);
    }
    for (offset, (&old, &new)) in before.iter().zip(&after).enumerate() {
        let selected = if null {
            offset == RECORD_AT + 62
        } else {
            (RECORD_AT + 45..RECORD_AT + 61).contains(&offset)
        };
        if !selected {
            assert_eq!(old, new, "protected complement at {offset}");
        }
    }
    assert_eq!(after.len(), before.len());
    assert_ne!(after, before);
    after
}

fn inspect_state(raw: &[u8], header: u8, null: bool) {
    let state = if null {
        "null-clear"
    } else {
        "present-replacement"
    };
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
        assert_eq!(field.field.offset, usize::from(literal.fixed_offset));
        assert!(field.fixed());
        assert_eq!(field.flags(), 3);
        assert_eq!(field.extended_flags(), 0);
    }
    assert_eq!(schema.columns()[9].name(), "GuidValue");
    assert_eq!(schema.columns()[9].native_type(), 15);
    assert_eq!(schema.columns()[9].storage_length(), 16);
    let expected = expected_rows(null);
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), expected);
    if null {
        assert_eq!(read.rows()[2][9], Value::Null);
    } else {
        assert!(
            matches!(&read.rows()[2][9], Value::Guid(value) if value.to_bytes() == NEW_CANONICAL)
        );
    }
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());

    match NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false) {
        Ok(_) => eprintln!("GUID_SPAN_NO_REQUIRED header={header} state={state} ADMITTED"),
        Err(error) => {
            eprintln!("GUID_SPAN_NO_REQUIRED header={header} state={state} REFUSED {error:?}")
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
            eprintln!("GUID_SPAN_CANONICAL_SOURCE header={header} state={state} ADMITTED");
        }
        Err(error) => {
            eprintln!("GUID_SPAN_CANONICAL_SOURCE header={header} state={state} REFUSED {error:?}")
        }
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "four literal states; original API contexts are covered by guid_update_originals"]
fn literal_unindexed_guid_states_preserve_all_other_bytes_and_observe_source_admission() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_WIDE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, pin)| {
            let raw =
                fs::read(directory.join(format!("h{header}-fixed-exact-after.accdb"))).unwrap();
            assert_eq!(raw.len(), 9 * PAGE);
            let digest = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(digest, pin);
            assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
            assert_eq!(
                &raw[DATA * PAGE + 18..DATA * PAGE + 20],
                &3907_u16.to_le_bytes()
            );
            assert_eq!(&raw[RECORD_AT..RECORD_AT + 2], &10_u16.to_le_bytes());
            assert_eq!(&raw[RECORD_AT + 45..RECORD_AT + 61], &OLD_NATIVE);
            assert_eq!(&raw[RECORD_AT + 61..RECORD_AT + 63], &[0xff, 3]);
            (header, raw)
        })
        .collect();

    // These two original states have no duplicate native reads here. Their
    // original-only sibling gate owns those API observations; new states only.
    let mut contexts = 0;
    for (header, before) in images {
        for null in [false, true] {
            inspect_state(&literal_state(&before, null), header, null);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
    eprintln!(
        "GUID_SPAN_COVERAGE originals_pinned=2 new_states=4 restore_associations=2 original_native_contexts_reused=2"
    );
}
