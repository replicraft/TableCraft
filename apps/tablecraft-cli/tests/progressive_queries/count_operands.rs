use super::*;

#[test]
fn count_operands_keep_null_raw_string_and_error_admission_in_the_cli() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Count(*) AS AllRows,Count(Id) AS PresentRows,Count(Null) AS NullRows, ",
        "Count(ChrW(55296)) AS RawUnits,Count(CVErr(2001)) AS Problems, ",
        "VarType(Count(Id)) AS Kind FROM Items",
    );
    let expected = b"AllRows\tPresentRows\tNullRows\tRawUnits\tProblems\tKind\n3\t3\t0\t3\t3\t20\n";

    assert_eq!(success(file.query(sql, &[])), expected);
    for batch_rows in ["1", "2"] {
        assert_eq!(
            success(file.query(sql, &["--stream", "--batch-rows", batch_rows])),
            expected
        );
    }
}

#[test]
fn nested_scalar_controls_keep_each_count_occurrences_own_result() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Round(Count(Id)/2,Count(Null)) AS Rounded, ",
        "Count(Null) AS EmptyCount,Count(Id) AS PresentCount FROM Items",
    );
    let expected = success(file.query(sql, &[]));

    assert!(!expected.is_empty());
    assert_eq!(
        success(file.query(sql, &["--stream", "--batch-rows", "1"])),
        expected
    );
}

#[test]
fn count_operand_failures_preserve_only_completed_earlier_batches() {
    let file = FileCopy::new(fixture::database());
    for sql in [
        "SELECT Count(10\\(Id-2)) FROM Items",
        "SELECT Count(IIf(True,Id,1/0)) FROM Items",
        "SELECT Count(IIf(True,ChrW(55296),1/0)) FROM Items",
    ] {
        assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }

    let sql = concat!(
        "SELECT Count(*) AS N FROM Items UNION ALL ",
        "SELECT Count(10\\(Id-2)) FROM Items",
    );
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"N\n3\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());
}
