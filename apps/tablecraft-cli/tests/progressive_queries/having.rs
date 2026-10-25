use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const FORMAT_RANGE: Range<usize> = 4..20;
const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;

fn database(profile: u8) -> FileCopy {
    let mut bytes = fixture::database();
    bytes[PROFILE_OFFSET] = profile;
    if profile != 1 {
        bytes[FORMAT_RANGE].copy_from_slice(b"Standard ACE DB\0");
    }
    FileCopy::new(bytes)
}

#[test]
fn having_streams_match_independent_global_and_grouped_answers_in_priority_profiles() {
    let cases: [(&str, &[u8]); 4] = [
        (
            "SELECT Count(*) AS N,Sum(Score) AS S FROM Items HAVING Avg(Score)=20",
            b"N\tS\n3\t60\n",
        ),
        ("SELECT 7 AS X FROM Items HAVING Count(*)>0", b"X\n7\n"),
        (
            "SELECT CLng(Id Mod 2) AS K,Count(*) AS N FROM Items GROUP BY Id Mod 2 HAVING Sum(Score)>30",
            b"K\tN\n1\t2\n",
        ),
        (
            "SELECT Count(*) AS N FROM Items WHERE False HAVING Count(*)=0",
            b"N\n0\n",
        ),
    ];

    for profile in [1, 2, 5, 6] {
        let file = database(profile);
        for (sql, expected) in cases {
            for declared in [false, true] {
                let mut options = Vec::new();
                if declared {
                    options.push("--declared-arithmetic");
                }
                assert_eq!(success(file.query(sql, &options)), expected);
                for batch in ["1", "2"] {
                    let mut streaming = options.clone();
                    streaming.extend(["--stream", "--batch-rows", batch]);
                    assert_eq!(success(file.query(sql, &streaming)), expected);
                }
            }
        }
    }
}

#[test]
fn rejected_having_suppresses_selected_errors_and_accepts_only_true() {
    let file = database(6);
    for sql in [
        "SELECT Sum(1/0) AS Bad FROM Items HAVING False",
        "SELECT 1/0 AS Bad,Count(*) AS N FROM Items HAVING Count(*)=0",
        "SELECT ChrW(55296) AS Bad FROM Items HAVING Count(*)<0",
        "SELECT Sum(1/0) AS Bad FROM Items HAVING Null",
    ] {
        let expected = success(file.query(sql, &[]));
        assert!(expected.ends_with(b"\n"));
        assert_eq!(expected.iter().filter(|byte| **byte == b'\n').count(), 1);
        for batch in ["1", "2"] {
            assert_eq!(
                success(file.query(sql, &["--stream", "--batch-rows", batch])),
                expected
            );
        }
    }

    for sql in [
        "SELECT Sum(1/0) AS Bad FROM Items HAVING True",
        "SELECT Count(*) AS N FROM Items HAVING False And 1/0=0",
        "SELECT Count(*) AS Population FROM Items HAVING Population>0",
    ] {
        assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }
}

#[test]
fn having_top_counts_accepted_groups_and_keeps_later_eager_errors() {
    let file = database(6);
    let sql = concat!(
        "SELECT TOP 1 CLng(Q.K) AS K,Sum(Q.Score) AS S FROM ",
        "(SELECT Id Mod 3 AS K,Score FROM Items) Q ",
        "GROUP BY Q.K HAVING Q.K<>1",
    );
    let expected = b"K\tS\n2\t20\n";
    assert_eq!(success(file.query(sql, &[])), expected);
    for batch in ["1", "2"] {
        assert_eq!(
            success(file.query(sql, &["--stream", "--batch-rows", batch])),
            expected
        );
    }

    let late_error = concat!(
        "SELECT TOP 1 CLng(Id Mod 3) AS K,Sum(10\\(Id-3)) AS S ",
        "FROM Items GROUP BY Id Mod 3 HAVING Id Mod 3<>1",
    );
    assert!(failure(file.query(late_error, &[])).is_empty());
    assert_eq!(
        failure(file.query(late_error, &["--stream", "--batch-rows", "1"])),
        b"K\tS\n2\t-10\n"
    );
    assert!(failure(file.query(late_error, &["--stream", "--batch-rows", "2"])).is_empty());
}

#[test]
fn having_never_publishes_before_checked_eof_or_exhausted_limits() {
    let sql = "SELECT Count(*) AS N FROM Items HAVING Count(*)>0";
    let file = database(6);
    for options in [
        vec!["--stream", "--execution-rows", "1"],
        vec!["--stream", "--execution-steps", "1"],
        vec!["--stream", "--native-rows", "1"],
        vec!["--stream", "--output-bytes", "1"],
    ] {
        assert!(failure(file.query(sql, &options)).is_empty());
    }

    let mut bytes = fixture::database();
    let start = TABLE_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET;
    bytes[start..start + size_of::<u32>()].copy_from_slice(&4_u32.to_le_bytes());
    let invalid = FileCopy::new(bytes);
    assert!(failure(invalid.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
}
