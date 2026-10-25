use std::process::Command;

#[test]
fn declared_numeric_text_preserves_longlong_precision_and_integer_rounding() {
    let sql =
        "SELECT CInt(2)='2.5' AS Rounded,CLngLng('9007199254740993')='9007199254740992' AS Exact";
    for (declared, expected) in [
        (false, "Rounded\tExact\nFALSE\tTRUE\n"),
        (true, "Rounded\tExact\nTRUE\tFALSE\n"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command.args([
            "query",
            "run",
            sql,
            "--number-format",
            "en-us",
            "--format",
            "tsv",
        ]);
        if declared {
            command.arg("--declared-arithmetic");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn selected_numeric_range_failures_remain_buffered_after_an_earlier_projection() {
    for expression in ["CByte(0)='256'", "CInt(0)='32768'", "CLng(0)='2147483648'"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "run",
                &format!("SELECT 1 AS Earlier,{expression} AS Failed"),
                "--declared-arithmetic",
                "--number-format",
                "en-us",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("overflow"));
    }
}
