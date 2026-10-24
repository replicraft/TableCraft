mod support;

use support::run;

#[test]
fn unicode_intermediates_remain_strings_inside_calls_and_strict_at_stdout() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Len(ChrW(55296)) AS L,AscW(ChrW(55296)) AS W,LenB(ChrW(55296)) AS B,VarType(ChrW(55296)) AS T,AscW(RightB('😀',2)) AS Tail",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"L\tW\tB\tT\tTail\n1\t-10240\t2\t8\t-8704\n");

        for expression in [
            "ChrW(55296)",
            "Len(ChrB(65))",
            "AscW(ChrB(65))",
            "Len(ChrW(1/0))",
            "AscW(ChrW(CVErr(1)))",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
