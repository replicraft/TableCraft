mod support;
use support::run;

#[test]
fn statistical_aggregates_run_with_grouped_inputs_and_documented_singleton_results() {
    let result = run(&[
        "query",
        "run",
        "SELECT Var(q.X) AS V,VarP(q.X) AS VP,StDev(q.X) AS S,StDevP(q.X) AS SP FROM (SELECT 1 AS X UNION ALL SELECT 3) AS q",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(result.stdout.as_slice());
    assert_eq!(
        reader.headers().unwrap(),
        &csv::StringRecord::from(vec!["V", "VP", "S", "SP"])
    );
    let row = reader.records().next().unwrap().unwrap();
    for (value, expected) in row.iter().zip([2.0_f64, 1.0, 2.0_f64.sqrt(), 1.0]) {
        assert!((value.parse::<f64>().unwrap() - expected).abs() < 1e-14);
    }
    let result = run(&[
        "query",
        "run",
        "SELECT Var(1) AS V,VarP(1) AS VP,StDev(1) AS S,StDevP(1) AS SP",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"V\tVP\tS\tSP\nNULL\tNULL\tNULL\t0\n");
}
#[test]
fn aggregate_refusals_and_late_errors_never_publish_partial_stdout() {
    for args in [
        vec!["query", "eval", "SELECT Var(1)"],
        vec!["query", "run", "SELECT Var(*)"],
        vec![
            "query",
            "run",
            "SELECT 1,Var(q.X) FROM (SELECT 1 AS X UNION ALL SELECT 'text') AS q",
        ],
        vec![
            "query",
            "run",
            "SELECT TOP 1 StDevP(1 \\ q.X) FROM (SELECT 1 AS X UNION ALL SELECT 0) AS q",
        ],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn date_statistics_require_explicit_numeric_conversion() {
    for function in ["Var", "VarP", "StDev", "StDevP"] {
        let sql = format!("SELECT 1,{function}(#2024-01-01#)");
        let result = run(&["query", "run", &sql]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("expression type mismatch"));
    }

    let result = run(&[
        "query",
        "run",
        "SELECT VarP(CDbl(X)) AS V,StDevP(CDbl(X)) AS S FROM (SELECT #2024-01-01# AS X UNION ALL SELECT #2024-01-03# UNION ALL SELECT NULL) AS T WHERE X IS NOT NULL",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"V\tS\n1\t1\n");
}
