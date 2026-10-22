use std::process::Command;

#[test]
fn both_query_commands_apply_date_policies_and_buffer_bad_dates() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT #2025-01-01#='01.01.2025' AS E,                 #2025-01-01# BETWEEN '31.12.2024' AND '02.01.2025' AS B",
                "--date-format",
                "de-de",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"E\tB\nTRUE\tTRUE\n");
        for controls in [vec![], vec!["--date-format", "iso"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, "SELECT 1,#2025-01-01#='bad'"])
                .args(controls)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}
