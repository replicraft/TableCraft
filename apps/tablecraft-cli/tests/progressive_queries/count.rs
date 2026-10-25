use super::*;

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;

#[test]
fn global_counts_publish_one_complete_row_including_empty_inputs() {
    let help = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "run", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("COUNT(*)"));

    let file = FileCopy::new(fixture::database());
    for (filter, expected) in [
        ("True", &b"Total\tNextValue\n3\t4\n"[..]),
        ("Id<=2", &b"Total\tNextValue\n2\t3\n"[..]),
        ("False", &b"Total\tNextValue\n0\t1\n"[..]),
    ] {
        let sql =
            format!("SELECT Count(*) AS Total,Total+1 AS NextValue FROM Items WHERE {filter}");
        let buffered = success(file.query(&sql, &[]));

        assert_eq!(buffered, expected);
        for batch_rows in ["1", "2"] {
            assert_eq!(
                success(file.query(&sql, &["--stream", "--batch-rows", batch_rows])),
                expected
            );
        }
    }
}

#[test]
fn count_arms_share_union_conversion_and_written_order_with_row_arms() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Id AS X FROM Items WHERE Id=1 UNION ALL ",
        "SELECT Count(*) FROM Items WHERE False UNION ALL ",
        "SELECT Count(*) FROM Items",
    );
    let expected = b"X\n1\n0\n3\n";

    assert_eq!(success(file.query(sql, &[])), expected);
    assert_eq!(
        success(file.query(sql, &["--stream", "--batch-rows", "1"])),
        expected
    );
}

#[test]
fn a_completed_count_arm_obeys_batch_boundaries_before_a_later_error() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Count(*) AS X FROM Items UNION ALL ",
        "SELECT 10\\(Id-1) FROM Items",
    );

    assert!(failure(file.query(sql, &[])).is_empty());
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"X\n3\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());
}

#[test]
fn count_failures_never_publish_a_premature_aggregate_row() {
    let file = FileCopy::new(fixture::database());
    for sql in [
        "SELECT Count(*) FROM Items WHERE False And 1/0=0",
        "SELECT 1/Count(*) FROM Items WHERE False",
        "SELECT IIf(True,Count(*),1/0) FROM Items",
    ] {
        assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }

    let sql = "SELECT Count(*) FROM Items";
    for (flag, value) in [
        ("--native-rows", "1"),
        ("--execution-rows", "1"),
        ("--execution-steps", "1"),
        ("--output-bytes", "1"),
    ] {
        assert!(failure(file.query(sql, &["--stream", flag, value])).is_empty());
    }

    let mut bytes = fixture::database();
    let start = TABLE_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET;
    bytes[start..start + size_of::<u32>()].copy_from_slice(&4_u32.to_le_bytes());
    let mismatch = FileCopy::new(bytes);

    assert!(failure(mismatch.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
}
