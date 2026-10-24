mod support;

use support::run;

#[test]
fn raw_keys_work_with_general_policies_and_keep_buffered_publication() {
    for collation in ["general-legacy", "general-2010"] {
        for (sql, expected) in [
            (
                "SELECT Count(*) AS N FROM (SELECT DISTINCT ChrW(55296) AS S) AS Q",
                "N\n1\n",
            ),
            (
                "SELECT ID FROM (SELECT 1 AS ID,ChrW(55296)&'b' AS S UNION ALL SELECT 2,ChrW(55296)&'a' UNION ALL SELECT 3,NULL) AS Q ORDER BY S",
                "ID\n3\n2\n1\n",
            ),
            (
                "SELECT TOP 1 ID FROM (SELECT 1 AS ID,ChrW(55296)&'a' AS S UNION ALL SELECT 2,ChrW(55296)&'A ') AS Q ORDER BY S",
                "ID\n1\n2\n",
            ),
        ] {
            let result = run(&["query", "run", sql, "--collation", collation]);
            assert!(result.status.success(), "{:?}", result.stderr);
            assert_eq!(result.stdout, expected.as_bytes(), "{sql}");
        }

        for sql in [
            "SELECT DISTINCT ChrW(55296)",
            "SELECT Count(*) FROM (SELECT ChrB(65) AS S UNION SELECT ChrB(65)) AS Q",
            "SELECT TOP 1 ID FROM (SELECT 1 AS ID,ChrW(55296) AS S UNION ALL SELECT 2,ChrW(1/0)) AS Q ORDER BY S",
        ] {
            let result = run(&["query", "run", sql, "--collation", collation]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
