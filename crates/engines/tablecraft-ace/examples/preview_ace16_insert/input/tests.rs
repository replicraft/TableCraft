use super::*;
use crate::{
    BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET,
    tests::{Directory, budget},
};
use tablecraft_core::limits::LimitError;
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single};

const VALUE_BYTES: usize = 1024;

#[test]
fn ordered_carriers_preserve_quotes_empty_null_binary_and_int32() {
    let rows = parse(
        b"int32\t-2147483648\tbinary\t00fF\ntext\t\"a\tb\0\n\"\tnull\t\ntext\t\"a\"\"b\"\tbinary\t\n",
        &mut budget(),
        VALUE_BYTES,
    )
    .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].values(),
        &[Value::Int32(i32::MIN), Value::Binary(vec![0, 255])]
    );
    assert_eq!(
        rows[1].values(),
        &[Value::Text("a\tb\0\n".into()), Value::Null]
    );
    assert_eq!(
        rows[2].values(),
        &[Value::Text("a\"b".into()), Value::Binary(Vec::new())]
    );
}

#[test]
fn exact_fixed_carriers_keep_widths_scaled_units_bits_and_guid_order() {
    let encoded = concat!(
        "boolean\ttrue\tbyte\t255\tint16\t-32768\t",
        "int32\t2147483647\tint64\t-9223372036854775808\t",
        "currency-scaled\t-12345\tsingle-bits\t80000000\t",
        "double-bits\t3ff0000000000000\tdate-bits\t8000000000000000\t",
        "guid\t00112233-4455-6677-8899-aabbccddeeff\tdecimal\t-12345:4\tnull\t\n",
    );
    let rows = parse(encoded.as_bytes(), &mut budget(), VALUE_BYTES).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values(),
        &[
            Value::Boolean(true),
            Value::Byte(u8::MAX),
            Value::Int16(i16::MIN),
            Value::Int32(i32::MAX),
            Value::Int64(i64::MIN),
            Value::Currency(Currency::from_scaled(-12345)),
            Value::Single(Single::from_bits(0x8000_0000)),
            Value::Double(Double::from_bits(0x3ff0_0000_0000_0000)),
            Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
            Value::Guid(Guid::from_bytes([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ])),
            Value::Decimal(Decimal::new(-12345, 4).unwrap()),
            Value::Null,
        ]
    );
}

#[test]
fn malformed_pairs_arity_values_and_carriers_refuse_the_complete_batch() {
    for bad in [
        &b""[..],
        b"int32\t1\tbinary\t00\nint32\t2\n",
        b"int32\t1\tbinary\n",
        b"int32\t2147483648\n",
        b"int32\t1.0\n",
        b"null\tx\n",
        b"binary\t0\n",
        b"binary\tzz\n",
        b"text\t\xff\n",
        b"Text\tx\n",
        b"boolean\tTRUE\n",
        b"byte\t256\n",
        b"int16\t-32769\n",
        b"int64\t9223372036854775808\n",
        b"currency-scaled\t1.25\n",
        b"single-bits\t100000000\n",
        b"double-bits\t10000000000000000\n",
        b"date-bits\t7ff8000000000000\n",
        b"guid\tnot-a-guid\n",
        b"int32\tnull\n",
        b"boolean\tnull\n",
        b"decimal\t1:29\n",
        b"decimal\t1.5:2\n",
        b"decimal\tnull\n",
    ] {
        assert!(parse(bad, &mut budget(), VALUE_BYTES).is_err());
    }
}

#[test]
fn row_and_value_limits_are_exact() {
    let full = "int32\t1\tbinary\t00\n".repeat(MAX_REQUESTS);
    assert_eq!(
        parse(full.as_bytes(), &mut budget(), VALUE_BYTES)
            .unwrap()
            .len(),
        4096
    );
    assert!(
        parse(
            (full + "int32\t2\tbinary\t00\n").as_bytes(),
            &mut budget(),
            VALUE_BYTES
        )
        .is_err()
    );
    assert!(
        parse(
            format!("text\t{}\n", "x".repeat(1024)).as_bytes(),
            &mut budget(),
            VALUE_BYTES
        )
        .is_ok()
    );
    assert!(
        parse(
            format!("text\t{}\n", "x".repeat(1025)).as_bytes(),
            &mut budget(),
            VALUE_BYTES
        )
        .is_err()
    );
}

#[test]
fn parser_uses_shared_bytes_rows_and_steps_pools() {
    for (mut allowance, resource) in [
        (
            ResourceBudget::new(0, ROW_BUDGET, STEP_BUDGET),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(BYTE_BUDGET, 0, STEP_BUDGET),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, 0),
            Resource::Steps,
        ),
    ] {
        let error = parse(b"int32\t1\tbinary\t00\n", &mut allowance, VALUE_BYTES)
            .err()
            .unwrap();
        assert_eq!(
            error.downcast_ref::<LimitError>().unwrap().resource,
            resource
        );
    }
}

#[test]
fn oversized_file_refuses_before_capture() {
    let directory = Directory::new();
    let path = directory.file("large.tsv", b"");
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        load(&path, &mut budget(), VALUE_BYTES)
            .err()
            .unwrap()
            .to_string()
            .contains("16 MiB")
    );
}

#[test]
fn explicit_larger_values_bound_encoded_hex_and_utf8_bytes_without_coercion() {
    let encoded = format!("binary\t{}\n", "85".repeat(4077));
    let rows = parse(encoded.as_bytes(), &mut budget(), 8154).unwrap();
    assert_eq!(rows[0].values(), &[Value::Binary(vec![0x85; 4077])]);
    for limit in [1024, 8153] {
        assert!(parse(encoded.as_bytes(), &mut budget(), limit).is_err());
    }
    let text = "Ā".repeat(2040);
    assert_eq!(text.len(), 4080);
    let encoded = format!("text\t{text}\n");
    let rows = parse(encoded.as_bytes(), &mut budget(), 4080).unwrap();
    assert_eq!(rows[0].values(), &[Value::Text(text)]);
    assert!(parse(encoded.as_bytes(), &mut budget(), 4079).is_err());
}

#[test]
fn explicit_value_limit_cannot_expand_file_capture_or_shared_byte_allowances() {
    let directory = Directory::new();
    let path = directory.file("large.tsv", b"");
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();
    let error = load(&path, &mut budget(), MAX_VALUE_BYTES).err().unwrap();
    assert!(error.to_string().contains("16 MiB input ceiling"));

    let mut allowance = ResourceBudget::new(READ_BUFFER_BYTES as u64, ROW_BUDGET, STEP_BUDGET);
    let error = parse(b"text\tx\n", &mut allowance, MAX_VALUE_BYTES)
        .err()
        .unwrap();
    assert_eq!(
        error.downcast_ref::<LimitError>().unwrap().resource,
        Resource::Bytes
    );
}
