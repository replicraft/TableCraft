mod support;

use support::run;

#[test]
fn raw_strcomp_works_in_both_hosts_and_keeps_failures_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StrComp(ChrW(55296),ChrW(57343),0) AS A,StrComp(ChrW(55296),ChrW(55296),0) AS B,StrComp(ChrB(65),NULL,0) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tB\tN\n-1\t0\tNULL\n");

        for expression in [
            "StrComp(ChrB(65),ChrB(65),0)",
            "StrComp(ChrB(65),NULL,3)",
            "StrComp(ChrW(55296),'x',1)",
            "StrComp(ChrW(55296),NULL,1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
