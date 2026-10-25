use super::*;

#[test]
fn late_expression_errors_retain_only_previously_completed_output_batches() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Id,10\\(2-Id) AS N FROM Items";
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"Id\tN\n1\t10\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());
    assert!(failure(file.query(sql, &[])).is_empty());

    for sql in [
        "SELECT Id FROM Items WHERE False AND 1/0=0",
        "SELECT IIf(True,Id,1/0) FROM Items",
        "SELECT ChrW(55296) FROM Items",
    ] {
        assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }
}

#[test]
fn late_native_corruption_and_count_mismatch_never_become_successful_streams() {
    let mut bytes = fixture::database();
    let slot = 12 * 4096 + 16;
    let row = u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) as usize & 0x1fff;
    bytes[12 * 4096 + row..12 * 4096 + row + 2].copy_from_slice(&100_u16.to_le_bytes());
    let corrupt = FileCopy::new(bytes);
    let sql = "SELECT Id,Score FROM Items";
    assert_eq!(
        failure(corrupt.query(sql, &["--stream", "--batch-rows", "1"])),
        b"Id\tScore\n1\t10\n"
    );
    assert!(failure(corrupt.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());

    let mut bytes = fixture::database();
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&4_u32.to_le_bytes());
    let mismatch = FileCopy::new(bytes);
    assert_eq!(
        failure(mismatch.query(sql, &["--stream", "--batch-rows", "1"])),
        b"Id\tScore\n1\t10\n2\t20\n3\t30\n"
    );
}

#[test]
fn explicit_parameters_text_policies_and_declared_arithmetic_reach_each_row() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "PARAMETERS Floor LONG; SELECT Id,CStr(Id)&'x' AS Caption FROM Items ",
        "WHERE Id>Floor AND CStr(Id)>='2'",
    );
    let policies = [
        "--param",
        "Floor=long:1",
        "--number-format",
        "invariant",
        "--collation",
        "ordinal",
    ];
    let expected = success(file.query(sql, &policies));
    let arguments = [policies.as_slice(), &["--stream", "--batch-rows", "1"]].concat();
    assert_eq!(success(file.query(sql, &arguments)), expected);

    let sql = "SELECT '02'=CVar(CLng(2)) AS Eq FROM Items";
    for (declared, expected) in [
        (false, &b"Eq\nTRUE\nTRUE\nTRUE\n"[..]),
        (true, &b"Eq\nFALSE\nFALSE\nFALSE\n"[..]),
    ] {
        let mut arguments = vec![
            "--stream",
            "--batch-rows",
            "1",
            "--number-format",
            "invariant",
            "--collation",
            "ordinal",
        ];
        if declared {
            arguments.push("--declared-arithmetic");
        }
        assert_eq!(success(file.query(sql, &arguments)), expected);
    }
    assert!(
        failure(file.query(
            "SELECT CInt(32767)+CInt(Id) FROM Items",
            &["--stream", "--declared-arithmetic", "--batch-rows", "1"]
        ))
        .is_empty()
    );

    let generated = "SELECT Len(String(70000,'a')) AS N FROM Items";
    assert!(failure(file.query(generated, &["--stream"])).is_empty());
    assert_eq!(
        success(file.query(
            generated,
            &["--stream", "--batch-rows", "1", "--text-bytes", "70000"]
        )),
        b"N\n70000\n70000\n70000\n"
    );
}

#[test]
fn native_and_query_controls_are_independent_and_cumulative() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Id FROM Items";
    for flag in [
        "--native-bytes",
        "--native-rows",
        "--native-steps",
        "--execution-bytes",
        "--execution-steps",
    ] {
        assert!(
            failure(file.query(sql, &["--stream", "--batch-rows", "1", flag, "1"])).is_empty(),
            "{flag}"
        );
    }
    assert_eq!(
        failure(file.query(
            sql,
            &["--stream", "--batch-rows", "1", "--execution-rows", "4"]
        )),
        b"Id\n1\n"
    );
}
