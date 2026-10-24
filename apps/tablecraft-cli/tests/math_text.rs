mod support;
use support::run;

#[test]
fn math_numeric_text_uses_explicit_format_and_buffers_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Abs('-2,5') AS A,RGB('2,5','1,5','3,5') AS R",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tR\n2.5\t262658\n");
        let failure = run(&["query", mode, "SELECT 1,Abs('-2,5')"]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("number-format"));
    }
}

#[test]
fn math_error_arguments_keep_eager_failures_and_ordered_validation() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("RGB(CVErr(1),1/0,0)", "division by zero"),
            ("Round(CVErr(1),1/0)", "division by zero"),
            ("Abs(CVErr(1))", "type mismatch"),
            ("Round(CVErr(1),-1)", "domain"),
            ("RGB(-1,CVErr(1),0)", "domain"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
    }
}

#[test]
fn null_math_union_arms_preserve_other_carriers_and_buffer_late_failures() {
    for function in [
        "Abs", "Int", "Fix", "Round", "Sgn", "Sqr", "Exp", "Log", "Sin", "Cos", "Tan", "Atn",
    ] {
        let sql = format!(
            "SELECT N,VarType(N) AS Kind FROM (SELECT {function}(NULL) AS N UNION ALL SELECT CByte(2)) AS Q"
        );
        let result = run(&["query", "run", &sql]);
        assert!(
            result.status.success(),
            "{function}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tKind\nNULL\t1\n2\t17\n", "{function}");
    }

    for sql in [
        "SELECT CByte(2) AS N UNION ALL SELECT Round(NULL,-1)",
        "SELECT CByte(2) AS N UNION ALL SELECT Round(NULL,1/0)",
    ] {
        let result = run(&["query", "run", sql]);
        assert_eq!(result.status.code(), Some(2), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
    }
}
