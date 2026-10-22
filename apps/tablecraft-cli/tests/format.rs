mod support;

use support::run;

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
