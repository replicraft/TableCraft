use std::process::Command;

#[test]
fn both_query_commands_compare_numeric_text_and_buffer_late_failures() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT 2='2,0' AS E,2 BETWEEN '3,0' AND '1,0' AS B,CCur(0.0002)='0,00015' AS C",
                "--number-format",
                "de-de",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"E\tB\tC\nTRUE\tTRUE\tTRUE\n");
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT 1,2='bad'",
                "--number-format",
                "en-us",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn both_query_commands_buffer_checked_currency_integer_failures() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT CCur(1)=CLngLng(1) AS E,CLngLng(922337203685477)>CCur(0) AS G",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(output.stdout, b"E\tG\nTRUE\tTRUE\n");

        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT 1 AS First,CCur(0)=CLngLng(922337203685478) AS Fails",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
