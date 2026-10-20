#[path = "support/page_reads.rs"]
mod page_reads;
mod support;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use support::{data_page, image, put16, put32};
use tablecraft_ace::{InspectError, read_table};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, Single, Value},
};
use tablecraft_model::CatalogScope;
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn scalar_image() -> Vec<u8> {
    let mut b = image(false, false);
    b.resize(10 * 4096, 0);
    let start = (u16::from_le_bytes([b[4 * 4096 + 14], b[4 * 4096 + 15]]) & 0x1fff) as usize;
    put32(&mut b, 4 * 4096 + start + 2, 6);
    let mut def = vec![0; 4096];
    def[..2].copy_from_slice(&[2, 1]);
    put32(&mut def, 8, 4096);
    put32(&mut def, 16, 3);
    put16(&mut def, 41, 9);
    put16(&mut def, 43, 2);
    put16(&mut def, 45, 9);
    put32(&mut def, 55, 7 << 8);
    let fields = [
        ("Flag", 1, 0, 0, 1, true),
        ("Byte", 2, 0, 0, 1, true),
        ("Short", 3, 0, 1, 2, true),
        ("Long", 4, 0, 3, 4, true),
        ("Money", 5, 0, 7, 8, true),
        ("Single", 6, 0, 15, 4, true),
        ("Double", 7, 0, 19, 8, true),
        ("Label", 10, 0, 0, 16, false),
        ("Blob", 9, 1, 0, 16, false),
    ];
    let mut names = 63 + 25 * fields.len();
    for (i, (name, kind, var, offset, len, fixed)) in fields.into_iter().enumerate() {
        let at = 63 + i * 25;
        def[at] = kind;
        put16(&mut def, at + 5, i as u16);
        put16(&mut def, at + 7, var);
        def[at + 15] = u8::from(fixed);
        put16(&mut def, at + 21, offset);
        put16(&mut def, at + 23, len);
        let name: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        put16(&mut def, names, name.len() as u16);
        names += 2;
        def[names..names + name.len()].copy_from_slice(&name);
        names += name.len();
    }
    b[6 * 4096..7 * 4096].copy_from_slice(&def);
    b[7 * 4096..8 * 4096].copy_from_slice(&data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 3])]));
    let rows = vec![
        (0, record(0)),
        (0, record(1)),
        (0x4000, (9_u32 << 8).to_le_bytes().to_vec()),
        (0x8000, vec![0xff; 4]),
    ];
    b[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &rows));
    b[9 * 4096..10 * 4096].copy_from_slice(&data_page(6, &[(0x8000, record(2))]));
    b
}
fn record(case: u8) -> Vec<u8> {
    let mut row = vec![0; 29];
    put16(&mut row, 0, 9);
    row[2] = 255;
    put16(&mut row, 3, i16::MIN as u16);
    put32(&mut row, 5, i32::MIN as u32);
    row[9..17].copy_from_slice(&i64::MIN.to_le_bytes());
    put32(&mut row, 17, 0x8000_0000);
    row[21..29].copy_from_slice(&0x7ff8_0000_0000_0001_u64.to_le_bytes());
    if case == 0 {
        row.extend("é🦀".encode_utf16().flat_map(u16::to_le_bytes));
    }
    let binary = row.len() as u16;
    if case == 0 {
        row.extend([0, 255]);
    }
    let end = row.len() as u16;
    for offset in [end, binary, 29, 2] {
        row.extend(offset.to_le_bytes());
    }
    row.extend(match case {
        0 => [0xff, 1],
        1 => [0x76, 1],
        _ => [0xff, 0],
    });
    row
}
#[test]
fn scalar_rows_preserve_exact_bits_null_empty_and_boolean_mask_values() {
    let bytes = scalar_image();
    let mut source = Cursor::new(bytes.clone());
    let table = read_table(
        &mut source,
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), 3);
    assert_eq!(
        &table.rows()[0][..7],
        &[
            Value::Boolean(true),
            Value::Byte(255),
            Value::Int16(i16::MIN),
            Value::Int32(i32::MIN),
            Value::Currency(Currency::from_scaled(i64::MIN)),
            Value::Single(Single::from_bits(0x8000_0000)),
            Value::Double(Double::from_bits(0x7ff8_0000_0000_0001))
        ]
    );
    assert_eq!(
        &table.rows()[0][7..],
        &[Value::Text("é🦀".into()), Value::Binary(vec![0, 255])]
    );
    assert_eq!(table.rows()[1][0], Value::Boolean(false));
    assert_eq!(table.rows()[1][3], Value::Null);
    assert_eq!(table.rows()[1][7], Value::Null);
    assert_eq!(table.rows()[1][8], Value::Binary(vec![]));
    assert_eq!(table.rows()[2][7], Value::Text(String::new()));
    assert_eq!(table.rows()[2][8], Value::Null);
    assert_eq!(source.into_inner(), bytes);
}
#[test]
fn sequential_roots_share_the_scan_page_but_relocations_remain_validated() {
    let mut source = page_reads::PageReads::new(scalar_image());
    let table = read_table(
        &mut source,
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), 3);
    assert_eq!(source.count(8), 1, "one read for three live row roots");
    assert_eq!(source.count(9), 2, "relocation read and deleted-root scan");

    for change in 0..4 {
        let mut bytes = scalar_image();
        match change {
            0 => put32(&mut bytes, 9 * 4096 + 4, 2), // Foreign relocation owner.
            1 => bytes[9 * 4096] = 2,                // Wrong target page type.
            2 => put16(&mut bytes, 9 * 4096 + 14, 0x8000), // Target overlaps slot directory.
            _ => {
                // Follow the first relocation, then detect its cycle.
                bytes[9 * 4096..10 * 4096].copy_from_slice(&data_page(
                    6,
                    &[(0xc000, ((8_u32 << 8) | 2).to_le_bytes().to_vec())],
                ));
            }
        }
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Table",
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Malformed),
        );
    }
}
#[test]
fn no_variable_columns_and_corrupt_rows_fail_or_decode_without_defaults() {
    let bytes = scalar_image();
    for change in 0..6 {
        let mut b = bytes.clone();
        match change {
            0 => put32(&mut b, 6 * 4096 + 16, 4), // Declared total mismatch.
            1 => put32(&mut b, 8 * 4096 + 4, 2),  // Foreign data owner.
            2 => put16(&mut b, 8 * 4096 + 14, 1), // Row bounds.
            3 => b[6 * 4096 + 63] = 15,           // Unsupported, even if NULL.
            4 => b[6 * 4096 + 63 + 16] = 0xc0,    // Calculated data cannot be treated as scalar.
            _ => b[6 * 4096 + 63 + 6 * 25] = 8,   // NaN is invalid as a native date.
        }
        assert!(
            read_table(
                &mut Cursor::new(b),
                CatalogScope::new(1),
                "Table",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut b = bytes;
    // Keep only Boolean and Byte in the definition and a fixed-only row.
    put16(&mut b, 6 * 4096 + 41, 2);
    put16(&mut b, 6 * 4096 + 43, 0);
    put16(&mut b, 6 * 4096 + 45, 2);
    let names: Vec<_> = ["Flag", "Byte"]
        .into_iter()
        .flat_map(|s| {
            let bytes: Vec<_> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
            [(bytes.len() as u16).to_le_bytes().as_slice(), &bytes].concat()
        })
        .collect();
    b[6 * 4096 + 113..6 * 4096 + 113 + names.len()].copy_from_slice(&names);
    put32(&mut b, 6 * 4096 + 16, 1);
    b[7 * 4096..8 * 4096].copy_from_slice(&data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 1])]));
    b[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &[(0, vec![2, 0, 12, 2])]));
    let table = read_table(
        &mut Cursor::new(b),
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[vec![Value::Boolean(false), Value::Byte(12)]]
    );
}
#[test]
fn cancellation_and_rows_budget_are_shared_with_catalog_work() {
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 30
    };
    assert_eq!(
        read_table(
            &mut Cursor::new(scalar_image()),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &cancel
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(
        read_table(
            &mut Cursor::new(scalar_image()),
            CatalogScope::new(1),
            "Table",
            &mut ResourceBudget::new(16 * 1024 * 1024, 3, 1_000_000),
            &|| false
        )
        .is_err()
    );
}
#[test]
fn candidate_local_scalar_tables_decode_with_exact_declared_counts() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    for (name, count) in [
        ("ROULETTE_TB", 38),
        ("TAG_GRP_TB", 19),
        ("TAG_NME_TB", 933),
        ("tblContacts", 0),
    ] {
        let t = read_table(
            &mut Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(t.rows().len(), count);
    }
}
proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x5441424c45524f57),..Default::default()})]
    #[test]
    fn mutated_data_pages_and_definitions_are_bounded(edits in prop::collection::vec((6*4096_usize..10*4096,any::<u8>()),1..24)) {
        let mut b=scalar_image();for (at,value) in edits {b[at]=value;}
        let _=read_table(&mut Cursor::new(b),CatalogScope::new(1),"Table",&mut budget(),&||false);
    }
}

