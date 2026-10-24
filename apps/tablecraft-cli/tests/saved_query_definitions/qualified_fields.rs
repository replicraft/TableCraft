use super::*;

#[test]
fn registered_object_list_reconstructs_without_enabling_its_vba_functions() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(&path).unwrap();
    let database = path.to_str().unwrap();
    let saved = cli_support::run(&[
        "query",
        "saved",
        "--database",
        database,
        "101_Object_List_qry",
    ]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let sql = String::from_utf8(saved.stdout).unwrap();
    assert!(sql.contains("[msysobjects].[Connect]"), "{sql}");
    assert!(sql.contains("GetObjectTypeName([Type])"), "{sql}");
    assert!(sql.contains("GetObjectDescription([Name])"), "{sql}");

    let result = cli_support::run(&[
        "query",
        "run",
        "--database",
        database,
        "--saved",
        "101_Object_List_qry",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("query feature is not implemented"));
    assert_eq!(std::fs::read(path).unwrap(), original);
}
