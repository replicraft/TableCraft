mod support;
use support::run;
#[test]
fn index_inspection_is_explicit_bounded_and_inert() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let path = path.to_str().unwrap();
    let result = run(&["inspect", path, "--table", "ROULETTE_TB", "--indexes"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout,b"Table\tROULETTE_TB\nIndex\tLogical\tPhysical\tRoot page\tType\tFlags\tColumn\tDescending\nCOLOR_ID\t0\t0\t729\t0\t0x80\tCOLOR_ID\tFALSE\nNumber\t1\t1\t730\t0\t0x81\tNBR_VALUE\tFALSE\nORDR_NBR\t2\t2\t731\t0\t0x81\tORDR_NBR\tFALSE\n");
    assert!(result.stderr.is_empty());
    assert_eq!(run(&["inspect", path, "--indexes"]).status.code(), Some(2));
}

#[test]
fn indexed_queries_are_explicit_and_retain_query_semantics() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let path = path.to_str().unwrap();
    let sql = "SELECT TOP 3 ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR > 4 ORDER BY ORDR_NBR DESC";
    let normal = run(&["query", "run", "--database", path, sql]);
    let indexed = run(&[
        "-v",
        "query",
        "run",
        "--database",
        path,
        "--index",
        "ROULETTE_TB",
        "ORDR_NBR",
        sql,
    ]);
    assert!(
        indexed.status.success(),
        "{}",
        String::from_utf8_lossy(&indexed.stderr)
    );
    assert_eq!(normal.stdout, indexed.stdout);
    assert!(String::from_utf8_lossy(&indexed.stderr).contains("indexed table completed"));
    let rejected = run(&[
        "query",
        "run",
        "--database",
        path,
        "--index",
        "ROULETTE_TB",
        "Number",
        sql,
    ]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    assert_eq!(
        run(&[
            "query",
            "run",
            "--index",
            "ROULETTE_TB",
            "ORDR_NBR",
            "SELECT 1"
        ])
        .status
        .code(),
        Some(2)
    );
}
