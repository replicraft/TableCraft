mod support;
use support::run;
#[test]
fn numeric_text_calls_work_in_both_paths_with_buffered_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Hex(459) AS H,Oct(459) AS O,Str(-459.65) AS S,Str(459.001) AS P,Hex(NULL) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec!["H", "O", "S", "P", "N"],
                vec!["\"1CB\"", "\"713\"", "\"-459.65\"", "\" 459.001\"", "NULL"]
            ]
        );
        if mode == "eval" {
            let oversized = format!("SELECT {}", vec!["Str(CDbl(1e-299))"; 16].join(","));
            let result = run(&["query", mode, &oversized]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        for expression in ["Hex('x')", "Oct(1,1/0)", "Str('1')", "Hex(2147483648.0)"] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn numeric_text_inputs_use_explicit_parsing_and_buffer_conversion_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Hex('3,5') AS H,Oct('8') AS O,Str('2,5') AS S",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [vec!["H", "O", "S"], vec!["\"4\"", "\"10\"", "\" 2.5\""]]
        );
        for expression in ["Hex('2147483648')", "Oct('bad')", "Str('bad')"] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-us",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn str_expression_values_use_standard_codes_and_explicit_date_policy() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Str(CVErr(2001)) AS E,Str(Nz(NULL,0)) AS Z,Str(CDate(2)) AS D",
            "--date-format",
            "iso",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec!["E", "Z", "D"],
                vec!["\"Error 2001\"", "\" 0\"", "\"1900-01-01\""]
            ]
        );
        for expression in ["Str(IIf(True,CVErr(1),1/0))", "Str(CDate(2))"] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn radix_date_inputs_need_no_locale_and_error_data_does_not_become_a_code() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Hex(CDate(2.5)) AS H,Oct(CDate(8.5)) AS O,Hex(CLng(CVErr(1))) AS C",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [vec!["H", "O", "C"], vec!["\"2\"", "\"10\"", "\"1\""]]
        );
        for expression in [
            "Hex(CVErr(1))",
            "Oct(CVErr(1))",
            "Hex(IIf(True,CVErr(1),1/0))",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}
