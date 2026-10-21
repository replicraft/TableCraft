mod support;
use support::run;

#[test]
fn explicit_general_searches_and_replacements_support_unequal_length_matches() {
    for policy in ["general-legacy", "general-2010"] {
        for (sql, expected) in [
            (
                "SELECT InStr(1,'Straße STRASSE','ss',2) AS [First],InStrRev('Straße STRASSE','ss',-1,2) AS [Last]",
                "First\tLast\n5\t12\n",
            ),
            ("SELECT InStr('Straße','SS') AS R", "R\n5\n"),
            (
                "SELECT Replace('SS ß','ß','x',1,-1,2) AS R",
                "R\n\"\"\"x x\"\"\"\n",
            ),
            (
                "SELECT Replace('SS ß','ß','x',1,1,2) AS R",
                "R\n\"\"\"x ß\"\"\"\n",
            ),
            ("SELECT InStrRev('SS ß','ss',3,2) AS R", "R\n1\n"),
            ("SELECT InStr(1,'a' & Chr(0),Chr(0),2) AS R", "R\n2\n"),
            (
                "SELECT Replace('a' & Chr(0),Chr(0),'!',1,-1,2) AS R",
                "R\n\"\"\"a!\"\"\"\n",
            ),
            (
                "SELECT InStr(1,'abc','',2) AS [Empty],InStr(1,NULL,'x',2) AS NullValue",
                "Empty\tNullValue\n1\tNULL\n",
            ),
        ] {
            let output = run(&["query", "run", sql, "--collation", policy]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, expected.as_bytes(), "{policy}: {sql}");
        }
    }
}

#[test]
fn general_search_keeps_mode_defaults_and_payload_free_failures() {
    for policy in ["general-legacy", "general-2010"] {
        let output = run(&[
            "query",
            "run",
            "SELECT InStrRev('Straße','ss') AS ReverseBinary,Replace('Straße','ss','X') AS ReplaceBinary",
            "--collation",
            policy,
        ]);
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            "ReverseBinary\tReplaceBinary\n0\t\"\"\"Straße\"\"\"\n".as_bytes()
        );
        for sql in [
            "SELECT InStr(1,'private-text','A',1) AS R",
            "SELECT Replace('private-text','A','x',1,-1,1) AS R",
        ] {
            let output = run(&["query", "run", sql, "--collation", policy]);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&output.stderr).contains("private-text"));
        }
    }
}

#[test]
fn cumulative_search_limits_discard_all_prior_rows() {
    let left = "P=text:".to_owned() + &"x".repeat(1024) + "a";
    let right = "Q=text:".to_owned() + &"x".repeat(1024) + "A";
    for rows in [32, 64] {
        let source = (1..=rows)
            .map(|number| {
                if number == 1 {
                    "SELECT 1 AS ID".into()
                } else {
                    format!("SELECT {number}")
                }
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let sql = format!(
            "PARAMETERS P TEXT,Q TEXT; SELECT q.ID,InStr(1,P,Q,2) AS R FROM ({source}) AS q"
        );
        for policy in ["general-legacy", "general-2010"] {
            let result = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--param",
                &left,
                "--param",
                &right,
            ]);
            if rows == 32 {
                assert!(result.status.success());
                assert_eq!(
                    String::from_utf8(result.stdout).unwrap().lines().count(),
                    rows + 1
                );
            } else {
                assert!(!result.status.success());
                assert!(result.stdout.is_empty());
                assert!(String::from_utf8_lossy(&result.stderr).contains("limit"));
            }
        }
    }
}
