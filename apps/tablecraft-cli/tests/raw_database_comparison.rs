mod support;

use support::run;

#[test]
fn raw_database_strcomp_uses_explicit_general_policies_and_keeps_buffered_errors() {
    for collation in ["general-legacy", "general-2010"] {
        let result = run(&[
            "query",
            "run",
            "SELECT StrComp(ChrW(55296),ChrW(55296),2) AS A,StrComp(ChrW(55296),ChrW(55296)) AS B,StrComp(ChrW(55296),ChrW(55296),-1) AS C,StrComp(ChrB(65),NULL,2) AS D",
            "--collation",
            collation,
        ]);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(result.stdout, b"A\tB\tC\tD\n0\t0\t0\tNULL\n");

        for sql in [
            "SELECT StrComp(ChrB(65),ChrW(65),2)",
            "SELECT StrComp(ChrW(65),'a',1)",
            "SELECT StrComp(ChrB(65),NULL,3)",
            "SELECT StrComp(ChrW(55296),ChrW(55296),2),1/0",
        ] {
            let result = run(&["query", "run", sql, "--collation", collation]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