#[test]
fn guid_native_little_endian_integer_groups_become_canonical_bytes() {
    let mut bytes = scalar_image();
    bytes[6 * 4096 + 63 + 8 * 25] = 15;
    let native = [
        0x40, 0xfc, 0x29, 0x6b, 0x47, 0xca, 0x67, 0x10, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
    ];
    let expected = [
        0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
    ];
    let original = record(0);
    let mut row = original[..35].to_vec();
    row.extend(native);
    for offset in [51_u16, 35, 29, 2] {
        row.extend(offset.to_le_bytes());
    }
    row.extend([0xff, 1]);
    let mut null = record(1);
    *null.last_mut().unwrap() = 0;
    let rows = [
        (0, row),
        (0, null),
        (0x4000, (9_u32 << 8).to_le_bytes().to_vec()),
    ];
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &rows));
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows()[0][8],
        Value::Guid(tablecraft_core::values::Guid::from_bytes(expected))
    );
    assert_eq!(table.rows()[1][8], Value::Null);
}

#[test]
fn ole_rows_are_opaque_bytes_with_distinct_empty_and_null_values() {
    let mut bytes = scalar_image();
    bytes[6 * 4096 + 63 + 8 * 25] = 11;
    put16(&mut bytes, 6 * 4096 + 63 + 8 * 25 + 23, 0);
    let make_row = |case, payload: &[u8]| {
        let original = record(case);
        let prefix = if case == 0 { 35 } else { 29 };
        let mut row = original[..prefix].to_vec();
        row.extend((0x8000_0000_u32 | payload.len() as u32).to_le_bytes());
        row.extend([0; 8]);
        row.extend(payload);
        let end = row.len() as u16;
        for offset in [end, prefix as u16, 29, 2] {
            row.extend(offset.to_le_bytes());
        }
        row.extend(if case == 0 { [0xff, 1] } else { [0x76, 1] });
        row
    };
    let rows = [
        (0, make_row(0, &[0, 255, 0, 216])), // Invalid UTF-16 must remain opaque bytes.
        (0, make_row(1, &[])),
        (0x4000, (9_u32 << 8).to_le_bytes().to_vec()),
    ];
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &rows));
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[0][8], Value::Binary(vec![0, 255, 0, 216]));
    assert_eq!(table.rows()[1][8], Value::Binary(vec![]));
    assert_eq!(table.rows()[2][8], Value::Null);
}

