mod support;
use support::run;

fn successful(sql: &str, policy: &str) -> Vec<u8> {
    let output = run(&["query", "run", sql, "--collation", policy]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

#[test]
fn explicit_general_policies_control_scalar_and_relational_comparison() {
    for policy in ["general-legacy", "general-2010"] {
        assert_eq!(
            successful(
                "SELECT StrComp('a','A',2) AS ExplicitMode,StrComp('a','A') AS DefaultMode",
                policy
            ),
            b"ExplicitMode\tDefaultMode\n0\t0\n"
        );
        assert_eq!(
            successful(
                "SELECT 'a'='A ' AS CaseEq, 'é'='é' AS AccentEq, 'ß'='ss' AS ExpansionEq, NULL='' AS NullEq",
                policy
            ),
            b"CaseEq\tAccentEq\tExpansionEq\tNullEq\nTRUE\tTRUE\tTRUE\tNULL\n"
        );
        assert_eq!(
            successful(
                "SELECT Min(q.ID) AS ID,Count(*) AS N FROM (SELECT 1 AS ID,'a' AS V UNION ALL SELECT 2,'A ' UNION ALL SELECT 3,'b') AS q GROUP BY q.V ORDER BY q.V",
                policy
            ),
            b"ID\tN\n1\t2\n3\t1\n"
        );
        assert_eq!(
            successful(
                "SELECT a.ID,b.ID AS B FROM (SELECT 1 AS ID,'a' AS V) AS a INNER JOIN (SELECT 2 AS ID,'A ' AS V) AS b ON a.V=b.V",
                policy
            ),
            b"ID\tB\n1\t2\n"
        );
        assert_eq!(
            successful(
                "SELECT Count(*) AS N FROM (SELECT 'a' AS V UNION SELECT 'A ') AS q",
                policy
            ),
            b"N\n1\n"
        );
        assert_eq!(
            successful(
                "SELECT TOP 1 q.ID FROM (SELECT 1 AS ID,'a' AS V UNION ALL SELECT 2,'A ' UNION ALL SELECT 3,'b') AS q ORDER BY q.V",
                policy
            ),
            b"ID\n1\n2\n"
        );
    }
    assert_eq!(successful("SELECT 'a'='A ' AS R", "ordinal"), b"R\nFALSE\n");
    let absent = run(&["query", "run", "SELECT 'a'='A ' AS R"]);
    assert!(!absent.status.success());
    assert!(absent.stdout.is_empty());
}

#[test]
fn general_profiles_retain_full_long_text_values_including_parameter_suffixes() {
    let left = "x".repeat(1024) + "a";
    let right = "x".repeat(1024) + "b";
    let equivalent = "x".repeat(1024) + "A  ";
    for policy in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "PARAMETERS L TEXT, R TEXT, E TEXT; SELECT L<R AS Less,L=E AS Equal",
            "--collation",
            policy,
            "--param",
            &format!("L=text:{left}"),
            "--param",
            &format!("R=text:{right}"),
            "--param",
            &format!("E=text:{equivalent}"),
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Less\tEqual\nTRUE\tTRUE\n");
    }
}

#[test]
fn cumulative_comparison_exhaustion_and_unsupported_search_publish_no_partial_results() {
    let inline = (1..=32)
        .map(|n| {
            if n == 1 {
                "SELECT 1 AS ID".to_string()
            } else {
                format!("SELECT {n}")
            }
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let sql = format!("PARAMETERS P TEXT, Q TEXT; SELECT q.ID FROM ({inline}) AS q WHERE P=Q");
    let left = format!("P=text:{}", "a".repeat(4096));
    let right = format!("Q=text:{}", "A".repeat(4096));
    for policy in ["general-legacy", "general-2010"] {
        let exhausted = run(&[
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
        assert!(!exhausted.status.success());
        assert!(exhausted.stdout.is_empty());
        assert!(String::from_utf8_lossy(&exhausted.stderr).contains("limit"));
        let search = run(&[
            "query",
            "run",
            "SELECT InStr(1,'a','A',1) AS R",
            "--collation",
            policy,
        ]);
        assert!(!search.status.success());
        assert!(search.stdout.is_empty());
        let update = run(&[
            "query",
            "update",
            "UPDATE T SET ID=1",
            "--database",
            "missing.mdb",
            "--output",
            "never-created.mdb",
            "--collation",
            policy,
        ]);
        assert!(!update.status.success());
        assert!(update.stdout.is_empty());
        assert!(String::from_utf8_lossy(&update.stderr).contains("invalid value"));
    }
}

#[test]
fn general_like_uses_explicit_utf16_wildcards() {
    for policy in ["general-legacy", "general-2010"] {
        for (mode, pattern) in [("ansi89", "a*"), ("ansi92", "a%")] {
            let sql = format!("SELECT 'ABC' LIKE '{pattern}' AS R,'😀' LIKE '?' AS OneUnit");
            let result = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--like-mode",
                mode,
            ]);
            assert!(result.status.success());
            // General counts raw UTF-16 units; ANSI-92 also keeps ? literal.
            assert_eq!(result.stdout, b"R\tOneUnit\nTRUE\tFALSE\n");
        }
    }
}
