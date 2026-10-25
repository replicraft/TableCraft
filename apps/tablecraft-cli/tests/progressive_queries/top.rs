use super::*;

#[test]
fn unordered_top_matches_buffered_prefixes_across_profiles_and_batches() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = profile;
        if profile != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        for count in [1, 2, 10] {
            let sql = format!("SELECT TOP {count} Id,Score FROM Items WHERE Id>=2");
            let expected = success(file.query(&sql, &[]));
            for batch in ["1", "2", "1000"] {
                assert_eq!(
                    success(file.query(&sql, &["--stream", "--batch-rows", batch])),
                    expected
                );
            }
        }
    }
}

#[test]
fn top_count_does_not_stop_native_eof_checks_or_eager_query_computation() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT TOP 1 Id,10\\(2-Id) AS N FROM Items";
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"Id\tN\n1\t10\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());

    let mut bytes = fixture::database();
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&4_u32.to_le_bytes());
    let mismatch = FileCopy::new(bytes);
    assert_eq!(
        failure(mismatch.query(
            "SELECT TOP 1 Id FROM Items",
            &["--stream", "--batch-rows", "1"]
        )),
        b"Id\n1\n"
    );
    assert!(
        failure(mismatch.query(
            "SELECT TOP 1 Id FROM Items",
            &["--stream", "--batch-rows", "2"]
        ))
        .is_empty()
    );
}

#[test]
fn top_discards_later_raw_strings_before_final_publication_like_buffered_execution() {
    let file = FileCopy::new(fixture::database());
    for raw in ["ChrW(55296)", "ChrB(65)"] {
        let sql = format!("SELECT TOP 1 IIf(Id=1,'kept',{raw}) AS S FROM Items");
        let expected = success(file.query(&sql, &[]));
        for batch in ["1", "2", "1000"] {
            assert_eq!(
                success(file.query(&sql, &["--stream", "--batch-rows", batch])),
                expected
            );
        }
    }
}

#[test]
fn top_keeps_query_allowances_cumulative_after_its_last_published_row() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT TOP 1 Id AS N FROM Items";
    assert_eq!(
        success(file.query(
            sql,
            &["--stream", "--batch-rows", "1", "--output-bytes", "4"]
        )),
        b"N\n1\n"
    );
    assert_eq!(
        failure(file.query(
            sql,
            &["--stream", "--batch-rows", "1", "--execution-rows", "4"]
        )),
        b"N\n1\n"
    );
    for sql in [
        "SELECT TOP 0 Id FROM Items",
        "SELECT TOP 0 PERCENT Id FROM Items",
    ] {
        assert!(failure(file.query(sql, &["--stream"])).is_empty());
    }
}
