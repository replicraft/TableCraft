mod support;
use support::run;

#[test]
fn query_literals_use_us_dates_and_times_without_a_regional_text_policy() {
    for mode in ["eval", "run"] {
        for options in [
            vec![],
            vec!["--date-format", "de-de", "--two-digit-year-max", "2049"],
        ] {
            let mut arguments = vec![
                "query",
                mode,
                "SELECT Month(#1/2/2026#) AS M,Day(#1/2/2026#) AS D,Hour(#2:24PM#) AS H,CDbl(#12/29/1899 06:00:00#) AS N,Year(#7-Mar-2017#) AS Y",
            ];
            arguments.extend(options);
            let result = run(&arguments);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"M\tD\tH\tN\tY\n1\t2\t14\t-1.25\t2017\n");
        }
        for sql in [
            "SELECT 1,#2/29/1900#",
            "SELECT 1,#1//2/26#",
            "SELECT 1,#Mar 7#",
            "SELECT 1,#32/13/2026#",
            "SELECT 1,#12/31/2024 24:00#",
        ] {
            let result = run(&["query", mode, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn sql_literal_extension_does_not_change_iso_typed_parameter_admission() {
    let sql = "PARAMETERS P DATETIME; SELECT Month(P) AS M,Day(#1/2/2026#) AS D";
    let result = run(&["query", "run", sql, "--param", "P=date:2026-01-02"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"M\tD\n1\t2\n");

    let result = run(&["query", "run", sql, "--param", "P=date:1/2/2026"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn sql_token_years_keep_the_fixed_mapping_beside_explicit_constructor_windows() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Year(#1/2/29#) AS A,Year(#1/2/30#) AS B,Year(#1/2/49#) AS C,Year(DateSerial(49,1,2)) AS D,Day(#31/12/26#) AS E",
            "--two-digit-year-max",
            "2049",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"A\tB\tC\tD\tE\n2029\t1930\t1949\t2049\t26\n"
        );
    }
}

#[test]
fn explicit_reference_year_resolves_sql_tokens_and_keeps_runtime_text_policy_separate() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Year(#Mar 7#) AS Y,Day(#29/2#) AS D,Year(#1/2/49#) AS Fixed,Hour(#Mar 7 6:05 PM#) AS H",
            "--reference-year",
            "2024",
            "--date-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Y\tD\tFixed\tH\n2024\t29\t1949\t18\n");

        let result = run(&["query", mode, "SELECT 1,#2/29#", "--reference-year", "2023"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
