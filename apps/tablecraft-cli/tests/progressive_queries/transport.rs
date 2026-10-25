use super::*;

#[test]
fn priority_profile_streams_match_buffered_queries_across_batch_boundaries() {
    let sql = "SELECT Id,Score+Id AS Total FROM Items WHERE Id>=2";
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = profile;
        if profile != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        let expected = success(file.query(sql, &[]));
        assert_eq!(expected, b"Id\tTotal\n2\t22\n3\t33\n");
        for batch in ["1", "2", "3", "1000"] {
            assert_eq!(
                success(file.query(sql, &["--stream", "--batch-rows", batch])),
                expected
            );
        }
        assert_eq!(success(file.query(sql, &["--stream"])), expected);
        assert_eq!(
            success(file.query(sql, &["--stream", "--format", "auto"])),
            expected
        );
    }
}

#[test]
fn original_ace2007_text_and_csv_escaping_match_buffered_output() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let file = FileCopy::new(fs::read(path).unwrap());
    let sql = "SELECT * FROM ROULETTE_TB";
    let expected = success(file.query(sql, &[]));
    for batch in ["1", "7", "38"] {
        let actual = success(file.query(sql, &["--stream", "--batch-rows", batch]));
        assert_eq!(actual, expected);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(actual.as_slice());
        assert!(!reader.headers().unwrap().is_empty());
        assert!(reader.records().all(|record| record.is_ok()));
    }
}

#[test]
fn filtered_and_physically_empty_sources_emit_one_header_after_checked_eof() {
    let file = FileCopy::new(fixture::database());
    for batch in ["1", "2", "1000"] {
        assert_eq!(
            success(file.query(
                "SELECT Id,Score AS N FROM Items WHERE False",
                &["--stream", "--batch-rows", batch]
            )),
            b"Id\tN\n"
        );
    }

    let mut bytes = fixture::database();
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&0_u32.to_le_bytes());
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &[]));
    let empty = FileCopy::new(bytes);
    assert_eq!(
        success(empty.query("SELECT Id FROM Items", &["--stream", "--batch-rows", "1"])),
        b"Id\n"
    );
}

#[test]
fn scalar_projection_of_mixed_native_schema_never_decodes_an_unselected_field() {
    let mut bytes = fixture::database();
    bytes[10 * 4096 + 63 + 25] = 0xff;
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.query("SELECT Id FROM Items", &["--stream", "--batch-rows", "1"])),
        b"Id\n1\n2\n3\n"
    );
    for sql in ["SELECT * FROM Items", "SELECT Score FROM Items"] {
        assert!(failure(file.query(sql, &["--stream"])).is_empty());
    }
}

#[test]
fn cumulative_encoded_output_limit_never_refills_between_batches() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Id AS N FROM Items";
    assert_eq!(
        failure(file.query(
            sql,
            &["--stream", "--batch-rows", "1", "--output-bytes", "4"]
        )),
        b"N\n1\n"
    );
    assert!(
        failure(file.query(
            sql,
            &["--stream", "--batch-rows", "2", "--output-bytes", "4"]
        ))
        .is_empty()
    );
    assert_eq!(
        success(file.query(
            sql,
            &["--stream", "--batch-rows", "1", "--output-bytes", "8"]
        )),
        b"N\n1\n2\n3\n"
    );
}
