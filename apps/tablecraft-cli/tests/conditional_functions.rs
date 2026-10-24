mod support;

use support::run;

#[test]
fn decimal_error_conversion_refuses_buffered_results_and_accepts_explicit_codes() {
    for mode in ["eval", "run"] {
        let result = run(&["query", mode, "SELECT CDec(CLng(CVErr(2001))) AS N"]);
        assert!(result.status.success());
        assert_eq!(result.stdout, b"N\n2001\n");

        for (expression, diagnostic) in [
            ("CDec(CVErr(0))", "expression type mismatch"),
            ("CDec(IIf(True,CVErr(1),1/0))", "division by zero"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
        }
    }
}

#[test]
fn choose_indices_round_through_single_in_scalar_and_query_execution() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Choose(1.49999999,1,2) AS A,Choose('2.50000001',1,2,3) AS B,IsNull(Choose(0.50000001,1)) AS C,IsNull(Choose(1e30,1)) AS D",
            "--number-format",
            "invariant",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tB\tC\tD\n2\t2\tTRUE\tTRUE\n");
        for (expression, diagnostic) in [
            ("Choose(1e100,1)", "numeric overflow"),
            ("Choose('1e1000',1)", "numeric overflow"),
            ("Choose(1e100,1,1/0)", "division by zero"),
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "invariant",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
        }
    }
}
