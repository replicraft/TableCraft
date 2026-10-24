mod support;

use support::run;

#[test]
fn string_constructs_observable_units_without_weakening_final_publication() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT LenB(String(3,ChrW(55296))) AS B,AscW(String(2,'😀')) AS U,LenB(String(0,'😀')) AS Z,String(ChrB(65),NULL) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"B\tU\tZ\tN\n6\t-10179\t0\tNULL\n");
        for sql in [
            "SELECT String(1,ChrW(55296))",
            "SELECT LenB(String(0,ChrB(65)))",
            "SELECT LenB(String(2,ChrW(55296))),1/0",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
