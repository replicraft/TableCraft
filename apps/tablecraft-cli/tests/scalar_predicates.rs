mod support;
use support::run;

#[test]
fn source_free_predicates_preserve_reversed_bounds_exact_int64_and_null() {
    let result = run(&[
        "query",
        "eval",
        "SELECT 2 BETWEEN 3 AND 1 AS InRange, 9007199254740993 IN (9007199254740992,9007199254740993) AS Exact, 2 NOT IN (1,Null) AS [Unknown]",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"InRange\tExact\tUnknown\nTRUE\tTRUE\tNULL\n"
    );
    for sql in ["SELECT 1 IN (1,1/0)", "SELECT 'secret' IN ('secret')"] {
        let result = run(&["query", "eval", sql]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("secret"));
    }
}

#[test]
fn builtins_share_scalar_and_retrieval_results_without_partial_error_output() {
    let sql = "SELECT IsNull(Null) AS MissingValue, IsNull('') AS EmptyText, IIf(Null,1,2) AS Chosen, IIf(9223372036854775807,'yes','no') AS Exact";
    for command in ["eval", "run"] {
        let result = run(&["query", command, sql]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"MissingValue\tEmptyText\tChosen\tExact\nTRUE\tFALSE\t2\t\"\"\"yes\"\"\"\n"
        );
        for sql in [
            "SELECT IIf(True,1,1/0)",
            "SELECT IIf(True,1)",
            "SELECT VBA.IIf(True,1,0)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn nz_query_context_is_the_same_in_eval_and_run() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT IsNull(Nz(Null)) AS EmptyDefault, Nz(Null,Null) AS ExplicitNull, Nz(Null,42) AS Fallback",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"EmptyDefault\tExplicitNull\tFallback\nFALSE\tNULL\t42\n"
        );
        let result = run(&["query", command, "SELECT Nz(1,1/0)"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn common_text_functions_use_shared_unicode_rules_without_partial_output() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Len('é🦀') AS Units, Left('é🦀',1) AS Prefix, IsNull(Trim(Null)) AS NullResult, Len(Trim('  x  ')) AS Trimmed",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            "Units\tPrefix\tNullResult\tTrimmed\n3\t\"\"\"é\"\"\"\tTRUE\t1\n".as_bytes()
        );
        for sql in [
            "SELECT 1,Left('🦀',1)",
            "SELECT Mid('abc',0)",
            "SELECT Len(*)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn date_components_share_scalar_and_retrieval_paths() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Year(#2026-03-04 12:34:56#) AS Y, Month(#2026-03-04#) AS M, Day(#2026-03-04#) AS D, Hour(#1899-12-29 06:00:00#) AS H, Minute(#2026-03-04 12:34:56#) AS N, Second(#2026-03-04 12:34:56#) AS S, Year(Null) AS Absent",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"Y\tM\tD\tH\tN\tS\tAbsent\n2026\t3\t4\t6\t34\t56\tNULL\n"
        );
        for sql in [
            "SELECT Second('12:34:56')",
            "SELECT Year(*)",
            "SELECT Year(1/0)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn date_add_is_shared_by_scalar_and_retrieval_with_buffered_errors() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Day(DateAdd('m',1,#2024-01-31#)) AS MonthEnd, Day(DateAdd('w',1,#2026-01-02#)) AS Weekend, Day(DateAdd('d',2.5,#2026-01-01#)) AS Rounded, DateAdd('d',1,Null) AS Absent",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"MonthEnd\tWeekend\tRounded\tAbsent\n29\t3\t3\tNULL\n"
        );
        for sql in [
            "SELECT 1, DateAdd('bad',1,#2026-01-01#)",
            "SELECT DateAdd('d',1/0,Null)",
            "SELECT DateAdd('d',-1,#0100-01-01#)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn date_diff_preserves_optional_omission_and_week_boundaries() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT DateDiff('yyyy',#2025-12-31#,#2026-01-01#) AS Y, DateDiff('w',#2026-01-05#,#2026-01-11#) AS FullWeeks, DateDiff('ww',#2026-01-05#,#2026-01-11#) AS Boundaries, DateDiff('ww',#2026-01-04#,#2026-01-05#,2) AS Monday, DateDiff('ww',#2026-01-04#,#2026-01-05#,,2) AS Omitted",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"Y\tFullWeeks\tBoundaries\tMonday\tOmitted\n1\t0\t1\t1\t0\n"
        );
        for sql in [
            "SELECT DateDiff('ww',#2026-01-04#,#2026-01-05#,0)",
            "SELECT DateDiff('d',Null,Null,1/0)",
            "SELECT DateDiff('d',#2026-01-04#,#2026-01-05#,Null)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn date_part_preserves_the_documented_monday_anomaly_and_optional_defaults() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT DatePart('ww',#2003-12-29#,2,2) AS Monday, DatePart('ww',#2003-12-30#,2,2) AS Tuesday, DatePart('y',#2024-02-29#) AS YearDay, DatePart('w',#2026-01-01#,2) AS Weekday, DatePart('ww',#2026-01-01#,,3) AS FullWeek",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"Monday\tTuesday\tYearDay\tWeekday\tFullWeek\n53\t1\t60\t4\t52\n"
        );
        for sql in [
            "SELECT DatePart('d',Null,1/0)",
            "SELECT DatePart('ww',#2026-01-01#,0)",
            "SELECT DatePart('ww',#2026-01-01#,Null)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn concrete_conversions_preserve_boolean_byte_exception_exact_int64_and_null_errors() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT CByte(True) AS ByteValue, CInt(True) AS IntValue, CLng(2.5) AS Rounded, CLngLng(9007199254740993) AS Exact, CBool(0.1) AS BoolValue",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"ByteValue\tIntValue\tRounded\tExact\tBoolValue\n255\t-1\t2\t9007199254740993\tTRUE\n"
        );
        for sql in [
            "SELECT 1,CInt(Null)",
            "SELECT CByte(-1)",
            "SELECT CInt(32767.5)",
            "SELECT CBool(Null)",
            "SELECT CInt('2')",
            "SELECT CLng(1/0)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn explicit_ieee_conversions_share_cli_paths_and_preserve_checked_failures() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT CSng(16777217) AS RoundedSingle, CDbl(9007199254740993) AS RoundedDouble, CDbl(True) AS BoolValue",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"RoundedSingle\tRoundedDouble\tBoolValue\n16777216\t9007199254740992\t-1\n"
        );
        for sql in [
            "SELECT 1,CSng(1e100)",
            "SELECT CDbl(Null)",
            "SELECT CDbl('1.25')",
            "SELECT CSng(*)",
            "SELECT VBA.CDbl(1)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn currency_conversion_rounds_numeric_inputs_and_buffers_invalid_results() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT CCur(1.25) AS MoneyValue,CCur(True) AS BoolValue,CCur(0.03125) AS TieValue",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"MoneyValue\tBoolValue\tTieValue\n1.2500\t-1.0000\t0.0312\n"
        );
        for sql in [
            "SELECT 1,CCur(Null)",
            "SELECT CCur(922337203685478)",
            "SELECT CCur('1.25')",
            "SELECT CCur(*)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn decimal_conversion_exceeds_field_precision_without_losing_digits() {
    for command in ["eval", "run"] {
        let output = run(&[
            "query",
            command,
            "SELECT CDec(2^95) AS Wide,CDec(0.1) AS Fraction,CDec(True) AS BoolValue",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"Wide\tFraction\tBoolValue\n39614081257132168796771975168\t0.1\t-1\n"
        );
        for sql in [
            "SELECT 1,CDec(Null)",
            "SELECT CDec(2^96)",
            "SELECT CDec('0.1')",
            "SELECT CDec(*)",
            "SELECT IIf(True,1,CDec(Null))",
        ] {
            let output = run(&["query", command, sql]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
        }
    }
}
