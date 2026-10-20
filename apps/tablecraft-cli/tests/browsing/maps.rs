use super::*;
#[test]
fn row_browsing_accepts_large_owned_maps_without_expanding_query_snapshots() {
    let mut bytes = map_fixture::database(6000, true, false);
    bytes[20] = 6;
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run("tables", &["--count"])),
        b"Name\tKind\tRow Count\nItems\tUser\t6000\n"
    );
    assert_eq!(
        success(file.run("rows", &["Items", "--offset", "1", "--limit", "2"])),
        b"Id\tScore\n2\t20\n3\t30\n"
    );
    let query = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "run", "SELECT TOP 1 Id FROM Items", "--database"])
        .arg(&file.path)
        .output()
        .unwrap();
    assert_eq!(query.status.code(), Some(2));
    assert!(query.stdout.is_empty());
    assert!(String::from_utf8_lossy(&query.stderr).contains("query structural limit exceeded"));
}
