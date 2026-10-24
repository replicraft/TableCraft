#[path = "support/mod.rs"]
mod cli_support;
use cli_support::run;

#[test]
fn declared_integral_flag_preserves_defaults_and_keeps_failures_buffered() {
    let sql = "SELECT 1,CInt(32767)+CInt(1)";
    let default = run(&["query", "run", sql]);
    assert!(default.status.success());
    let strict = run(&["query", "run", sql, "--declared-integral"]);
    assert_eq!(strict.status.code(), Some(2));
    assert!(strict.stdout.is_empty());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("overflow"));
    let variant = run(&[
        "query",
        "run",
        "SELECT Q.N+CInt(1) AS N FROM (SELECT CVar(CInt(32767)) AS N) AS Q",
        "--declared-integral",
    ]);
    assert!(
        variant.status.success(),
        "{}",
        String::from_utf8_lossy(&variant.stderr)
    );
    assert_eq!(variant.stdout, b"N\n32768\n");
    let unknown = run(&[
        "query",
        "run",
        "SELECT First(CInt(32767))+CInt(1)",
        "--declared-integral",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("declaration is unknown"));
}

#[test]
fn unknown_fixed_width_results_do_not_need_a_widening_declaration() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT Count(*)+CInt(1) AS A,First(CInt(-32768)) Mod CInt(-1) AS B,-First(CByte(255)) AS C",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tB\tC\n2\t0\t-255\n");

        let result = run(&[
            "query",
            "run",
            "SELECT 1,First(CLngLng(9223372036854775807))+CInt(1)",
            flag,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("numeric overflow"));
    }
}

