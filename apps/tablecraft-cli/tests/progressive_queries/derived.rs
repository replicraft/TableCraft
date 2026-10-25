use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const FORMAT_RANGE: Range<usize> = 4..20;

#[test]
fn native_inline_pipelines_feed_rows_and_existing_aggregate_consumers() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[PROFILE_OFFSET] = profile;
        if profile != 1 {
            bytes[FORMAT_RANGE].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        for (sql, expected) in [
            (
                "SELECT Q.Id,Q.V FROM (SELECT Id,Score+1 AS V FROM Items) Q WHERE Q.Id>=2",
                b"Id\tV\n2\t21\n3\t31\n".as_slice(),
            ),
            (
                "SELECT Count(*) AS N,Sum(Q.V) AS S FROM (SELECT Score+1 AS V FROM Items) Q",
                b"N\tS\n3\t63\n".as_slice(),
            ),
            (
                "SELECT CLng(Q.Id Mod 2) AS K,Count(*) AS N,Sum(Q.V) AS S FROM (SELECT Id,Score+1 AS V FROM Items) Q GROUP BY Q.Id Mod 2",
                b"K\tN\tS\n1\t2\t42\n0\t1\t21\n".as_slice(),
            ),
        ] {
            assert_eq!(success(file.query(sql, &[])), expected);
            for batch in ["1", "2"] {
                assert_eq!(
                    success(file.query(sql, &["--stream", "--batch-rows", batch])),
                    expected
                );
            }
        }
    }
}

#[test]
fn inner_tops_and_unused_selected_expressions_keep_their_own_eager_phase() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Q.Id FROM (SELECT TOP 2 Id FROM Items) Q WHERE Q.Id>=3";
    assert_eq!(success(file.query(sql, &["--stream"])), b"Id\n");

    let filtered = "SELECT Q.Id FROM (SELECT Id,10\\(Id-2) AS Bad FROM Items) Q WHERE False";
    assert!(failure(file.query(filtered, &["--stream", "--batch-rows", "1"])).is_empty());
    let capped = "SELECT TOP 1 Q.Id FROM (SELECT Id,10\\(Id-2) AS Bad FROM Items) Q";
    assert_eq!(
        failure(file.query(capped, &["--stream", "--batch-rows", "1"])),
        b"Id\n1\n"
    );
    assert!(failure(file.query(capped, &["--stream", "--batch-rows", "2"])).is_empty());
}

#[test]
fn retained_inner_raw_and_error_values_wait_for_their_actual_outer_consumers() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT LenB(Q.Raw) AS B,CLng(Q.Err) AS E FROM ",
        "(SELECT Id,ChrW(55296) AS Raw,CVErr(Id) AS Err FROM Items) Q WHERE Q.Id=1",
    );
    assert_eq!(success(file.query(sql, &["--stream"])), b"B\tE\n2\t1\n");
    let unused = "SELECT Q.Id FROM (SELECT Id,ChrW(55296) AS Raw,CVErr(Id) AS Err FROM Items) Q";
    assert_eq!(success(file.query(unused, &["--stream"])), b"Id\n1\n2\n3\n");
}
