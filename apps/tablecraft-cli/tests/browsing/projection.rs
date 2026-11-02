//! Actual buffered scalar projection leaves default complete browsing intact.
use super::*;

#[test]
fn buffered_selected_columns_keep_native_order_paging_and_stream_compatibility() {
    for code in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        assert_eq!(
            success(file.run(
                "rows",
                &[
                    "Items", "--column", "Score", "--column", "ID", "--offset", "1", "--limit", "1"
                ]
            )),
            b"Id\tScore\n2\t20\n"
        );
        assert_eq!(
            success(file.run("rows", &["Items", "--columns", "Score", "--offset", "3"])),
            b"Score\n"
        );
        assert_eq!(
            success(file.run("rows", &["Items", "--column", "Id"])),
            b"Id\n1\n2\n3\n"
        );
        assert_eq!(
            success(file.run(
                "rows",
                &["Items", "--column", "Id", "--stream", "--limit", "1"]
            )),
            b"Id\n1\n2\n3\n"
        );
        assert_eq!(
            success(file.run(
                "rows",
                &["--all", "--column", "Id", "--offset", "1", "--limit", "1"]
            )),
            b"Table\tItems\nId\n2\n"
        );
        let pretty = String::from_utf8(success(file.run(
            "rows",
            &[
                "Items", "--column", "Id", "--format", "table", "--limit", "1",
            ],
        )))
        .unwrap();
        assert!(pretty.starts_with('┌'));
        assert!(pretty.contains("Id"));
        assert!(!pretty.contains("Score"));
        assert_eq!(
            success(file.run("rows", &["Items"])),
            b"Id\tScore\n1\t10\n2\t20\n3\t30\n"
        );
    }
}

#[test]
fn explicit_mixed_projection_does_not_decode_unselected_collection_payloads() {
    let mut bytes = attachment_fixture::database();
    // Keep declarations valid but corrupt the flat child page. The explicitly
    // unselected collection stays unchecked, while default browsing must fail.
    bytes[attachment_fixture::CHILD_PAGE * 4096] = 0;
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--column", "Id", "--offset", "1", "--limit", "1"]
        )),
        b"Id\n2\n"
    );
    let default = file.run("rows", &["Items", "--limit", "1"]);
    assert_eq!(default.status.code(), Some(2));
    assert!(default.stdout.is_empty());
    let selected = file.run("rows", &["Items", "--column", "Score"]);
    assert_eq!(selected.status.code(), Some(2));
    assert!(selected.stdout.is_empty());

    let mut bytes = fixture::database();
    bytes[10 * 4096 + 63 + 25] = 0xff;
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run("rows", &["Items", "--column", "Id"])),
        b"Id\n1\n2\n3\n"
    );
    for names in [["Id", "ID"], ["Id", "missing"], ["Id", "Score"]] {
        let result = file.run(
            "rows",
            &["Items", "--column", names[0], "--column", names[1]],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn selected_page_failures_are_buffered_and_original_native_pools_still_apply() {
    let mut bytes = fixture::database();
    let start = usize::from(
        u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) & 0x1fff,
    );
    support::put16(&mut bytes, 12 * 4096 + start, u16::MAX);
    let file = FileCopy::new(bytes);
    let bad = file.run("rows", &["Items", "--column", "Id", "--limit", "1"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--column", "Id", "--offset", "1", "--limit", "1"]
        )),
        b"Id\n2\n"
    );

    let file = FileCopy::new(fixture::database());
    for pool in ["--native-bytes", "--native-rows", "--native-steps"] {
        let result = file.run("rows", &["Items", "--column", "Id", pool, "1"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let mut bytes = fixture::database();
    support::put32(&mut bytes, 10 * 4096 + 16, 4);
    let file = FileCopy::new(bytes);
    let result = file.run("rows", &["Items", "--column", "Id", "--limit", "4"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn projection_mode_conflicts_refuse_before_open_and_templates_do_not_ignore_selection() {
    for mode in ["--collections", "--multivalues", "--stored-calculations"] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "rows",
                "missing-projection-input.accdb",
                "Items",
                "--column",
                "Id",
                mode,
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("Usage:"));
    }
    let file = FileCopy::new(tablecraft_testkit::fixtures::formats::template::empty_template());
    assert_eq!(success(file.run("tables", &[])), b"Name\tKind\n");
    assert_eq!(success(file.run("rows", &["--all"])), b"");
    // With no table objects the old ignored-selection path would succeed, so
    // this pair directly exercises the template projection refusal.
    let result = file.run("rows", &["--all", "--column", "Id"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("template package"));
    // Normal inert OPC capture still happens; this tests refusal before table
    // interpretation, not a claim of zero archive payload I/O.
}

#[test]
fn cached_selection_is_explicit_and_name_envelope_is_bounded_before_open() {
    let file = FileCopy::new(calculated_fixture::database_with_expression(
        &[Some(77)],
        "1/0",
    ));
    assert_eq!(
        success(file.run("rows", &["Items", "--column", "Id"])),
        b"Id\n1\n"
    );
    assert_eq!(
        success(file.run("rows", &["Items"])),
        b"Id\tScore [cached]\n1\t77\n"
    );
    let selected = file.run("rows", &["Items", "--column", "Score"]);
    assert_eq!(selected.status.code(), Some(2));
    assert!(selected.stdout.is_empty());

    for stream in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command.args(["rows", "missing-projection-cap-input.accdb", "Items"]);
        if stream {
            command.arg("--stream");
        }
        for _ in 0..257 {
            command.args(["--column", "Id"]);
        }
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        let error = String::from_utf8_lossy(&result.stderr);
        let expected = if stream {
            "error: too many selected streaming columns"
        } else {
            "error: database inspection structural limit exceeded"
        };
        assert_eq!(error.trim(), expected);
    }
}
