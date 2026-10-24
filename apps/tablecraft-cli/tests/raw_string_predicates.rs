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
