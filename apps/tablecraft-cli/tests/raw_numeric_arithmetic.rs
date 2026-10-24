mod support;

use support::run;

#[test]
fn raw_numeric_nulls_and_eager_failures_use_the_same_profile_in_both_hosts() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT ChrW(55296)-NULL AS S,NULL*ChrB(65) AS M,ChrW(55296)/NULL AS D,ChrB(65)\\NULL AS Q,ChrW(55296) Mod NULL AS R,NULL^ChrW(55296) AS P",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"S\tM\tD\tQ\tR\tP\nNULL\tNULL\tNULL\tNULL\tNULL\tNULL\n"
        );
        for expression in [
            "(ChrW(55296)-NULL)*(1/0)",
            "ChrW(65536)-NULL",
            "ChrW(55296)*1",
            "LeftB('12',4)/2",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
