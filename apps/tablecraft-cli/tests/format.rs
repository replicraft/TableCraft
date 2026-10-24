mod support;

use support::run;

#[test]
fn format_states_keep_control_precedence_and_failed_stdout_atomic() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Len(Format(CLng(CVErr(11)),'0')) AS N,Len(Format(CStr(CVErr(11)),'@')) AS T",
            "--number-format",
            "en-US",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tT\n2\t8\n");
        for (expression, diagnostic) in [
            ("Format(CVErr(11),'0',8,CVErr(12))", "domain"),
            ("Format(CVErr(11),CVErr(12),8,NULL)", "invalid use of NULL"),
            ("Format(CVErr(11),NULL)", "invalid use of NULL"),
            ("Format(1,'0',CVErr(11))", "type mismatch"),
            ("Format(CVErr(11),'0',1/0)", "division by zero"),
        ] {
            let sql = format!("SELECT 1,{expression}");
            let result = run(&["query", mode, &sql, "--number-format", "en-US"]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                "{sql}"
            );
        }
    }
}

#[test]
fn custom_format_sections_and_optional_slots_execute_in_both_query_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Format(1234,'#,##0.00') AS N,Format(-12,'0;(0)') AS S,Format(23,,1,1) AS D,Format(Null,'0;0;0;\"missing\"') AS Z",
            "--number-format",
            "en-US",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"1,234.00\"", "\"(12)\"", "\"23\"", "\"missing\""]
        );
        for expression in [
            "Format(1,'0E+')",
            "Format(1,'0',1,1,1/0)",
            "Format(Null,'0',1/0)",
            "Format(1,Null)",
            "Format(*)",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-US",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}

#[test]
fn date_serial_numeric_formats_do_not_need_calendar_text_settings() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Format(#2025-01-01 12:00:00#,'0.00'),Format(#1899-12-29 06:00:00#,'General Number')",
            "--number-format",
            "de-DE",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"45658,50\"", "\"-1,25\""]
        );
    }
}

#[test]
fn interleaved_numeric_masks_preserve_literal_positions_and_buffer_later_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Format(12345,'00-00'),Format(12.345,'0.0\"x\"0'),Format(1,'0#'),Eval(\"Format(12,'00-00')\")",
            "--number-format",
            "de-DE",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"123-45\"", "\"12,3x5\"", "\"01\"", "\"00-12\""]
        );
        let failed = run(&[
            "query",
            mode,
            "SELECT Format(12,'00-00'),1/0",
            "--number-format",
            "en-US",
        ]);
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }
}
