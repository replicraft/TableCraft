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

#[test]
fn raw_like_complete_units_use_explicit_general_profiles() {
    for collation in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "SELECT S LIKE '?' AS A,S LIKE ChrW(55296) AS B,S LIKE ChrW(55296)&'*' AS C FROM (SELECT ChrW(55296) AS S) AS Q",
            "--like-mode",
            "ansi89",
            "--collation",
            collation,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"A\tB\tC\nTRUE\tTRUE\tTRUE\n");

        let result = run(&[
            "query",
            "run",
            "SELECT ChrB(65) LIKE '*'",
            "--like-mode",
            "ansi89",
            "--collation",
            collation,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn raw_relational_comparisons_use_general_sql_space_semantics() {
    for collation in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "SELECT S=ChrW(55296)&'a' AS A,S IN(ChrW(55296)&'A') AS B,LeftB('a',2)='A ' AS C FROM (SELECT ChrW(55296)&'A ' AS S) AS Q",
            "--collation",
            collation,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"A\tB\tC\nTRUE\tTRUE\tTRUE\n");

        let result = run(&[
            "query",
            "run",
            "SELECT ChrW(55296)=ChrW(55296),1/0",
            "--collation",
            collation,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
