mod support;

use support::run;

#[test]
fn nz_default_text_participates_in_union_conversion_and_keeps_atomic_policy_failures() {
    let sql =
        "SELECT TypeName(D.V) AS Kind,D.V FROM (SELECT Nz(NULL) AS V UNION ALL SELECT 7) AS D";
    let result = run(&["query", "run", sql, "--number-format", "invariant"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let actual = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(result.stdout.as_slice())
        .records()
        .map(|row| row.unwrap().iter().map(str::to_owned).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            vec!["Kind", "V"],
            vec!["\"String\"", "\"\""],
            vec!["\"String\"", "\"7\""],
        ]
    );
    for sql in [
        sql,
        "SELECT 1,Nz(NULL)+1",
        "SELECT 1,CInt(NULL)",
        "SELECT 1,Nz(7,CInt(NULL))",
    ] {
        let result = run(&["query", "run", sql]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
