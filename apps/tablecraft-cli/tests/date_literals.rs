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
            "SELECT 1,#1/2/26#",
            "SELECT 1,#Mar 7#",
            "SELECT 1,#31/12/2026#",
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
