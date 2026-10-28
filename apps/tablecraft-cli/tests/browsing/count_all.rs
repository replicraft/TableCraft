use super::*;

const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 2;
const CATALOG_ROWS_PAGE: usize = 4;
const OTHER_TABLE: usize = 14;
const OTHER_USAGE: usize = 15;
const ROW_COUNT_OFFSET: usize = 16;

fn two_tables(linked: bool) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize((OTHER_USAGE + 1) * PAGE_BYTES, 0);
    support::put32(&mut bytes, CATALOG_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 4);
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        support::record(
            OTHER_TABLE as i32,
            0,
            if linked { 6 } else { 1 },
            0,
            b"\xff\xfeOther",
        ),
    ];
    bytes[CATALOG_ROWS_PAGE * PAGE_BYTES..(CATALOG_ROWS_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &support::data_page(
            CATALOG_PAGE as u32,
            &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
        ),
    );
    bytes[OTHER_TABLE * PAGE_BYTES..(OTHER_TABLE + 1) * PAGE_BYTES].copy_from_slice(
        &fixture::definition(&[("Name", 10, 0, 40)], 0, (OTHER_USAGE as u32) << 8),
    );
    bytes[OTHER_USAGE * PAGE_BYTES..(OTHER_USAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0; 7])]));
    bytes
}
#[test]
fn counts_follow_kind_and_linked_counts_are_unavailable_without_opening_sources() {
    let file = FileCopy::new(two_tables(true));
    assert_eq!(
        success(file.run("tables", &["--count"])),
        b"Name\tKind\tRow Count\nItems\tUser\t3\nOther\tUser\tN/A\n"
    );
    let text = String::from_utf8(success(file.run("tables", &["--count", "--details"]))).unwrap();
    assert!(text.starts_with("Name\tKind\tRow Count\tStorage\tRows\n"));
    assert!(text.contains("Items\tUser\t3\tLocal\tNot inspected"));
    let system =
        String::from_utf8(success(file.run("tables", &["--count", "--show-system"]))).unwrap();
    assert!(system.contains("MSysObjects\tSystem\t4\n"));
    let bordered = String::from_utf8(success(
        file.run("tables", &["--count", "--format", "table"]),
    ))
    .unwrap();
    assert!(bordered.contains("Row Count") && bordered.contains('┌'));
}
#[test]
fn all_tables_have_separate_named_sections_empty_headers_and_per_table_paging() {
    let file = FileCopy::new(two_tables(false));
    assert_eq!(
        success(file.run("tables", &["--count"])),
        b"Name\tKind\tRow Count\nItems\tUser\t3\nOther\tUser\t0\n"
    );
    assert_eq!(
        success(file.run("rows", &["--all", "--offset", "1", "--limit", "1"])),
        b"Table\tItems\nId\tScore\n2\t20\n\nTable\tOther\nName\n"
    );
    let bordered = String::from_utf8(success(
        file.run("rows", &["--all", "--limit", "1", "--format", "table"]),
    ))
    .unwrap();
    assert!(bordered.contains("Items") && bordered.contains("Other") && bordered.contains("Score"));
    assert_eq!(bordered.matches('┌').count(), 4);
    for args in [&[][..], &["Items", "--all"][..]] {
        let result = file.run("rows", args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
#[test]
fn linked_sections_are_explicit_and_late_local_failures_publish_no_partial_output() {
    let linked = FileCopy::new(two_tables(true));
    let text = String::from_utf8(success(linked.run("rows", &["--all"]))).unwrap();
    assert!(text.contains("Table\tOther\nStatus\nExternal or unknown source not opened"));
    let mut bytes = two_tables(false);
    const FIRST_COLUMN: usize = 63;
    bytes[OTHER_TABLE * PAGE_BYTES + FIRST_COLUMN] = u8::MAX;
    let broken = FileCopy::new(bytes);
    let result = broken.run("rows", &["--all"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    // Metadata counts do not require readable scalar carriers or payloads.
    assert!(success(broken.run("tables", &["--count"])).ends_with(b"Other\tUser\t0\n"));
}

#[test]
fn omitted_table_selects_one_user_table_and_preserves_streaming_and_budgets() {
    let file = FileCopy::new(fixture::database());
    let expected = success(file.run("rows", &["Items"]));
    assert_eq!(success(file.run("rows", &[])), expected);
    assert_eq!(success(file.run("rows", &["--stream"])), expected);
    assert_eq!(
        success(file.run(
            "rows",
            &["--native-rows", "100000", "--native-steps", "10000000"]
        )),
        expected
    );
}

#[test]
fn omitted_table_counts_all_user_tables_including_links_and_hidden_tables() {
    for linked in [false, true] {
        let file = FileCopy::new(two_tables(linked));
        for arguments in [&[][..], &["--stream"][..]] {
            let result = file.run("rows", arguments);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr)
                    .contains("more than one user table found; specify TABLE or use --all")
            );
        }
    }

    let mut bytes = two_tables(false);
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        support::record(14, 0, 1, 1, b"\xff\xfeHiddenOther"),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    let file = FileCopy::new(bytes);
    let result = file.run("rows", &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("more than one user table"));
}

#[test]
fn omitted_table_with_only_system_tables_has_clear_error_without_output() {
    let mut bytes = fixture::database();
    support::put32(&mut bytes, 2 * PAGE_BYTES + 16, 2);
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    let file = FileCopy::new(bytes);
    for arguments in [&[][..], &["--stream"][..]] {
        let result = file.run("rows", arguments);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("no user tables found; specify TABLE explicitly or use --all")
        );
    }
}
