mod support;

use support::run;

#[test]
fn raw_predicates_keep_null_results_and_buffer_eager_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT ChrW(55296)=NULL AS A,NULL<>ChrB(65) AS B,ChrW(55296) IN(NULL) AS C,ChrB(65) BETWEEN NULL AND NULL AS D",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tB\tC\tD\nNULL\tNULL\tNULL\tNULL\n");

        let result = run(&["query", mode, "SELECT 1,ChrW(55296) IN(NULL,1/0)"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }

    let result = run(&[
        "query",
        "run",
        "SELECT CVar(ChrW(55296))>CVar(0) AS V",
        "--declared-arithmetic",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"V\nTRUE\n");
}

#[test]
fn raw_like_null_uses_explicit_mode_and_buffers_later_failures() {
    for wildcard in ["ansi89", "ansi92"] {
        let result = run(&[
            "query",
            "run",
            "SELECT S LIKE NULL AS A,NULL NOT LIKE S AS B FROM (SELECT ChrW(55296) AS S) AS Q",
            "--like-mode",
            wildcard,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"A\tB\nNULL\tNULL\n");

        let result = run(&[
            "query",
            "run",
            "SELECT ChrB(65) LIKE NULL,1/0",
            "--like-mode",
            wildcard,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }

    let result = run(&["query", "run", "SELECT ChrB(65) LIKE NULL"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
