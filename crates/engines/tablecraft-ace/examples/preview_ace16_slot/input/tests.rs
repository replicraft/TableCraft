use super::*;
use crate::{
    BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET,
    tests::{Directory, budget},
};
use tablecraft_core::limits::LimitError;

#[test]
fn quoted_text_nul_empty_null_and_binary_are_exact_distinct_carriers() {
    let raw =
        b"0\ttext\t\"a\tb\0\n\"\ttext\t\"a\"\"b\"\n1\tnull\t\ttext\t\n2\tbinary\t00fF\tbinary\t\n";
    let rows = parse(raw, &mut budget()).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].row(), 0);
    assert_eq!(rows[0].expected(), &Value::Text("a\tb\0\n".into()));
    assert_eq!(rows[0].value(), &Value::Text("a\"b".into()));
    assert_eq!(rows[1].expected(), &Value::Null);
    assert_eq!(rows[1].value(), &Value::Text(String::new()));
    assert_eq!(rows[2].expected(), &Value::Binary(vec![0, 255]));
    assert_eq!(rows[2].value(), &Value::Binary(Vec::new()));
}

#[test]
fn malformed_kinds_fields_rows_hex_and_utf8_never_return_a_partial_batch() {
    for bad in [
        &b""[..],
        b"0\ttext\ta\ttext\tb\n1\tnull\tx\ttext\tb\n",
        b"0\tbinary\t0\ttext\tb\n",
        b"0\tbinary\tzz\ttext\tb\n",
        b"0\tbinary\t+1\ttext\tb\n",
        b"0\ttext\t\xff\ttext\tb\n",
        b"-1\tnull\t\tnull\t\n",
        b"0\ttext\tx\ttext\ty\textra\n",
        b"0\tText\tx\ttext\ty\n",
        b"0\ttext\tx\ttext\n",
    ] {
        assert!(parse(bad, &mut budget()).is_err());
    }
}

#[test]
fn request_count_value_width_and_hostile_field_count_remain_bounded() {
    let full = "0\tnull\t\tnull\t\n".repeat(MAX_REQUESTS);
    assert_eq!(parse(full.as_bytes(), &mut budget()).unwrap().len(), 4096);
    let excessive = full + "0\tnull\t\tnull\t\n";
    assert!(
        parse(excessive.as_bytes(), &mut budget())
            .err()
            .unwrap()
            .to_string()
            .contains("Too many slot requests")
    );
    let wide = format!("0\ttext\t{}\tnull\t\n", "x".repeat(1025));
    assert!(
        parse(wide.as_bytes(), &mut budget())
            .err()
            .unwrap()
            .to_string()
            .contains("value exceeds input ceiling")
    );
    let hostile = "\t".repeat(16_384);
    assert!(
        parse(hostile.as_bytes(), &mut budget())
            .err()
            .unwrap()
            .to_string()
            .contains("expected five TSV fields")
    );
}

#[test]
fn parser_scratch_rows_and_work_share_typed_operation_limits() {
    let raw = b"0\ttext\ta\ttext\tb\n";
    for (mut allowance, expected) in [
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
        let error = parse(raw, &mut allowance).err().unwrap();
        assert_eq!(
            error.downcast_ref::<LimitError>().unwrap().resource,
            expected
        );
    }
}

#[test]
fn file_length_ceiling_refuses_before_capture_or_parsing() {
    let directory = Directory::new();
    let path = directory.file("oversized.tsv", b"");
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let error = load(&path, &mut budget()).err().unwrap();
    assert!(error.to_string().contains("16 MiB"));
}
