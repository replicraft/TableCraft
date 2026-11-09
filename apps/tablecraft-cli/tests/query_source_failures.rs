use tablecraft_testkit::{fixtures::access::mutation_fixture, support::cli::Runner};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
fn malformed_user_records_keep_their_cause_without_partial_buffered_output_or_private_context() {
    let original = mutation_fixture::with_invalid_row_page();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private-source.mdb");
    std::fs::write(&path, &original).unwrap();
    let sql = "SELECT Id, Score FROM Items";
    let output = CLI.run_os([
        "query".as_ref(),
        "run".as_ref(),
        "--database".as_ref(),
        path.as_os_str(),
        sql.as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("query source failed: invalid data"),
        "{error}"
    );
    assert!(!error.contains(sql));
    assert!(!error.contains("private-source.mdb"));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn native_query_row_allowance_reports_source_origin_and_never_publishes_a_prefix() {
    let records = (1..=205).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let original = mutation_fixture::with_rows(&records);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bounded-source.mdb");
    std::fs::write(&path, &original).unwrap();
    let output = CLI.run_os([
        "query".as_ref(),
        "run".as_ref(),
        "--database".as_ref(),
        path.as_os_str(),
        "--native-rows".as_ref(),
        "20".as_ref(),
        "SELECT Id FROM Items".as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("query source failed: Rows resource limit exceeded"),
        "{error}"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}