#[test]
fn declared_integral_flag_is_scoped_to_query_execution() {
    for mode in ["eval", "parse", "saved", "update"] {
        let result = run(&["query", mode, "SELECT 1", "--declared-integral"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let help = run(&["query", "run", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--declared-arithmetic"));
    assert!(String::from_utf8_lossy(&help.stdout).contains("declared-integral"));
}

#[test]
fn declared_comparisons_distinguish_variant_categories_and_buffer_failures() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&["query", "run", "SELECT CVar(1)<CVar('bad') AS N", flag]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\nTRUE\n");
        let failure = run(&["query", "run", "SELECT CVar(1)<CVar('bad'),1/0", flag]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("division by zero"));
    }
    let default = run(&["query", "run", "SELECT CVar(1)<CVar('bad') AS N"]);
    assert_eq!(default.status.code(), Some(2));
    assert!(default.stdout.is_empty());
    assert!(String::from_utf8_lossy(&default.stderr).contains("number-format"));
    let ordinary = run(&[
        "query",
        "run",
        "SELECT CVar(1)=CVar('1') AS N",
        "--number-format",
        "en-us",
    ]);
    assert!(ordinary.status.success());
    assert_eq!(ordinary.stdout, b"N\nTRUE\n");
}

#[test]
fn like_and_null_check_categories_reach_the_cli() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let variant = run(&[
            "query",
            "run",
            "SELECT CInt(-32768)\\('abc' LIKE CVar('a*')) AS N",
            "--like-mode",
            "ansi89",
            "--collation",
            "ordinal",
            flag,
        ]);
        assert!(
            variant.status.success(),
            "{}",
            String::from_utf8_lossy(&variant.stderr)
        );
        assert_eq!(variant.stdout, b"N\n32768\n");
        let scalar = run(&[
            "query",
            "run",
            "SELECT CInt(-32768)\\('abc' LIKE 'a*') AS N",
            "--like-mode",
            "ansi89",
            "--collation",
            "ordinal",
            flag,
        ]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
        let null = run(&[
            "query",
            "run",
            "SELECT CInt(-32768)\\IsNull(CVar(NULL))",
            flag,
        ]);
        assert_eq!(null.status.code(), Some(2));
        assert!(null.stdout.is_empty());
        assert!(String::from_utf8_lossy(&null.stderr).contains("overflow"));
    }
}

#[test]
fn list_range_and_subquery_categories_reach_the_cli() {
    let variant = run(&[
        "query",
        "run",
        "SELECT CInt(-32768)\\(CVar(1) IN (1,2)) AS N,CInt(-32768)\\(1 BETWEEN CVar(0) AND 2) AS R,CInt(-32768)\\(CVar(1)=ALL(SELECT 1 WHERE False)) AS Q",
        "--declared-arithmetic",
    ]);
    assert!(
        variant.status.success(),
        "{}",
        String::from_utf8_lossy(&variant.stderr)
    );
    assert_eq!(variant.stdout, b"N\tR\tQ\n32768\t32768\t32768\n");
    for expression in ["1 IN (1,2)", "EXISTS(SELECT CVar(NULL))"] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        let scalar = run(&["query", "run", &sql, "--declared-arithmetic"]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
    }
}

#[test]
fn rounded_operand_declarations_keep_scalar_overflow_and_variant_widening() {
    for (expression, succeeds) in [
        ("CInt(-32768)\\CDbl(-1)", false),
        ("CVar(CInt(-32768))\\CDbl(-1)", true),
        ("CInt(-32768)\\CVar(CDbl(-1))", true),
        ("CInt(-32768)\\CDate(-1)", false),
        ("CInt(-32768)\\CVDate(-1)", true),
        ("CInt(-32768)\\CDate(CVar(-1))", false),
        ("CInt(-32768)\\CVar(CDate(-1))", true),
    ] {
        let sql = format!("SELECT {expression} AS N");
        let result = run(&["query", "run", &sql, "--declared-integral"]);
        assert_eq!(
            result.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if succeeds {
            assert_eq!(result.stdout, b"N\n32768\n");
        } else {
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("numeric overflow"));
        }
    }
}

#[test]
fn static_subquery_categories_survive_empty_results() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        for predicate in [
            "CInt(1) IN (SELECT CVar(CInt(1)))",
            "CInt(1)=ALL(SELECT CVar(CInt(1)) WHERE False)",
            "(SELECT CVar(CInt(1)) WHERE False)=ALL(SELECT CInt(0) WHERE False)",
        ] {
            let sql = format!("SELECT CInt(-32768)\\({predicate}) AS N");
            let result = run(&["query", "run", &sql, flag]);
            assert!(
                result.status.success(),
                "{sql}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"N\n32768\n");
        }
        let scalar = run(&[
            "query",
            "run",
            "SELECT 1,CInt(-32768)\\(CInt(1)=ALL(SELECT CInt(1) WHERE False))",
            flag,
        ]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
    }
}

#[test]
fn declared_date_envelope_overflow_is_buffered_and_variant_can_widen() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        for expression in ["CDate(2958465)+1", "-CDate(2958465)"] {
            let sql = format!("SELECT 1,{expression}");
            let result = run(&["query", "run", &sql, flag]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("overflow"));
        }
        let variant = run(&["query", "run", "SELECT CVDate(2958465)+1 AS N", flag]);
        assert!(
            variant.status.success(),
            "{}",
            String::from_utf8_lossy(&variant.stderr)
        );
        assert_eq!(variant.stdout, b"N\n2958466\n");
    }
}

#[test]
fn numeric_text_does_not_bypass_declared_date_overflow() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let scalar = run(&[
            "query",
            "run",
            "SELECT 1,CDate(2958465)+'1,0'",
            "--number-format",
            "de-de",
            flag,
        ]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
        let variant = run(&[
            "query",
            "run",
            "SELECT CDate(2958465)+CVar('1,0') AS N",
            "--number-format",
            "de-de",
            flag,
        ]);
        assert!(
            variant.status.success(),
            "{}",
            String::from_utf8_lossy(&variant.stderr)
        );
        assert_eq!(variant.stdout, b"N\n2958466\n");
    }
}

#[test]
fn effective_numeric_comparisons_are_explicit_and_keep_output_buffered() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT CInt(2)='1.5' AS N",
            "--number-format",
            "en-us",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\nTRUE\n");
        let overflow = run(&[
            "query",
            "run",
            "SELECT 1,CByte(1)='256'",
            "--number-format",
            "en-us",
            flag,
        ]);
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert!(String::from_utf8_lossy(&overflow.stderr).contains("overflow"));
    }
}

