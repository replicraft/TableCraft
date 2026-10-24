mod support;

use support::run;

#[test]
fn raw_binary_searches_run_in_both_cli_evaluators_with_buffered_failures() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT InStr(1,ChrW(55296)&'a',ChrW(55296),0) AS F,InStrRev('a'&ChrW(55296),ChrW(55296)) AS R",
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"F\tR\n1\t2\n");

        for sql in [
            "SELECT InStrRev(ChrW(55296),ChrW(55296)),1/0",
            "SELECT InStr(1,ChrB(65),'',0)",
            "SELECT InStr(0,ChrW(55296),NULL,0)",
            "SELECT InStr(1,ChrW(55296),ChrW(55296),1)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}

#[test]
fn raw_general_searches_use_explicit_database_callbacks_in_query_run() {
    let command = "run";
    for collation in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            command,
            "SELECT InStr(1,ChrW(55296)&'Straße','ss',2) AS F,InStrRev(ChrW(55296)&'SS ß','ss',-1,2) AS R,InStr(1,ChrB(65),NULL,2) AS N",
            "--collation",
            collation,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"F\tR\tN\n6\t5\tNULL\n");

        for sql in [
            "SELECT InStr(1,ChrB(65),'',2)",
            "SELECT InStrRev(ChrW(55296),ChrW(55296),-1,2),1/0",
            "SELECT InStr(1,ChrW(55296),ChrW(55296),1033)",
        ] {
            let result = run(&["query", command, sql, "--collation", collation]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
