mod support;
use support::run;
#[test]
fn searches_preserve_utf16_positions_defaults_and_buffered_errors() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT InStr(1,'😀ababa','aba',0) AS F,InStrRev('😀ababa','aba',6) AS R,InStrRev('abc','b',,0) AS D",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"F\tR\tD\n3\t3\t2\n");
        for expression in [
            "InStr('a','a')",
            "InStrRev('a','a',0)",
            "InStrRev(NULL,'a',-1,NULL)",
            "InStr(1,NULL,'a',1/0)",
            "InStr(,'a','a',0)",
        ] {
            let result = run(&["query", command, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn explicit_ordinal_search_is_separate_from_locale_comparison_and_reverse_default() {
    let result = run(&[
        "query",
        "run",
        "SELECT InStr('😀ababa','aba') AS A,InStr(,'aba','a',) AS B,InStrRev('😀ababa','aba',,-1) AS C,InStrRev('😀ababa','😀',1,2) AS D",
        "--collation",
        "ordinal",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"A\tB\tC\tD\n3\t1\t5\t0\n");
    for expression in ["InStr(1,'A','a',1)", "InStrRev('A','a',-1,1)"] {
        let result = run(&[
            "query",
            "run",
            &format!("SELECT 1,{expression}"),
            "--collation",
            "ordinal",
        ]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn explicit_lcid_never_borrows_the_database_or_global_general_provider() {
    for command in ["eval", "run"] {
        for policy in ["ordinal", "general-legacy", "general-2010"] {
            let result = run(&[
                "query",
                command,
                "SELECT 1,InStr(1,'private-text','TEXT',1033)",
                "--collation",
                policy,
            ]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&result.stderr).contains("private-text"));
        }
        let result = run(&["query", command, "SELECT InStr(1,NULL,'x',1033) AS N"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\nNULL\n");
        let result = run(&["query", command, "SELECT InStr(1,NULL,'x',1048576)"]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn search_and_replacement_states_keep_eager_failures_buffered() {
    for command in ["eval", "run"] {
        for (expression, diagnostic) in [
            ("InStr(CVErr(1),1/0)", "division by zero"),
            ("InStrRev(CVErr(1),1/0)", "division by zero"),
            ("InStrRev(CVErr(1),'a',0)", "domain"),
            ("Replace(CVErr(1),'a',1/0)", "division by zero"),
            ("Replace(CVErr(1),'a','X',0)", "domain"),
            ("Replace(NULL,CVErr(1),'X')", "invalid use of NULL"),
        ] {
            let result = run(&["query", command, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
