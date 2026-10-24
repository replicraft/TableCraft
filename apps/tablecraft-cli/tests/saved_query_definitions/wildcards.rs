use super::*;

#[test]
fn registered_saved_join_with_qualified_wildcard_matches_explicit_query() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(&path).unwrap();
    let database = path.to_str().unwrap();
    let saved = cli_support::run(&[
        "query",
        "saved",
        "--database",
        database,
        "950_Leszynski_Conventions_qry",
    ]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert!(
        saved
            .stdout
            .starts_with(b"SELECT [N].*, [G].[TAG_GRP_NME] FROM ")
    );

    let result = cli_support::run(&[
        "query",
        "run",
        "--database",
        database,
        "--saved",
        "950_Leszynski_Conventions_qry",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let explicit = cli_support::run(&[
        "query",
        "run",
        "SELECT N.*,G.TAG_GRP_NME FROM TAG_NME_TB AS N INNER JOIN TAG_GRP_TB AS G ON N.TAG_GRP_ID=G.TAG_GRP_ID",
        "--database",
        database,
    ]);
    assert!(
        explicit.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    assert_eq!(result.stdout, explicit.stdout);
    assert!(!result.stdout.is_empty());
    let limited = cli_support::run(&[
        "query",
        "run",
        "--database",
        database,
        "--saved",
        "950_Leszynski_Conventions_qry",
        "--output-bytes",
        "1",
    ]);
    assert_eq!(limited.status.code(), Some(2));
    assert!(limited.stdout.is_empty());
    assert!(String::from_utf8_lossy(&limited.stderr).contains("output limit"));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
