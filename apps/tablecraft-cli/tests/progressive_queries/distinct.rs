use super::*;
use std::ops::Range;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;
const PROFILE_OFFSET: usize = 20;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

#[test]
fn distinct_fixed_keys_keep_first_encounter_and_top_after_deduplication() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[PROFILE_OFFSET] = profile;
        if profile != 1 {
            bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);

        for (sql, expected) in [
            ("SELECT DISTINCT Id Mod 2 AS K FROM Items", "K\n1\n0\n"),
            (
                "SELECT DISTINCT CCur(Id Mod 2) AS K FROM Items",
                "K\n1.0000\n0.0000\n",
            ),
            (
                "SELECT DISTINCT CDec(Id Mod 2) AS K FROM Items",
                "K\n1\n0\n",
            ),
            ("SELECT DISTINCT TOP 1 Id Mod 2 AS K FROM Items", "K\n1\n"),
        ] {
            for declared in [false, true] {
                let options = if declared {
                    vec!["--declared-arithmetic"]
                } else {
                    Vec::new()
                };
                assert_eq!(success(file.query(sql, &options)), expected.as_bytes());

                for batch in ["1", "2", "8"] {
                    let mut streaming = options.clone();
                    streaming.extend(["--stream", "--batch-rows", batch]);
                    assert_eq!(
                        success(file.query(sql, &streaming)),
                        expected.as_bytes(),
                        "profile {profile}, declared {declared}, batch {batch}: {sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn distinct_waits_for_expression_and_native_eof_before_publishing_rows() {
    let file = FileCopy::new(fixture::database());
    let late_error = "SELECT DISTINCT TOP 1 CLng(10\\(Id-3)) AS K FROM Items";
    for batch in ["1", "2", "8"] {
        assert!(failure(file.query(late_error, &["--stream", "--batch-rows", batch])).is_empty());
    }

    let mut bytes = fixture::database();
    let start = DEFINITION_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET;
    bytes[start..start + size_of::<u32>()].copy_from_slice(&4_u32.to_le_bytes());
    let mismatch = FileCopy::new(bytes);
    for batch in ["1", "2", "8"] {
        assert!(
            failure(mismatch.query(
                "SELECT DISTINCT Id Mod 2 AS K FROM Items",
                &["--stream", "--batch-rows", batch],
            ))
            .is_empty()
        );
    }
}

#[test]
fn distinct_rejects_unproven_later_arms_before_any_output() {
    let file = FileCopy::new(fixture::database());
    for sql in [
        "SELECT DISTINCT CDbl(Id) FROM Items",
        "SELECT DISTINCT CStr(Id) FROM Items WHERE False",
        "SELECT Id FROM Items UNION ALL SELECT DISTINCT CDbl(Id) FROM Items WHERE False",
        "SELECT Q.Id FROM (SELECT DISTINCT Id FROM Items) Q",
    ] {
        for batch in ["1", "2"] {
            assert!(
                failure(file.query(sql, &["--stream", "--batch-rows", batch])).is_empty(),
                "{sql}"
            );
        }
    }
}
