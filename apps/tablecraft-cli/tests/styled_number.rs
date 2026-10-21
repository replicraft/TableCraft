mod support;
use support::run;
fn rows(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn all_three_formats_optional_slots_and_overrides_work_in_both_paths() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT FormatNumber(1234.5) AS N,FormatPercent(0.125) AS P,FormatCurrency(-1234.5) AS C,FormatNumber(0.5,2,False) AS Z,FormatNumber(1234.5,,,,False) AS G,FormatNumber(-1.25,,,-1,) AS B",
            "--number-format",
            "en-us",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout),
            [
                vec!["N", "P", "C", "Z", "G", "B"],
                vec![
                    "\"1,234.50\"",
                    "\"12.50%\"",
                    "\"($1,234.50)\"",
                    "\".50\"",
                    "\"1234.50\"",
                    "\"(1.25)\""
                ]
            ]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT FormatNumber(1234.5),FormatCurrency('1.234,50 €'),FormatPercent(0.125,1)",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout)[1],
            ["\"1.234,50\"", "\"1.234,50 €\"", "\"12,5%\""]
        );
    }
}
#[test]
fn invalid_call_shapes_options_and_policies_publish_nothing() {
    for mode in ["eval", "run"] {
        for expression in [
            "FormatNumber(NULL)",
            "FormatNumber(1,NULL)",
            "FormatNumber(1,-2)",
            "FormatNumber(1,256)",
            "FormatNumber(1,2,1)",
            "FormatPercent(1,2,False,False,False,1/0)",
            "FormatNumber(*)",
            "FormatNumber(,2)",
            "VBA.FormatCurrency(1)",
            "FormatNumber(NULL,1/0)",
            "IIf(False,FormatNumber(1/0),0)",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-us",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
        for (expression, format) in [
            ("FormatNumber(1)", None),
            ("FormatCurrency(1)", Some("invariant")),
            ("FormatNumber(1,2,-2,-2,True)", Some("invariant")),
        ] {
            let mut arguments = vec!["query", mode, expression];
            let sql = format!("SELECT 1,{expression}");
            arguments[2] = &sql;
            if let Some(format) = format {
                arguments.extend(["--number-format", format]);
            }
            let result = run(&arguments);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
