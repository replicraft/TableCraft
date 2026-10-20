mod support;
use support::run;
#[test]
fn explicit_binary_mode_works_in_eval_and_run_with_buffered_refusals() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StrComp('ABCD','abcd',0) AS C,StrComp(NULL,'x',0) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"C\tN\n-1\tNULL\n");
        for sql in [
            "SELECT 1,StrComp('a','b')",
            "SELECT 1,StrComp(NULL,'b',NULL)",
            "SELECT 1,StrComp(NULL,'b',1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn ordinal_database_policy_does_not_pretend_to_be_a_locale_text_provider() {
    let result = run(&[
        "query",
        "run",
        "SELECT StrComp('A','a') AS A,StrComp('A','a',2) AS B,StrComp('A','a',) AS C",
        "--collation",
        "ordinal",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"A\tB\tC\n-1\t-1\t-1\n");
    let result = run(&[
        "query",
        "run",
        "SELECT 1,StrComp('A','a',1)",
        "--collation",
        "ordinal",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
}
