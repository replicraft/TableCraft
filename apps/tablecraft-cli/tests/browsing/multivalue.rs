use super::{Command, FileCopy, calculated_fixture, complex_fixture, success, support};

#[path = "../../../../crates/engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;

#[test]
fn buffered_multivalue_browsing_keeps_null_empty_and_paging_distinctions() {
    let mut bytes = links_fixture::populated();
    const PAGE_BYTES: usize = 4096;
    const PARENT_DATA_PAGE: usize = 12;
    const THIRD_SLOT: usize = 18;
    const NULL_MASK_OFFSET: usize = 10;
    let row = u16::from_le_bytes(
        bytes[PARENT_DATA_PAGE * PAGE_BYTES + THIRD_SLOT
            ..PARENT_DATA_PAGE * PAGE_BYTES + THIRD_SLOT + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[PARENT_DATA_PAGE * PAGE_BYTES + row + NULL_MASK_OFFSET] = 1;
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run("rows", &["Items"])),
        b"Id\tScore [multivalue]\n1\t[100, 101]\n2\t[200]\n3\tNULL\n"
    );
    let ordinary = file.run("rows", &["Items", "--stream"]);
    assert_eq!(ordinary.status.code(), Some(2));
    assert!(ordinary.stdout.is_empty());
    assert_eq!(
        success(file.run("rows", &["Items", "--multivalues", "--format", "tsv"])),
        b"Id\tScore [multivalue]\n1\t[100, 101]\n2\t[200]\n3\tNULL\n"
    );
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--multivalues", "--offset", "1", "--limit", "1"]
        )),
        b"Id\tScore [multivalue]\n2\t[200]\n"
    );
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--multivalues", "--offset", "4294967295"]
        )),
        b"Id\tScore [multivalue]\n"
    );
    let pretty = String::from_utf8(success(file.run(
        "rows",
        &[
            "Items",
            "--multivalues",
            "--format",
            "table",
            "--limit",
            "1",
        ],
    )))
    .unwrap();
    assert!(pretty.starts_with('┌'));
    assert!(pretty.contains("[100, 101]"));
    // Cached-only tables can use the mixed profile only with the separate cache opt-in.
    let cache = FileCopy::new(calculated_fixture::database(&[Some(41)]));
    let result = cache.run("rows", &["Items", "--multivalues"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        success(cache.run("rows", &["Items", "--multivalues", "--stored-calculations"])),
        b"Id\tScore [cached]\n1\t41\n"
    );
}

#[test]
fn all_collection_mode_retains_multivalue_contents_and_rejects_conflicting_opt_ins() {
    let file = FileCopy::new(links_fixture::populated());
    let restricted = success(file.run("rows", &["Items", "--multivalues", "--format", "tsv"]));
    let general = success(file.run("rows", &["Items", "--collections", "--format", "tsv"]));
    assert_eq!(general, restricted);
    let result = file.run("rows", &["Items", "--collections", "--multivalues"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn explicit_collection_read_pools_preserve_rendering_and_refuse_invalid_scope_before_io() {
    let file = FileCopy::new(links_fixture::populated());
    for mode in [None, Some("--multivalues"), Some("--collections")] {
        let mut arguments = vec!["Items"];
        if let Some(mode) = mode {
            arguments.push(mode);
        }
        let expected = success(file.run("rows", &arguments));
        let mut bounded = arguments.clone();
        bounded.extend([
            "--native-bytes",
            "67108864",
            "--native-rows",
            "100000",
            "--native-steps",
            "10000000",
        ]);
        assert_eq!(success(file.run("rows", &bounded)), expected);
        for flag in ["--native-bytes", "--native-rows", "--native-steps"] {
            let mut limited = arguments.clone();
            limited.extend([flag, "1"]);
            let result = file.run("rows", &limited);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
    for mode in ["--multivalues", "--collections"] {
        for (flag, value) in [
            ("--native-rows", "0"),
            ("--native-rows", "1000001"),
            ("--native-bytes", "268435457"),
            ("--native-steps", "100000001"),
        ] {
            let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["rows", "missing.accdb", "Items", mode, flag, value])
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("error:"));
            assert!(!String::from_utf8_lossy(&result.stderr).contains("I/O failed"));
        }
    }
}
