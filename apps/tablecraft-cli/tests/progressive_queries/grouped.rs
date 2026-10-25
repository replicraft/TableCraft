use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const FORMAT_RANGE: Range<usize> = 4..20;

#[test]
fn grouped_aggregate_streams_match_exact_results_in_priority_native_profiles() {
    let sql = concat!(
        "SELECT CLng(Id Mod 2) AS K,Count(*) AS N,Sum(Score) AS S,",
        "Avg(Id) AS A,Min(Id) AS Lo,Max(Score) AS Hi FROM Items GROUP BY Id Mod 2",
    );
    let expected = b"K\tN\tS\tA\tLo\tHi\n1\t2\t40\t2\t1\t30\n0\t1\t20\t2\t2\t20\n";
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[PROFILE_OFFSET] = profile;
        if profile != 1 {
            bytes[FORMAT_RANGE].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
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

#[test]
fn grouped_text_extrema_use_explicit_policies_and_normal_tsv_escaping() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT CLng(Id Mod 2) AS K,Min(CStr(Score)) AS Low,",
        "Max(CStr(Score)) AS High FROM Items GROUP BY Id Mod 2",
    );
    let options = ["--number-format", "invariant", "--collation", "ordinal"];
    let expected =
        b"K\tLow\tHigh\n1\t\"\"\"10\"\"\"\t\"\"\"30\"\"\"\n0\t\"\"\"20\"\"\"\t\"\"\"20\"\"\"\n";
    assert_eq!(success(file.query(sql, &options)), expected);
    let mut streaming = options.to_vec();
    streaming.extend(["--stream", "--batch-rows", "1"]);
    assert_eq!(success(file.query(sql, &streaming)), expected);
}

#[test]
fn grouped_final_errors_retain_only_completed_prior_batches_and_stop_union_arms() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Count(*) AS N FROM Items UNION ALL ",
        "SELECT 10\\(Id Mod 2) FROM Items GROUP BY Id Mod 2 ",
        "UNION ALL SELECT Id FROM Items",
    );
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"N\n3\n10\n"
    );
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "2"])),
        b"N\n3\n10\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "3"])).is_empty());
}
