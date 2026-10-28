use super::*;
use crate::{
    BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET,
    tests::{Directory, budget},
};
use tablecraft_core::limits::LimitError;

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
