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

#[test]
fn unary_date_math_preserves_results_and_buffers_date_overflow() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Abs(CDate(-1.75)) AS A,CDbl(Int(CDate(-1.75))) AS I,CDbl(Fix(CDate(-1.75))) AS F,Sgn(CDate(-1.75)) AS S,VarType(Int(CDate(2.5))) AS T",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tI\tF\tS\tT\n1.75\t-2\t-1\t-1\t7\n");
        let result = run(&["query", mode, "SELECT 1,Int(CDate(-657434.75))"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn round_date_digit_controls_use_raw_numeric_serials_in_both_query_paths() {
    for mode in ["eval", "run"] {
        let output = run(&[
            "query",
            mode,
            "SELECT Round(2.25,CDate(1)),Round(2.25,CDate(1.5)),Round(2.25,CDate(-0.5)),Round(NULL,CDate(1))",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(output.stdout.as_slice());
        let row = reader.records().next().unwrap().unwrap();
        assert_eq!(row.iter().collect::<Vec<_>>(), ["2.2", "2.25", "2", "NULL"]);

        for expression in [
            "Round(NULL,CDate(29))",
            "Round(CVErr(1),CDate(29))",
            "Round(1/0,CDate(1))",
        ] {
            let output = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!output.status.success(), "{expression}");
            assert!(output.stdout.is_empty(), "{expression}");
        }
    }
}
