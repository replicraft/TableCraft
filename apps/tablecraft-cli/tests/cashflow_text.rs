mod support;
use support::run;

#[test]
fn cashflow_rate_text_is_explicit_and_failures_are_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT NPV('0,25',Array(-100,0,125)) AS N",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut rows = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice());
        let row = rows.records().next().unwrap().unwrap();
        assert!((row[0].parse::<f64>().unwrap() + 16.0).abs() < 1e-12);
        let failure = run(&["query", mode, "SELECT 1,NPV('0,25',Array(-100,0,125))"]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("number-format"));
    }
}

#[test]
fn cashflow_error_data_retains_eager_array_and_control_evaluation() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("NPV(CVErr(1),Array(1/0))", "division by zero"),
            ("NPV(0,Array(CVErr(1),1/0))", "division by zero"),
            ("IRR(Array(CVErr(1)),1/0)", "division by zero"),
            ("NPV(CVErr(1),Array(NULL))", "type mismatch"),
            ("IRR(Array(NULL),CVErr(1))", "NULL"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
    }
}
