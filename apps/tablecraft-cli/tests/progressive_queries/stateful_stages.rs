//! Nested barriers publish only after their own complete logical inputs.
use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

fn database(profile: u8) -> FileCopy {
    let mut bytes = fixture::database();
    bytes[PROFILE_OFFSET] = profile;
    if profile != 1 {
        bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard ACE DB\0");
    }
    FileCopy::new(bytes)
}

#[test]
fn nested_aggregate_stages_match_buffered_results_across_priority_profiles() {
    let cases = [
        (
            "SELECT Sum(N) AS Total,Count(*) AS [Groups] FROM \
             (SELECT Id Mod 2 AS K,Count(*) AS N FROM Items GROUP BY Id Mod 2) Q",
            &b"Total\tGroups\n3\t2\n"[..],
        ),
        (
            "SELECT Count(*) AS N FROM \
             (SELECT Count(*) AS N FROM Items WHERE False) Q",
            &b"N\n1\n"[..],
        ),
        (
            "SELECT Count(*) AS N FROM \
             (SELECT Id,Count(*) AS N FROM Items WHERE False GROUP BY Id) Q",
            &b"N\n0\n"[..],
        ),
        (
            "SELECT Count(*) AS N FROM \
             (SELECT TOP 1 Id Mod 2 AS K,Count(*) AS N \
             FROM Items GROUP BY Id Mod 2) Q WHERE K=0",
            &b"N\n0\n"[..],
        ),
        (
            "SELECT Sum(N) AS N FROM \
             (SELECT Sum(N) AS N FROM \
             (SELECT Count(*) AS N FROM Items GROUP BY Id Mod 2) A) B",
            &b"N\n3\n"[..],
        ),
    ];
    for profile in [1, 2, 5, 6] {
        let file = database(profile);
        for declared in [false, true] {
            let controls = if declared {
                vec!["--declared-arithmetic"]
            } else {
                Vec::new()
            };
            for (sql, expected) in cases {
                assert_eq!(success(file.query(sql, &controls)), expected);
                for batch in ["1", "2"] {
                    let mut streaming = controls.clone();
                    streaming.extend(["--stream", "--batch-rows", batch]);
                    assert_eq!(success(file.query(sql, &streaming)), expected);
                }
            }
        }
    }
}

#[test]
fn inner_final_expression_failure_never_completes_an_outer_aggregate() {
    let file = database(6);
    let inner = "(SELECT CLng(Id Mod 2) AS K,10\\(Id Mod 2) AS Bad \
                 FROM Items GROUP BY Id Mod 2) Q";
    let rows = format!("SELECT K FROM {inner}");
    assert_eq!(
        failure(file.query(&rows, &["--stream", "--batch-rows", "1"])),
        b"K\n1\n",
    );
    assert!(failure(file.query(&rows, &["--stream", "--batch-rows", "2"])).is_empty());

    for projection in ["Count(*) AS N", "Sum(K) AS N"] {
        let sql = format!("SELECT {projection} FROM {inner}");
        assert!(failure(file.query(&sql, &[])).is_empty());
        assert!(failure(file.query(&sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }
    let filtered = format!("SELECT K FROM {inner} WHERE False");
    assert!(failure(file.query(&filtered, &["--stream"])).is_empty());

    let guarded = "SELECT Sum(N) AS N FROM \
                   (SELECT Sum(10\\(Id Mod 2)) AS N FROM Items \
                   GROUP BY Id Mod 2 HAVING Id Mod 2<>0) Q";
    assert_eq!(success(file.query(guarded, &["--stream"])), b"N\n20\n");
}

#[test]
fn intermediate_extrema_keep_raw_strings_errors_and_natural_carriers() {
    let file = database(6);
    let sql = "SELECT LenB(S) AS B,CLng(E) AS E,VarType(N) AS V,N FROM \
               (SELECT Min(ChrW(55296)) AS S,Min(CVErr(7)) AS E,Min(Id) AS N \
               FROM Items WHERE Id=1) Q";
    let expected = b"B\tE\tV\tN\n2\t7\t3\t1\n";
    assert_eq!(success(file.query(sql, &[])), expected);
    assert_eq!(success(file.query(sql, &["--stream"])), expected);

    let raw = "SELECT S FROM (SELECT Min(ChrW(55296)) AS S \
               FROM Items WHERE Id=1) Q";
    assert!(failure(file.query(raw, &["--stream"])).is_empty());
}

#[test]
fn physical_eof_failure_prevents_all_nested_aggregate_publication() {
    let mut bytes = fixture::database();
    support::put32(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET,
        4,
    );
    let file = FileCopy::new(bytes);
    let sql = "SELECT Sum(N) AS N FROM \
               (SELECT Id Mod 2 AS K,Count(*) AS N FROM Items GROUP BY Id Mod 2) Q";
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
}
