mod support;
use support::run;

#[test]
fn general_like_supports_utf16_counts_and_literal_or_class_expansions() {
    for policy in ["general-legacy", "general-2010"] {
        for mode in ["ansi89", "ansi92"] {
            let one = if mode == "ansi89" { "?" } else { "_" };
            let many = if mode == "ansi89" { "*" } else { "%" };
            let sql = format!(
                "SELECT '😀' LIKE '{one}' AS OneUnit,'😀' LIKE '{one}{one}' AS TwoUnits,'ss' LIKE 'ß' AS Literal,'ss' LIKE '[ß]' AS ClassValue,'æ' LIKE 'a[]e' AS EmptyGroup,'abc' LIKE '[a-z]' AS WholeWord,'a' LIKE 'a ' AS SpaceValue,'ABC' LIKE 'a{many}' AS PrefixValue"
            );
            let result = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--like-mode",
                mode,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout,b"OneUnit\tTwoUnits\tLiteral\tClassValue\tEmptyGroup\tWholeWord\tSpaceValue\tPrefixValue\nFALSE\tTRUE\tTRUE\tTRUE\tTRUE\tFALSE\tFALSE\tTRUE\n");
        }
    }
    let result = run(&[
        "query",
        "run",
        "SELECT '😀' LIKE '?' AS R,'ss' LIKE 'ß' AS E",
        "--collation",
        "ordinal",
        "--like-mode",
        "ansi89",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"R\tE\nTRUE\tFALSE\n");
}

#[test]
fn negated_class_prefixes_nulls_and_bad_patterns_have_explicit_results() {
    for policy in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "SELECT 'ss' LIKE '[!ß]*' AS Excluded,'ss' LIKE '[!b]' AS Allowed,NULL LIKE '[z-a]' AS NullValue,'ss' NOT LIKE '[ß]' AS Negated",
            "--collation",
            policy,
            "--like-mode",
            "ansi89",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"Excluded\tAllowed\tNullValue\tNegated\nFALSE\tTRUE\tNULL\tFALSE\n"
        );
        for pattern in ["[z-a]", "[!]", "[a-b-c]"] {
            let sql = format!("SELECT 1 AS BeforeValue,'private-value' LIKE '{pattern}' AS R");
            let result = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--like-mode",
                "ansi89",
            ]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&result.stderr).contains("private-value"));
        }
    }
}

#[test]
fn full_literals_retain_long_suffixes_without_prefix_index_or_space_coercion() {
    let left = "L=text:".to_owned() + &"x".repeat(1024) + "a";
    let right = "R=text:".to_owned() + &"x".repeat(1024) + "A";
    for policy in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "PARAMETERS L TEXT,R TEXT; SELECT L LIKE R AS R",
            "--collation",
            policy,
            "--like-mode",
            "ansi89",
            "--param",
            &left,
            "--param",
            &right,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"R\nTRUE\n");
    }
}

#[test]
fn cumulative_like_limit_discards_previously_evaluated_rows() {
    fn source(rows: usize) -> String {
        (1..=rows)
            .map(|n| {
                if n == 1 {
                    "SELECT 1 AS ID".into()
                } else {
                    format!("SELECT {n}")
                }
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ")
    }
    let left = "L=text:".to_owned() + &"x".repeat(1024) + "a";
    let right = "R=text:".to_owned() + &"x".repeat(1024) + "A";
    for count in [8, 16] {
        let sql = format!(
            "PARAMETERS L TEXT,R TEXT; SELECT a.ID,L LIKE R AS Matched FROM ({}) AS a,({}) AS b",
            source(16),
            source(count)
        );
        for policy in ["general-legacy", "general-2010"] {
            let output = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--like-mode",
                "ansi89",
                "--param",
                &left,
                "--param",
                &right,
            ]);
            if count == 8 {
                assert!(output.status.success());
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap().lines().count(),
                    129
                );
            } else {
                assert!(!output.status.success());
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("Bytes resource limit"));
            }
        }
    }
}

#[test]
fn ordinary_contains_searches_prune_impossible_spans_within_unchanged_limits() {
    for policy in ["general-legacy", "general-2010"] {
        for (pattern, expected) in [("*a*", true), ("*[a-z]*", true), ("*z*", false)] {
            let source = "a".repeat(128);
            let sql = format!("SELECT '{source}' LIKE '{pattern}' AS R");
            let output = run(&[
                "query",
                "run",
                &sql,
                "--collation",
                policy,
                "--like-mode",
                "ansi89",
            ]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                output.stdout,
                if expected {
                    b"R\nTRUE\n".as_slice()
                } else {
                    b"R\nFALSE\n".as_slice()
                }
            );
        }
    }
}