#[test]
fn date_numeric_predicates_keep_effective_types_and_buffer_range_failures() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT CDate(1)=CInt(1) AS N,CDate(-0.25)=CDec(0.25) AS D",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tD\nTRUE\tFALSE\n");
        let overflow = run(&["query", "run", "SELECT 1,CDate(1)=CLng(2958466)", flag]);
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert!(String::from_utf8_lossy(&overflow.stderr).contains("overflow"));
    }
}

#[test]
fn text_return_categories_and_chr_dollar_are_preserved_at_the_cli() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let variant = run(&[
            "query",
            "run",
            "SELECT CInt(-32768)\\Left('-1x',2) AS N,CVar(65)<Chr(65) AS C",
            "--number-format",
            "en-us",
            flag,
        ]);
        assert!(
            variant.status.success(),
            "{}",
            String::from_utf8_lossy(&variant.stderr)
        );
        assert_eq!(variant.stdout, b"N\tC\n32768\tTRUE\n");
        let scalar = run(&["query", "run", "SELECT 1,CInt(-32768)\\Val('-1')", flag]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
        let dollar = run(&[
            "query",
            "run",
            "SELECT 1,CVar(65)<Chr$(65)",
            "--number-format",
            "en-us",
            flag,
        ]);
        assert_eq!(dollar.status.code(), Some(2));
        assert!(dollar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&dollar.stderr).contains("type mismatch"));
    }
}

#[test]
fn context_return_categories_reach_empty_subqueries_without_activating_them() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let variant = run(&[
            "query",
            "run",
            "SELECT CInt(-32768)\\(0=ALL(SELECT Error(1) WHERE False)) AS N",
            flag,
        ]);
        assert!(
            variant.status.success(),
            "{}",
            String::from_utf8_lossy(&variant.stderr)
        );
        assert_eq!(variant.stdout, b"N\n32768\n");
        let scalar = run(&[
            "query",
            "run",
            "SELECT 1,CInt(-32768)\\(0=ALL(SELECT CurrentUser() WHERE False))",
            flag,
        ]);
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("overflow"));
    }
}

#[test]
fn unknown_negation_uses_agreement_without_assigning_a_category() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT -First(CInt(2)) AS I,-First(CLng(-2147483647)) AS L,CDbl(-First(CDate(-0.25))) AS D",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"I\tL\tD\n-2\t2147483647\t0.25\n");

        for sql in [
            "SELECT 1,-First(CInt(-32768))",
            "SELECT 1,-First(CLng(-2147483648))",
            "SELECT 1,-First(CDate(2958465))",
            "SELECT 1,(-First(CInt(2)))+CInt(-32767)",
            "SELECT -Q.N FROM (SELECT CInt(2) AS N UNION ALL SELECT CInt(-32768)) AS Q",
        ] {
            let failure = run(&["query", "run", sql, flag]);
            assert_eq!(failure.status.code(), Some(2), "{sql}");
            assert!(failure.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&failure.stderr).contains("declaration is unknown"),
                "{sql}"
            );
        }
    }
}

#[test]
fn unknown_binary_arithmetic_uses_agreement_and_keeps_late_failures_buffered() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT First(CInt(2))+CInt(3) AS I,First(CSng(1.25))*CInt(2) AS S,First(CInt(7))\\'2.5' AS Q",
            "--number-format",
            "en-us",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"I\tS\tQ\n5\t2.5\t3\n");

        for (sql, error) in [
            (
                "SELECT Q.N+CInt(1) FROM (SELECT CInt(2) AS N UNION ALL SELECT CInt(32767)) AS Q",
                "declaration is unknown",
            ),
            ("SELECT 1,First(CInt(7))\\CInt(0)", "division by zero"),
            ("SELECT 1,First('bad')\\CInt(2)", "type mismatch"),
            (
                "SELECT CInt(-32768)\\((First(CInt(2))+CInt(1))=CInt(3))",
                "declaration is unknown",
            ),
        ] {
            let failure = run(&["query", "run", sql, "--number-format", "en-us", flag]);
            assert_eq!(failure.status.code(), Some(2), "{sql}");
            assert!(failure.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&failure.stderr).contains(error),
                "{sql}"
            );
        }
    }
}