fn append_field(mut bytes: Vec<u8>, boolean: bool) -> Vec<u8> {
    let base = 6 * 4096;
    let old_names = bytes[base + 288..base + 500].to_vec();
    bytes[base + 313..base + 525].copy_from_slice(&old_names);
    bytes[base + 288..base + 313].fill(0);
    put16(&mut bytes, base + 41, 10);
    put16(&mut bytes, base + 45, 10);
    bytes[base + 288] = if boolean { 1 } else { 4 };
    put16(&mut bytes, base + 293, 9);
    bytes[base + 303] = 1;
    put16(&mut bytes, base + 309, 27);
    put16(&mut bytes, base + 311, if boolean { 1 } else { 4 });
    let mut end = base + 313;
    for _ in 0..9 {
        let length = u16::from_le_bytes([bytes[end], bytes[end + 1]]) as usize;
        end += 2 + length;
    }
    put16(&mut bytes, end, 2);
    bytes[end + 2..end + 4].copy_from_slice(&[b'N', 0]);
    bytes
}
#[test]
fn dense_appended_columns_read_old_fixed_and_variable_geometry() {
    for boolean in [false, true] {
        let mut bytes = append_field(scalar_image(), boolean);
        // Unused high bits in an older mask are not appended Boolean values.
        bytes[9 * 4096 - 1] = 0xff;
        let table = read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows().len(), 3);
        for row in table.rows() {
            assert_eq!(
                row[9],
                if boolean {
                    Value::Boolean(false)
                } else {
                    Value::Null
                }
            );
        }
        assert_eq!(table.rows()[0][7], Value::Text("é🦀".into()));
        assert_eq!(table.rows()[0][8], Value::Binary(vec![0, 255]));
    }
}
#[test]
fn known_gapped_histories_read_but_invented_current_payloads_are_rejected() {
    let mut gap = append_field(scalar_image(), false);
    put16(&mut gap, 6 * 4096 + 41, 11);
    put16(&mut gap, 6 * 4096 + 293, 10);
    let table = read_table(
        &mut Cursor::new(gap),
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        table
            .rows()
            .iter()
            .all(|row| row.last() == Some(&Value::Null))
    );
    let mut bytes = append_field(scalar_image(), false);
    let start = u16::from_le_bytes([bytes[8 * 4096 + 14], bytes[8 * 4096 + 15]]) as usize;
    // Claim the new fixed field exists, without actually providing its payload.
    put16(&mut bytes, 8 * 4096 + start, 10);
    assert!(
        read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn older_fixed_only_rows_do_not_consume_new_variable_tail_bytes() {
    for row in [vec![1, 0, 1], vec![2, 0, 12, 3]] {
        let mut bytes = append_field(scalar_image(), true);
        put32(&mut bytes, 6 * 4096 + 16, 1);
        bytes[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &[(0, row.clone())]));
        let table = read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows()[0][0], Value::Boolean(true));
        assert_eq!(
            table.rows()[0][1],
            if row[0] == 2 {
                Value::Byte(12)
            } else {
                Value::Null
            }
        );
        assert!(
            table.rows()[0][2..9]
                .iter()
                .all(|value| matches!(value, Value::Null))
        );
        assert_eq!(table.rows()[0][9], Value::Boolean(false));
    }
}
