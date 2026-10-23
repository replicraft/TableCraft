mod support;
use support::run;

#[test]
fn cli_date_interval_sources_share_explicit_policies_and_buffer_failures() {
    let sql = concat!(
        "SELECT Day(DateAdd('m','1','2024-01-31')),",
        "DatePart('h','-1,25'),DateDiff('h','-1,25',0)"
    );
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            sql,
            "--date-format",
            "iso",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["29", "6", "18"]);
        for sql in [
            "SELECT 99,DateAdd('d',1,'2025-01-01')",
            "SELECT 99,DatePart('d','2025-01-01')",
            "SELECT 99,DateDiff('d',NULL,'2025-01-01')",
            "SELECT 99,DateAdd('bad',NULL,1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn interval_error_states_keep_eager_arguments_and_progressive_week_validation() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("DateAdd(CVErr(1),1/0,0)", "division by zero"),
            ("DateDiff('d',CVErr(1),0,1/0)", "division by zero"),
            ("DatePart('yyyy',CVErr(1),1/0)", "division by zero"),
            ("DateAdd('bad',CVErr(1),0)", "invalid date interval"),
            ("DateDiff('d',CVErr(1),0,8,CVErr(1))", "domain"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
    }
}
