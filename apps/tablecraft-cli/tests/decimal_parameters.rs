mod support;

use support::run;

#[test]
fn decimal_constraints_reach_parameters_subqueries_and_union_without_rescaling() {
    for spelling in ["DECIMAL", "NUMERIC"] {
        let sql =
            format!("PARAMETERS P {spelling}(4,2); SELECT (SELECT P) AS Exact UNION ALL SELECT P");
        let output = run(&["query", "run", &sql, "--param", "P=decimal:1.20"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"Exact\n1.20\n1.20\n");
    }
}

#[test]
fn decimal_precision_scale_and_carrier_failures_publish_no_prefix() {
    for parameter in [
        "P=decimal:100.00",
        "P=decimal:-100.00",
        "P=decimal:1.2",
        "P=decimal:1.200",
        "P=long:1",
        "P=double:1.2",
    ] {
        let output = run(&[
            "query",
            "run",
            "PARAMETERS P DECIMAL(4,2); SELECT 1 AS Prefix,P",
            "--param",
            parameter,
        ]);
        assert_eq!(output.status.code(), Some(2), "{parameter}");
        assert!(output.stdout.is_empty(), "{parameter}");
    }
    for declaration in [
        "DECIMAL(0,0)",
        "NUMERIC(29,0)",
        "DECIMAL(4,5)",
        "DECIMAL(256,0)",
    ] {
        let sql = format!("PARAMETERS P {declaration}; SELECT 1,P");
        let output = run(&["query", "run", &sql, "--param", "P=decimal:1.20"]);
        assert_eq!(output.status.code(), Some(2), "{declaration}");
        assert!(output.stdout.is_empty());
    }
}
