use super::*;

#[test]
fn scalar_streams_match_buffered_output_with_one_header_in_every_priority_profile() {
    for code in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        let expected = success(file.run("rows", &["Items"]));
        for batch in ["1", "2", "3", "100"] {
            assert_eq!(
                success(file.run("rows", &["Items", "--stream", "--limit", batch])),
                expected
            );
        }
    }
}

#[test]
fn stored_text_escaping_and_csv_transport_are_identical_across_stream_batches() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let file = FileCopy::new(fs::read(path).unwrap());
    let buffered = success(file.run("rows", &["ROULETTE_TB"]));
    for batch in ["1", "7", "38"] {
        assert_eq!(
            success(file.run("rows", &["ROULETTE_TB", "--stream", "--limit", batch])),
            buffered
        );
    }
}

#[test]
fn stream_projections_support_mixed_schemas_without_null_placeholders() {
    let mut bytes = fixture::database();
    bytes[10 * 4096 + 63 + 25] = 0xff;
    let file = FileCopy::new(bytes);
    let result = file.run("rows", &["Items", "--stream"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--stream", "--column", "Id", "--limit", "1"]
        )),
        b"Id\n1\n2\n3\n"
    );
    for columns in [["Id", "ID"], ["Id", "missing"], ["Id", "Score"]] {
        let result = file.run(
            "rows",
            &[
                "Items", "--stream", "--column", columns[0], "--column", columns[1],
            ],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn stream_late_read_failures_keep_only_previously_completed_batches() {
    let mut bytes = fixture::database();
    let slot = 12 * 4096 + 16;
    let row = u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) as usize & 0x1fff;
    bytes[12 * 4096 + row..12 * 4096 + row + 2].copy_from_slice(&100_u16.to_le_bytes());
    let file = FileCopy::new(bytes);
    let result = file.run("rows", &["Items", "--stream", "--limit", "1"]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(result.stdout, b"Id\tScore\n1\t10\n");
    let result = file.run("rows", &["Items", "--stream", "--limit", "2"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let result = file.run("rows", &["Items"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn stream_ranges_and_conflicting_modes_fail_before_opening_sources() {
    for args in [
        vec!["--stream", "--all"],
        vec!["--stream", "--offset", "0"],
        vec!["--stream", "--collections"],
        vec!["--stream", "--multivalues"],
        vec!["--stream", "--stored-calculations"],
        vec!["--column", "Id"],
        vec!["--native-rows", "10"],
        vec!["--native-steps", "10"],
        vec!["--native-bytes", "10"],
        vec!["--stream", "--native-rows", "0"],
        vec!["--stream", "--native-rows", "1000001"],
        vec!["--stream", "--native-steps", "100000001"],
        vec!["--stream", "--native-bytes", "268435457"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["rows", "missing-stream-input.accdb", "Items"])
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
        assert!(
            (String::from_utf8_lossy(&result.stderr).contains("Usage:")
                || String::from_utf8_lossy(&result.stderr).contains("invalid value")),
            "{args:?}"
        );
    }
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "--format",
            "table",
            "rows",
            "missing-stream-input.accdb",
            "Items",
            "--stream",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("streaming rows require TSV"));
}

#[test]
fn streaming_enforces_native_quotas_without_refilling_between_batches() {
    let file = FileCopy::new(fixture::database());
    for option in ["--native-bytes", "--native-rows", "--native-steps"] {
        let result = file.run("rows", &["Items", "--stream", option, "1"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("resource limit exceeded"));
    }
    let result = file.run(
        "rows",
        &["Items", "--stream", "--limit", "1", "--native-rows", "4"],
    );
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(result.stdout, b"Id\tScore\n1\t10\n");
}

#[test]
fn empty_streams_publish_one_header_after_count_validation() {
    let mut bytes = fixture::database();
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&0_u32.to_le_bytes());
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &[]));
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run("rows", &["Items", "--stream", "--limit", "1"])),
        b"Id\tScore\n"
    );
}

#[test]
fn streaming_reports_late_declared_count_mismatches_after_previous_rows() {
    let mut bytes = fixture::database();
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&4_u32.to_le_bytes());
    let file = FileCopy::new(bytes);
    let result = file.run("rows", &["Items", "--stream", "--limit", "1"]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(result.stdout, b"Id\tScore\n1\t10\n2\t20\n3\t30\n");
    assert!(!result.stderr.is_empty());
}
