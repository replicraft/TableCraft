mod support;

use support::run;

#[test]
fn raw_extrema_allow_lossless_observers_and_keep_strict_buffered_output() {
    let result = run(&[
        "query",
        "run",
        "SELECT LenB(Min(S)) AS N FROM (SELECT ChrW(55296) AS S) AS Q",
    ]);
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(result.stdout, b"N\n2\n");

    for collation in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "SELECT LenB(Min(S)) AS L,StrComp(Max(S),ChrW(55296)&'b',0) AS H FROM (SELECT ChrW(55296)&'b' AS S UNION ALL SELECT ChrW(55296)&'a') AS Q",
            "--collation",
            collation,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"L\tH\n4\t0\n");

        for sql in [
            "SELECT Min(ChrW(55296))",
            "SELECT LenB(Min(ChrW(55296/(2-ID)))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
        ] {
            let result = run(&["query", "run", sql, "--collation", collation]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
