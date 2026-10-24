mod support;

use support::run;

#[test]
fn query_rows_retain_raw_strings_until_explicit_consumption_or_final_output() {
    let result = run(&[
        "query",
        "run",
        "SELECT LenB(S) AS B,VarType(S) AS T,IsNull(S) AS N FROM (SELECT ChrB(65) AS S) AS Q",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"B\tT\tN\n1\t8\tFALSE\n");

    let result = run(&[
        "query",
        "run",
        "SELECT 1,S FROM (SELECT ChrW(55296) AS S) AS Q",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
