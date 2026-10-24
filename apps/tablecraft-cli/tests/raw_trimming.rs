mod support;

use support::run;

#[test]
fn trimming_retains_raw_results_for_observation_and_keeps_output_buffered() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT LenB(LTrim(' '&ChrW(55296)&' ')) AS L,LenB(RTrim(' '&ChrW(55296)&' ')) AS R,LenB(Trim(' '&ChrW(55296)&' ')) AS B,Trim(NULL) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"L\tR\tB\tN\n4\t4\t2\tNULL\n");
        for sql in [
            "SELECT Trim(ChrW(55296))",
            "SELECT LenB(Trim(ChrB(32)))",
            "SELECT LenB(Trim(ChrW(55296))),1/0",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
