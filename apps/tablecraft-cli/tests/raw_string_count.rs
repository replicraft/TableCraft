mod support;

use support::run;

#[test]
fn count_consumes_raw_strings_and_buffers_failures() {
    let result = run(&[
        "query",
        "run",
        "SELECT Count(ChrW(N)) AS W,Count(ChrB(65)) AS B,Count(CStr(LeftB(ChrW(N),1))) AS C,Count(ChrW(N)+NULL) AS Z FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"W\tB\tC\tZ\n2\t2\t2\t0\n");

    for expression in [
        "ChrW(65536)",
        "ChrW(NULL)",
        "CStr(NULL)",
        "ChrW(55296)+(1/0)",
    ] {
        let result = run(&["query", "run", &format!("SELECT 1,Count({expression})")]);
        assert_eq!(result.status.code(), Some(2), "{expression}");
        assert!(result.stdout.is_empty(), "{expression}");
    }
}
