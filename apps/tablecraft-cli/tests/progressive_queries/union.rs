use super::*;

#[test]
fn union_all_preserves_written_arm_order_across_profiles_and_output_batches() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = profile;
        if profile != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        for (sql, expected) in [
            (
                "SELECT Id AS N FROM Items UNION ALL SELECT Id+10 FROM Items",
                b"N\n1\n2\n3\n11\n12\n13\n".as_slice(),
            ),
            (
                "SELECT TOP 1 Id AS N FROM Items UNION ALL SELECT TOP 2 Id+10 FROM Items",
                b"N\n1\n11\n12\n".as_slice(),
            ),
            (
                "SELECT Id AS N FROM Items WHERE False UNION ALL SELECT Id FROM Items WHERE False",
                b"N\n".as_slice(),
            ),
        ] {
            assert_eq!(success(file.query(sql, &[])), expected);
            for batch in ["1", "2", "4", "1000"] {
                assert_eq!(
                    success(file.query(sql, &["--stream", "--batch-rows", batch])),
                    expected
                );
            }
        }
    }
}

#[test]
fn late_union_arm_failure_preserves_only_complete_output_batches() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Id AS N FROM Items UNION ALL SELECT 10\\(2-Id) FROM Items";
    for batch in ["1", "2", "4"] {
        assert_eq!(
            failure(file.query(sql, &["--stream", "--batch-rows", batch])),
            b"N\n1\n2\n3\n10\n"
        );
    }
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "8"])).is_empty());
    assert!(failure(file.query(sql, &[])).is_empty());
}

#[test]
fn union_text_conversion_uses_the_existing_explicit_number_policy() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT 'kept' AS N FROM Items WHERE Id=1 UNION ALL SELECT Id FROM Items";
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"N\n\"\"\"kept\"\"\"\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());
    let options = ["--number-format", "invariant"];
    let expected = success(file.query(sql, &options));
    assert_eq!(
        expected,
        b"N\n\"\"\"kept\"\"\"\n\"\"\"1\"\"\"\n\"\"\"2\"\"\"\n\"\"\"3\"\"\"\n"
    );
    assert_eq!(
        success(file.query(
            sql,
            &[
                "--stream",
                "--batch-rows",
                "2",
                "--number-format",
                "invariant"
            ]
        )),
        expected
    );
}
