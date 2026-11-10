use std::io::Cursor;
use support::{data_page, image, put16, put32};
use tablecraft_ace::{InspectError, inspect_table_schema, read_table};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access as support;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn decimal_image() -> Vec<u8> {
    let mut b = image(false, false);
    b.resize(9 * 4096, 0);
    let start = (u16::from_le_bytes([b[4 * 4096 + 14], b[4 * 4096 + 15]]) & 0x1fff) as usize;
    put32(&mut b, 4 * 4096 + start + 2, 6);
    let mut def = vec![0; 4096];
    def[..2].copy_from_slice(&[2, 1]);
    put32(&mut def, 8, 4096);
    put32(&mut def, 16, 2);
    put16(&mut def, 41, 1);
    put16(&mut def, 43, 1);
    put16(&mut def, 45, 1);
    put32(&mut def, 55, 7 << 8);
    def[63] = 16;
    def[74] = 28;
    def[75] = 4;
    def[78] = 2;
    put16(&mut def, 86, 17);
    put16(&mut def, 88, 2);
    def[90..92].copy_from_slice(&[b'N', 0]);
    b[6 * 4096..7 * 4096].copy_from_slice(&def);
    b[7 * 4096..8 * 4096].copy_from_slice(&data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 1])]));
    let mut row = vec![1, 0, 0x80];
    row.extend([0; 12]);
    row.extend(12300_u32.to_le_bytes());
    row.extend([19, 0, 2, 0, 1, 0, 1]);
    let null = vec![1, 0, 2, 0, 2, 0, 1, 0, 0];
    b[8 * 4096..9 * 4096].copy_from_slice(&data_page(6, &[(0, row), (0, null)]));
    b
}
#[test]
fn native_metadata_and_rows_preserve_decimal_scale_and_null() {
    let b = decimal_image();
    let schema = inspect_table_schema(
        &mut Cursor::new(b.clone()),
        CatalogScope::new(1),
        "Table",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns()[0].decimal_parameters(), Some((28, 4)));
    assert_eq!(schema.columns()[0].scalar_type(), Some(ScalarType::Decimal));
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
        &[
            vec![Value::Decimal(Decimal::new(-12300, 4).unwrap())],
            vec![Value::Null]
        ]
    );
}
#[test]
fn invalid_metadata_fails_even_when_no_present_decimal_value_is_needed() {
    for (offset, value) in [(74, 0), (74, 29), (75, 29), (79, 2), (86, 9)] {
        let mut b = decimal_image();
        b[6 * 4096 + offset] = value;
        assert_eq!(
            read_table(
                &mut Cursor::new(b),
                CatalogScope::new(1),
                "Table",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
    let mut b = decimal_image();
    b[6 * 4096 + 74] = 4;
    // The stored coefficient 12300 exceeds the declared four-digit precision.
    assert_eq!(
        read_table(
            &mut Cursor::new(b),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
}

fn fixed_decimal_image() -> Vec<u8> {
    let mut bytes = decimal_image();
    put16(&mut bytes, 6 * 4096 + 43, 0);
    bytes[6 * 4096 + 78] = 3;
    put32(&mut bytes, 6 * 4096 + 16, 4);
    let positive = vec![
        1, 0, 0, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9, 1,
    ];
    // NO_CLONE: These independent raw records differ only in sign; data_page
    // owns both payloads and a borrowed slice cannot represent their mutations.
    let mut negative = positive.clone();
    negative[2] = 0x80;
    let mut signed_zero = vec![0; 20];
    signed_zero[0] = 1;
    signed_zero[2] = 0x80;
    signed_zero[19] = 1;
    let mut null = vec![0xff; 20];
    null[..2].copy_from_slice(&[1, 0]);
    null[19] = 0;
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data_page(
        6,
        &[(0, positive), (0, negative), (0, signed_zero), (0, null)],
    ));
    bytes
}

#[test]
fn fixed_numeric_rows_preserve_word_order_sign_scale_and_opaque_null_payloads() {
    for version in [1, 2, 3, 5, 6] {
        let mut bytes = fixed_decimal_image();
        bytes[20] = version;
        if version == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let mut source = Cursor::new(bytes);
        let schema = inspect_table_schema(
            &mut source,
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(schema.columns()[0].fixed());
        assert_eq!(schema.columns()[0].decimal_parameters(), Some((28, 4)));
        assert_eq!(schema.columns()[0].scalar_type(), Some(ScalarType::Decimal));
        let table = read_table(
            &mut source,
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &[
                vec![Value::Decimal(
                    Decimal::new(311917102708983781990730508, 4).unwrap()
                )],
                vec![Value::Decimal(
                    Decimal::new(-311917102708983781990730508, 4).unwrap()
                )],
                vec![Value::Decimal(Decimal::new(0, 4).unwrap())],
                vec![Value::Null],
            ],
            "native version {version}"
        );
    }
}

#[test]
fn fixed_numeric_metadata_and_present_payload_fail_without_coercion() {
    for (offset, value) in [(74, 0), (74, 29), (75, 29), (79, 2), (86, 16), (86, 18)] {
        let mut bytes = fixed_decimal_image();
        bytes[6 * 4096 + offset] = value;
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Table",
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
    let mut bytes = fixed_decimal_image();
    bytes[6 * 4096 + 74] = 26;
    assert_eq!(
        read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Table",
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(InspectError::Malformed)
    );
}
