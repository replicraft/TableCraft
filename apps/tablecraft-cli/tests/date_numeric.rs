mod support;
use support::run;

#[test]
fn query_commands_convert_dates_without_ambient_number_or_calendar_formats() {
    for mode in ["eval", "run"] {
        let output = run(&[
            "query",
            mode,
            "SELECT CDbl(#2025-01-01 12:00:00#),CLng(#1900-01-01 12:00:00#),CBool(#1899-12-30#),IsNumeric(#2025-01-01#),TypeName(CDbl(#2025-01-01#)),TypeName(CDec(#2025-01-01#)),TypeName(CCur(#2025-01-01#)),Eval('CDbl(#1899-12-29 06:00:00#)')",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(output.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            [
                "45658.5",
                "2",
                "FALSE",
                "FALSE",
                "\"Double\"",
                "\"Decimal\"",
                "\"Currency\"",
                "-1.25",
            ]
        );
        for sql in [
            "SELECT CDbl(#2025-01-01#),CInt(#2025-01-01#)",
            "SELECT CByte(#1899-12-29#)",
            "SELECT CDbl(Null)",
            "SELECT CDbl(#2025-01-01#),1/0",
            "SELECT CStr(#2025-01-01#)",
        ] {
            let output = run(&["query", mode, sql]);
            assert!(!output.status.success(), "{sql}");
            assert!(output.stdout.is_empty(), "{sql}");
        }
    }
}
